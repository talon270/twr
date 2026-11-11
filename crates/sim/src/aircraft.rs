//! TWR-SIM · AIRCRAFT
//!   · Phase: where an aircraft is in its departure, on stand to departed
//!   · Aircraft: position on its route in integer milli-decimetres, speed,
//!     the runway it must stop short of, and the edges it has entered
//!   · verbs: push, taxi, hold, cross, line up, take off. Each refuses with
//!     a reason; `verbs()` lists the ones that would succeed right now
//!   · hold-short limits: where a route stops before each runway it meets
//!   · Wake: the ICAO wake category an aircraft carries (PLAN-twr.md § B4, W0)
//!   · arrivals (B5): final, landing roll braking late to its exit, the
//!     exit taxiway to the vacate point; verbs C clear to land, G go
//!     around, E pick the exit. A landing with no exit is B4's synthetic
//!     arrival and stops on the runway
//!   · ground control (C3): hold at a point and continue, reroute from a
//!     node already passed, taxi speed, cross without stopping, backtrack.
//!     Give way needs a second aircraft, so it lives on `World`
//!
//! An aircraft never enters a runway it was not cleared onto. Every route it
//! receives is scanned for runway nodes, and it stops short of the first one
//! not yet cleared, at the OSM holding point if one is on the route, or at
//! HOLD_SHORT_M before the runway otherwise.

use crate::airport::{Airport, EdgeKind};
use crate::arrival::{self, brake_mdm, Exit, GoAround, DECISION_M, FINAL_SPAWN_M, GA_CIRCUIT_S};
use crate::route::{Graph, Request, Route};
use crate::TICK_HZ;

/// Movement unit: milli-decimetres (0.1 mm). Integer, so a takeoff roll's
/// acceleration adds no float error and two runs agree to the unit.
const MDM_PER_DM: u64 = 1000;
/// Milli-decimetres in a metre, for rules written in metres.
pub const MDM_PER_M: u64 = 10_000;

/// Taxi speed per tick at 20 Hz: 5 dm, so 10 m/s, about 19 kt. One speed for
/// straight and turning taxi until turns get their own.
pub const TAXI_MDM_PER_TICK: u64 = 5_000;
/// Pushback: 1 m/s, a walking-pace tug. Authored, not measured.
pub const PUSH_MDM_PER_TICK: u64 = 500;
/// Takeoff acceleration, 2 m/s² (an A320-class average to rotation;
/// authored). At 20 Hz: 20 dm/s² / 400 = 0.05 dm per tick per tick.
pub const TAKEOFF_ACCEL_MDM: u64 = 50;
/// Rotation speed, 75 m/s (about 146 kt; authored). At 2 m/s² the roll to
/// here is v² / 2a = 1,406 m.
pub const ROTATE_MDM_PER_TICK: u64 = 37_500;
/// Ticks from K to lift-off: every roll starts at rest and gains
/// TAKEOFF_ACCEL_MDM per tick until ROTATE_MDM_PER_TICK, so 750 ticks
/// (37.5 s game). The wake timer counts on this being the same for all.
pub const ROLL_TICKS: u64 = ROTATE_MDM_PER_TICK / TAKEOFF_ACCEL_MDM;
/// Climb-out speed cap, 90 m/s, until the aircraft leaves the runway's end.
pub const CLIMB_MDM_PER_TICK: u64 = 45_000;
/// Distance short of a runway node to stop when no OSM holding point is on
/// the route. 90 m is the runway-holding distance this author recalls from
/// ICAO Annex 14 Table 3-2 for a code 4 instrument runway; not checked
/// against the table.
pub const HOLD_SHORT_M: u64 = 90;
/// How far before a runway node an OSM holding point still counts as the
/// one guarding it.
pub const HOLD_SEARCH_M: u64 = 250;
/// Speed over the threshold for a synthetic arrival, 70 m/s (about 136 kt;
/// authored, B5 may retune).
pub const LAND_MDM_PER_TICK: u64 = 35_000;
/// Landing-roll deceleration, 1.6 m/s² (authored). At 20 Hz: 16 dm/s² /
/// 400 = 0.04 dm per tick per tick. 70 to 10 m/s takes 37.5 s and 1,500 m.
pub const LAND_DECEL_MDM: u64 = 40;
/// Least runway ahead for a takeoff, by class (B9, from the `aircraft`
/// research). Narrowbody and turboprop: 2,030 m, an A320neo at MTOW at
/// Delhi's elevation (Airbus chart, read by eye, ±100 m). Widebody:
/// 3,100 m, a 777-300ER likewise (Boeing chart, ±150 m). Every H and J
/// type here is a widebody and every M type is not, so wake picks the
/// class. MTOW is the conservative case; a lighter aircraft needs less.
pub const MIN_TAKEOFF_RUN_M: u64 = 2_030;
pub const MIN_TAKEOFF_RUN_HEAVY_M: u64 = 3_100;

/// C3 taxi speeds per tick at 20 Hz: slow 5 m/s, expedite 15 m/s. Both
/// `authored`; normal is TAXI_MDM_PER_TICK (10 m/s).
pub const SLOW_MDM_PER_TICK: u64 = 2_500;
pub const EXPEDITE_MDM_PER_TICK: u64 = 7_500;
/// C3 give way: the giver stops this far short of the junction, along its
/// route. Over BODY_M (20 m), so the passing aircraft's G1 check does not
/// see a stopped giver at all, and over GROUND_MIN_M (45 m) for an aircraft
/// that turns onto the giver's own route. `authored`.
pub const GIVE_WAY_GAP_M: u64 = 60;

/// C3: the taxi speed you gave. It applies to taxi and line-up (backtrack
/// included) and stays with the aircraft until you change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaxiSpeed {
    Slow,
    Normal,
    Expedite,
}

impl TaxiSpeed {
    pub fn mdm_per_tick(self) -> u64 {
        match self {
            TaxiSpeed::Slow => SLOW_MDM_PER_TICK,
            TaxiSpeed::Normal => TAXI_MDM_PER_TICK,
            TaxiSpeed::Expedite => EXPEDITE_MDM_PER_TICK,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TaxiSpeed::Slow => "slow",
            TaxiSpeed::Normal => "normal",
            TaxiSpeed::Expedite => "expedite",
        }
    }

    pub fn slower(self) -> Option<TaxiSpeed> {
        match self {
            TaxiSpeed::Expedite => Some(TaxiSpeed::Normal),
            TaxiSpeed::Normal => Some(TaxiSpeed::Slow),
            TaxiSpeed::Slow => None,
        }
    }

    pub fn faster(self) -> Option<TaxiSpeed> {
        match self {
            TaxiSpeed::Slow => Some(TaxiSpeed::Normal),
            TaxiSpeed::Normal => Some(TaxiSpeed::Expedite),
            TaxiSpeed::Expedite => None,
        }
    }
}

/// C3: where a taxiing aircraft stops before the end of its route. A hold
/// point (J) stops at the node itself and then waits for Enter; a give way
/// (Y) stops GIVE_WAY_GAP_M short of the junction and the world releases it
/// once the other aircraft has passed (`World::ground_tick`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    /// The hold point, or the junction given way at.
    pub node: u32,
    /// Route distance to stop at, mdm. Never behind the aircraft: every new
    /// route drops the stop, so `pos_mdm` cannot pass it unnoticed.
    pub at_mdm: u64,
    /// Giving way to this callsign.
    pub give_way: Option<String>,
}

pub fn min_takeoff_run_m(wake: Wake) -> u64 {
    match wake {
        Wake::H | Wake::J => MIN_TAKEOFF_RUN_HEAVY_M,
        Wake::M | Wake::L => MIN_TAKEOFF_RUN_M,
    }
}

/// ICAO wake turbulence category. L 7,000 kg or less; M over 7,000 and
/// under 136,000; H 136,000 or more; J the A380-800 (GACA AIC 07/2020,
/// mass limits from Doc 4444 4.9.1 via a snippet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Wake {
    L,
    M,
    H,
    J,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    OnStand,
    Pushback,
    /// Stopped with nothing pending: pushed back, or at the end of a route.
    Holding,
    Taxi,
    /// Stopped mid-route by H; H again continues.
    HoldPosition,
    /// Stopped short of this runway (its name), waiting for X or L.
    HoldShort(String),
    /// Cleared onto this runway, rolling to the entry.
    LineUp(String),
    /// On the runway, facing down this designator, waiting for K.
    LinedUp(String),
    TakeoffRoll(String),
    Airborne(String),
    Departed(String),
    /// Landing roll on this designator. With an exit it brakes to reach
    /// taxi speed at the exit node, then turns `Taxi` on the exit route.
    /// Without one (B4's synthetic arrival) it slows and stops on the runway.
    Landing(String),
    /// On final for this designator, off the network: `air_mdm` before the
    /// threshold. Name kept stable: the scripts read the Debug text.
    Final(String),
    /// Climbing out over this designator after a go-around, then out of
    /// sight on the circuit until it is back on final.
    GoAround(String),
    /// B6: an arrival that has shut down on its stand and left the game.
    ShutDown,
}

#[derive(Debug, Clone)]
pub struct Aircraft {
    pub callsign: String,
    pub wake: Wake,
    /// ICAO type designator (B6); the strip shows it beside the wake.
    pub type_code: &'static str,
    /// B6 departures: the runway traffic assigned. Advice only; nothing
    /// holds you to it.
    pub assigned: Option<&'static str>,
    /// B7 departures: the tick it fell due, from which its stand grace runs.
    pub ready: Option<u64>,
    pub phase: Phase,
    pub route: Route,
    /// Index into `route.edges` of the edge being rolled along; equal to
    /// `route.edges.len()` once at the end.
    pub leg: usize,
    pub along_mdm: u64,
    /// Distance from the route's first node, so a limit can be one number.
    pub pos_mdm: u64,
    pub speed_mdm: u64,
    /// Stop here (route distance) and the runway it guards.
    pub limit: Option<(u64, String)>,
    /// Crossings cleared on the current route, each as the route index of
    /// its first runway node. Per crossing, not per runway name: a route that
    /// meets one runway twice stops short of it twice (found by the B4
    /// 10-hour run, where name-based clearance left aircraft parked on 09/27).
    pub cleared: Vec<usize>,
    /// Every edge entered, in order. What the aircraft actually did, kept so
    /// a test can compare it with what the screen drew.
    pub trail: Vec<u32>,
    /// Arrivals: the exit the landing route ends through. Cleared by any
    /// new route, so it marks "still on the landing route".
    pub exit: Option<Exit>,
    /// Arrivals in the air: signed distance along the landing direction
    /// from the threshold. Negative on final, positive past it.
    pub air_mdm: i64,
    pub cleared_to_land: bool,
    /// A pilot go-around waiting to happen at this distance out, drawn from
    /// the world's seeded stream when the aircraft comes onto final.
    pub pilot_ga: Option<(u64, &'static str)>,
    /// B9: a windshear go-around at this distance out, gusty shifts only.
    pub weather_ga: Option<u64>,
    /// No landing clearance by this distance out and the aircraft goes
    /// around: DECISION_M, or 2 NM under LVP (V1). The world sets it each
    /// time the aircraft comes onto final.
    pub decide_mdm: u64,
    /// Why the last go-around happened.
    pub went_around: Option<GoAround>,
    /// Ticks left on the circuit after a go-around has left the runway.
    pub circuit: u64,
    /// Came in on final: it shuts down on its stand and leaves the game.
    /// Per aircraft, not per traffic stream, so Practice's arrival leaves too.
    pub arrival: bool,
    /// The tick an arrival was first seen on a stand; its shut-down timer.
    pub stand_since: Option<u64>,
    /// C2: route distance where a tail push stops, mdm. None for a push that
    /// ends at the junction (`push`).
    pub push_to: Option<u64>,
    /// C3: a hold point or a give way ahead on the route.
    pub stop: Option<Stop>,
    /// C3: taxi and line-up speed.
    pub taxi_speed: TaxiSpeed,
    /// C3: X given before the hold: the crossing (route index of its first
    /// runway node) and its runway. Crossed without stopping if the runway
    /// is clear when the aircraft reaches the hold line.
    pub cross_ahead: Option<(usize, String)>,
    /// C3: backtracking to line up facing this designator. Set from B until
    /// the aircraft is lined up at the far end.
    pub backtrack: Option<String>,
}

/// C2: one way to push back. Down the stand link, then tail-first along a
/// taxilane for one aircraft length, ending facing the other way.
#[derive(Debug, Clone)]
pub struct TailOption {
    /// "tail W": the compass point the tail ends up toward.
    pub label: String,
    /// The taxiway the tail goes down; empty if the map leaves it unnamed.
    pub taxiway: String,
    /// Stand, junction, then the nodes the tail follows.
    pub route: Route,
    /// Route distance where the push stops, mdm.
    pub push_mdm: u64,
}

impl Aircraft {
    pub fn parked(callsign: &str, stand_node: u32) -> Aircraft {
        Aircraft {
            callsign: callsign.into(),
            // IGO512 is an A320neo: 79,000 kg MTOW, MEDIUM. Traffic sets both per type.
            wake: Wake::M,
            type_code: "A20N",
            assigned: None,
            ready: None,
            phase: Phase::OnStand,
            route: Route::at(stand_node),
            leg: 0,
            along_mdm: 0,
            pos_mdm: 0,
            speed_mdm: 0,
            limit: None,
            cleared: Vec::new(),
            trail: Vec::new(),
            exit: None,
            air_mdm: 0,
            cleared_to_land: false,
            pilot_ga: None,
            weather_ga: None,
            decide_mdm: DECISION_M * MDM_PER_M,
            went_around: None,
            circuit: 0,
            arrival: false,
            stand_since: None,
            push_to: None,
            stop: None,
            taxi_speed: TaxiSpeed::Normal,
            cross_ahead: None,
            backtrack: None,
        }
    }

    /// An arrival FINAL_SPAWN_M out on final for `desig`, set to take the
    /// default exit. None if `desig` is not a landing direction here.
    pub fn on_final(callsign: &str, wake: Wake, desig: &str, ap: &Airport, g: &Graph) -> Option<Aircraft> {
        let (x, route) = arrival::default_exit(ap, g, desig)?;
        let mut a = Aircraft { wake, arrival: true, ..Aircraft::parked(callsign, route.nodes[0]) };
        a.set_route(route, ap, g);
        a.limit = None;
        a.exit = Some(x);
        a.back_on_final(desig);
        Some(a)
    }

    pub(crate) fn back_on_final(&mut self, desig: &str) {
        self.phase = Phase::Final(desig.into());
        self.air_mdm = -((FINAL_SPAWN_M * MDM_PER_M) as i64);
        self.speed_mdm = LAND_MDM_PER_TICK;
        self.cleared_to_land = false;
        self.pilot_ga = None;
        self.weather_ga = None;
    }

    fn go_around_now(&mut self, why: GoAround) {
        let Phase::Final(d) = &self.phase else { return };
        self.phase = Phase::GoAround(d.clone());
        self.went_around = Some(why);
        self.cleared_to_land = false;
        self.pilot_ga = None;
        self.weather_ga = None;
    }

    /// Game ticks until this arrival is clear of its runway, by R4's own
    /// measure: past its vacate point. None for anything that is not an
    /// arrival still to land, landing, or still between exit node and vacate
    /// point. Late braking as in `step`: the profile `a9` measures (62.6 s to
    /// the exit node on 29L) plus the taxi to the vacate point at taxi speed.
    pub fn ticks_to_clear(&self) -> Option<u64> {
        let x = self.exit.as_ref()?;
        let taxi = (x.vacate_mdm - x.at_mdm) / TAXI_MDM_PER_TICK;
        let roll = |left: u64, v: u64| {
            let slow = brake_mdm(v);
            left.saturating_sub(slow) / v + (v - TAXI_MDM_PER_TICK) / LAND_DECEL_MDM
        };
        match &self.phase {
            Phase::Final(_) => Some((-self.air_mdm).max(0) as u64 / LAND_MDM_PER_TICK + roll(x.at_mdm, LAND_MDM_PER_TICK) + taxi),
            Phase::Landing(_) => Some(roll(x.at_mdm.saturating_sub(self.pos_mdm), self.speed_mdm.max(TAXI_MDM_PER_TICK)) + taxi),
            Phase::Taxi | Phase::Holding | Phase::HoldPosition if self.arrival && self.pos_mdm < x.vacate_mdm => Some((x.vacate_mdm - self.pos_mdm) / TAXI_MDM_PER_TICK),
            _ => None,
        }
    }

    /// Distance to the threshold while on final, metres.
    pub fn final_m(&self) -> f64 {
        (-self.air_mdm).max(0) as f64 / MDM_PER_M as f64
    }

    /// Over the runway or still before its far end, on a go-around.
    pub fn over_runway(&self) -> bool {
        matches!(self.phase, Phase::GoAround(_)) && self.exit.as_ref().is_some_and(|x| self.air_mdm >= 0 && (self.air_mdm as u64) < x.run_mdm)
    }

    /// Game ticks until a go-around is back on final: the rest of the
    /// circuit once it has left the runway.
    pub fn circuit_left(&self) -> u64 {
        self.circuit
    }

    /// A synthetic arrival crossing the threshold at `entry`, rolling down
    /// the longer run from it. None if `entry` is not on a runway.
    pub fn landing(callsign: &str, wake: Wake, entry: u32, ap: &Airport, g: &Graph) -> Option<Aircraft> {
        let (run, d) = g.runway_run(ap, entry)?;
        let mut a = Aircraft { wake, ..Aircraft::parked(callsign, entry) };
        a.set_route(run, ap, g);
        a.limit = None;
        a.start_rolling();
        a.speed_mdm = LAND_MDM_PER_TICK;
        a.phase = Phase::Landing(d);
        Some(a)
    }

    pub fn on_edge(&self) -> bool {
        self.leg < self.route.edges.len()
    }

    /// Where a new taxi route must start: the node ahead if the aircraft is
    /// on an edge (it can't turn mid-edge), else the node it is at.
    pub fn next_node(&self) -> u32 {
        if self.on_edge() { self.route.nodes[self.leg + 1] } else { self.route.end() }
    }

    /// Position in metres east and north of the airport origin.
    pub fn pos_m(&self, ap: &Airport, g: &Graph) -> (f64, f64) {
        let n = |i: u32| {
            let p = ap.nodes[i as usize];
            (f64::from(p.x) / 10.0, f64::from(p.y) / 10.0)
        };
        if matches!(self.phase, Phase::Final(_) | Phase::GoAround(_)) {
            // Off the network: along the line of the runway's first edge.
            let (a, b) = (n(self.route.nodes[0]), n(self.route.nodes[1]));
            let l = (b.0 - a.0).hypot(b.1 - a.1);
            let s = self.air_mdm as f64 / MDM_PER_M as f64 / l;
            return (a.0 + (b.0 - a.0) * s, a.1 + (b.1 - a.1) * s);
        }
        if !self.on_edge() {
            return n(self.route.end());
        }
        let (a, b) = (n(self.route.nodes[self.leg]), n(self.route.nodes[self.leg + 1]));
        let t = self.along_mdm as f64 / (u64::from(g.len_dm[self.route.edges[self.leg] as usize]) * MDM_PER_DM) as f64;
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }

    /// Departed, or out on the circuit after a go-around: not drawn.
    pub fn visible(&self) -> bool {
        match self.phase {
            Phase::Departed(_) | Phase::ShutDown => false,
            Phase::GoAround(_) => self.air_mdm < 0 || self.over_runway(),
            _ => true,
        }
    }

    /// Route distance still to roll, in metres.
    pub fn metres_left(&self, g: &Graph) -> f64 {
        let total: u64 = self.route.edges.iter().map(|&e| u64::from(g.len_dm[e as usize]) * MDM_PER_DM).sum();
        (total - self.pos_mdm) as f64 / 10_000.0
    }

    // ---- Movement ----------------------------------------------------------

    pub(crate) fn step(&mut self, ap: &Airport, g: &Graph) {
        match &self.phase {
            Phase::Final(d) => {
                let out = (-self.air_mdm) as u64;
                if self.pilot_ga.is_some_and(|(at, _)| out <= at) {
                    let why = GoAround::Pilot(self.pilot_ga.expect("checked").1);
                    self.go_around_now(why);
                } else if self.weather_ga.is_some_and(|at| out <= at) {
                    self.go_around_now(GoAround::Weather("windshear"));
                } else if !self.cleared_to_land && out <= self.decide_mdm {
                    self.go_around_now(GoAround::NoClearance);
                } else {
                    self.air_mdm = (self.air_mdm + LAND_MDM_PER_TICK as i64).min(0);
                    if self.air_mdm == 0 {
                        // Over the threshold: the landing roll starts on the route.
                        self.phase = Phase::Landing(d.clone());
                        self.start_rolling();
                    }
                }
                return;
            }
            Phase::GoAround(_) => {
                let run = self.exit.as_ref().map_or(0, |x| x.run_mdm) as i64;
                if self.air_mdm < run {
                    self.speed_mdm = (self.speed_mdm + TAKEOFF_ACCEL_MDM).min(CLIMB_MDM_PER_TICK);
                    self.air_mdm += self.speed_mdm as i64;
                    if self.air_mdm >= run {
                        self.circuit = GA_CIRCUIT_S * u64::from(TICK_HZ);
                    }
                } else if self.circuit > 0 {
                    self.circuit -= 1;
                }
                // Circuit done: the world puts it back on final once approach
                // has a slot (`traffic::final_slot_free`, A12).
                return;
            }
            _ => {}
        }
        let v = match &self.phase {
            Phase::Pushback => PUSH_MDM_PER_TICK,
            Phase::Taxi | Phase::LineUp(_) => self.taxi_speed.mdm_per_tick(),
            Phase::TakeoffRoll(_) | Phase::Airborne(_) => {
                self.speed_mdm = (self.speed_mdm + TAKEOFF_ACCEL_MDM).min(CLIMB_MDM_PER_TICK);
                self.speed_mdm
            }
            Phase::Landing(_) if self.exit.is_some() => {
                // Brake as late as possible: the highest speed from which
                // LAND_DECEL_MDM still makes taxi speed at the exit node.
                // Integer square root, so two runs agree to the unit.
                let left = self.exit.as_ref().expect("guarded").at_mdm.saturating_sub(self.pos_mdm);
                let cap = (TAXI_MDM_PER_TICK * TAXI_MDM_PER_TICK + 2 * LAND_DECEL_MDM * left).isqrt();
                self.speed_mdm = self.speed_mdm.min(cap).max(TAXI_MDM_PER_TICK);
                self.speed_mdm
            }
            Phase::Landing(_) => {
                self.speed_mdm = if self.speed_mdm > TAXI_MDM_PER_TICK {
                    (self.speed_mdm - LAND_DECEL_MDM).max(TAXI_MDM_PER_TICK)
                } else {
                    0
                };
                self.speed_mdm
            }
            _ => return,
        };
        let stop = self.stop.as_ref().filter(|_| self.phase == Phase::Taxi).map(|s| s.at_mdm);
        let cap = self.limit.as_ref().map(|(lim, _)| *lim).into_iter().chain(self.push_to).chain(stop).min();
        let go = cap.map_or(v, |c| v.min(c - self.pos_mdm));
        self.advance(go, g);
        if self.push_to == Some(self.pos_mdm) {
            self.end_tail_push(g);
            return;
        }

        if let Some((lim, rwy)) = &self.limit {
            if self.pos_mdm == *lim {
                self.phase = Phase::HoldShort(rwy.clone());
                return;
            }
        }
        // C3: at a hold point, wait for Enter; at a give way, for the world.
        if self.phase == Phase::Taxi && stop == Some(self.pos_mdm) {
            self.phase = Phase::HoldPosition;
            if self.stop.as_ref().is_some_and(|s| s.give_way.is_none()) {
                self.stop = None;
            }
            return;
        }
        if let Phase::TakeoffRoll(d) = &self.phase {
            if self.speed_mdm >= ROTATE_MDM_PER_TICK {
                self.phase = Phase::Airborne(d.clone());
            }
        }
        // Past the exit node: off the runway and onto the exit taxiway, where
        // the ground check sees it like any other taxiing aircraft.
        if matches!(self.phase, Phase::Landing(_)) && self.exit.as_ref().is_some_and(|x| self.pos_mdm >= x.at_mdm) {
            self.phase = Phase::Taxi;
            self.speed_mdm = TAXI_MDM_PER_TICK;
        }
        // C3 backtrack. At the entry: roll back down the runway to its end.
        // At the end (the route is runway): line up below, as any LineUp.
        if !self.on_edge()
            && matches!(self.phase, Phase::LineUp(_))
            && let Some(d) = self.backtrack.take()
            && !self.on_runway_route(ap)
            && let Some(run) = backtrack_run(ap, g, self.route.end(), &d)
        {
            self.start_backtrack(run, d, ap, g);
            return;
        }
        if !self.on_edge() {
            self.phase = match std::mem::replace(&mut self.phase, Phase::Holding) {
                Phase::LineUp(_) => match g.runway_run_facing(ap, self.route.end(), self.assigned) {
                    Some((run, d)) => {
                        self.line_up_on(run, ap, g);
                        Phase::LinedUp(d)
                    }
                    None => Phase::Holding,
                },
                Phase::TakeoffRoll(d) | Phase::Airborne(d) => Phase::Departed(d),
                Phase::Landing(d) => Phase::Landing(d),
                // A route that ends through a stand link ends on the stand.
                _ if self.route.edges.last().is_some_and(|&e| ap.edges[e as usize].kind == EdgeKind::StandLink)
                    && ap.stands.iter().any(|s| s.node == self.route.end()) =>
                {
                    Phase::OnStand
                }
                _ => Phase::Holding,
            };
        }
    }

    /// The tail push has gone its length. The aircraft now sits on the last
    /// edge it was pushed along, facing back down it (toward the junction),
    /// so a taxi from the node ahead leaves without a U-turn, and the edge
    /// behind it is the one a taxi may not return along.
    fn end_tail_push(&mut self, g: &Graph) {
        let (k, along) = if self.on_edge() { (self.leg, self.along_mdm) } else { (self.route.edges.len() - 1, u64::MAX) };
        let (e, near, far) = (self.route.edges[k], self.route.nodes[k], self.route.nodes[k + 1]);
        let len = u64::from(g.len_dm[e as usize]) * MDM_PER_DM;
        let from_far = len - along.min(len);
        self.route = Route { nodes: vec![far, near], edges: vec![e] };
        self.leg = 0;
        self.along_mdm = from_far;
        self.pos_mdm = from_far;
        self.speed_mdm = 0;
        self.limit = None;
        self.cleared.clear();
        self.push_to = None;
        self.phase = Phase::Holding;
    }

    fn advance(&mut self, mut go: u64, g: &Graph) {
        self.pos_mdm += go;
        while self.on_edge() {
            let len = u64::from(g.len_dm[self.route.edges[self.leg] as usize]) * MDM_PER_DM;
            if self.along_mdm + go < len {
                self.along_mdm += go;
                return;
            }
            go -= len - self.along_mdm;
            self.leg += 1;
            self.along_mdm = 0;
            if let Some(&e) = self.route.edges.get(self.leg) {
                self.trail.push(e);
            }
        }
        // Rolled off the end: give back what could not be used.
        self.pos_mdm -= go;
    }

    /// Start a fresh route, stationary at its first node. The first edge is
    /// recorded when the aircraft actually starts along it.
    fn set_route(&mut self, route: Route, ap: &Airport, g: &Graph) {
        self.route = route;
        self.leg = 0;
        self.along_mdm = 0;
        self.pos_mdm = 0;
        self.speed_mdm = 0;
        self.cleared.clear();
        self.exit = None;
        self.stop = None;
        self.cross_ahead = None;
        self.limit = self.next_hold(ap, g);
    }

    /// Route distance of `route.nodes[k]`, mdm.
    pub fn dist_to(&self, k: usize, g: &Graph) -> u64 {
        self.route.edges[..k].iter().map(|&e| u64::from(g.len_dm[e as usize]) * MDM_PER_DM).sum()
    }

    /// The route runs along a runway: a backtrack, or a lined-up run.
    pub fn on_runway_route(&self, ap: &Airport) -> bool {
        self.route.edges.first().is_some_and(|&e| ap.edges[e as usize].kind == EdgeKind::Runway)
    }

    /// Roll along `run` (runway edges, against `d`) to the runway's end.
    fn start_backtrack(&mut self, run: Route, d: String, ap: &Airport, g: &Graph) {
        let rwy = ap.runways[g.runway_at[run.nodes[0] as usize].expect("a runway run")].name.clone();
        self.set_route(run, ap, g);
        // All runway: no hold-short of its own, as for a lined-up run.
        self.limit = None;
        self.start_rolling();
        self.phase = Phase::LineUp(rwy);
        self.backtrack = Some(d);
    }

    /// Record the first edge of a fresh route as entered. Called once, when
    /// the aircraft is set moving on a route `set_route` just installed.
    fn start_rolling(&mut self) {
        if let Some(&e) = self.route.edges.first() {
            self.trail.push(e);
        }
    }

    /// Stand on the runway at `run`'s first node, facing down it. The run is
    /// all runway, so it gets no hold-short limit of its own.
    fn line_up_on(&mut self, run: Route, ap: &Airport, g: &Graph) {
        self.set_route(run, ap, g);
        self.limit = None;
    }

    /// The first runway ahead on the route not yet cleared, and where to
    /// stop for it, never behind where the aircraft already is.
    fn next_hold(&self, ap: &Airport, g: &Graph) -> Option<(u64, String)> {
        crossings(ap, g, &self.route, 0, 0)
            .into_iter()
            .find(|c| c.last > self.leg && !self.cleared.contains(&c.first))
            .map(|c| (c.lim.max(self.pos_mdm), c.name))
    }

    // ---- Verbs -------------------------------------------------------------

    pub fn push(&mut self, ap: &Airport, g: &Graph) -> Result<(), String> {
        if self.phase != Phase::OnStand {
            return Err("only an aircraft on stand can push back".into());
        }
        let stand = self.route.end();
        let (ei, e) = ap.edges.iter().enumerate()
            .find(|(_, e)| e.kind == EdgeKind::StandLink && (e.a == stand || e.b == stand))
            .ok_or("this stand has no link to a taxiway")?;
        let other = if e.a == stand { e.b } else { e.a };
        self.set_route(Route { nodes: vec![stand, other], edges: vec![ei as u32] }, ap, g);
        self.start_rolling();
        self.phase = Phase::Pushback;
        Ok(())
    }

    /// C2: the ways this aircraft can push back from its stand. One per
    /// taxilane at the stand link's junction; each follows the lane as
    /// straight ahead as it goes until the aircraft's length is used up (the
    /// edges at IGI stands are short: median 38 m, 252 of 525 under an A320's
    /// length), or the lane ends. Lanes that would trap the aircraft facing
    /// a dead end are left out; with fewer than two lanes left, none are offered.
    pub fn tail_options(&self, ap: &Airport, g: &Graph) -> Vec<TailOption> {
        let Some((ei, junction)) = self.stand_link(ap) else { return Vec::new() };
        let stand = self.route.end();
        let want = (crate::shape::dims(self.type_code).0 * MDM_PER_M as f64) as u64;
        let len = |e: u32| u64::from(g.len_dm[e as usize]) * MDM_PER_DM;
        let pos = |n: u32| (f64::from(ap.nodes[n as usize].x), f64::from(ap.nodes[n as usize].y));
        // Compass bearing of travel from node p to node q, degrees, 0 = north.
        let bearing = |p: u32, q: u32| {
            let (a, b) = (pos(p), pos(q));
            (b.0 - a.0).atan2(b.1 - a.1).to_degrees().rem_euclid(360.0)
        };
        let turn = |h1: f64, h2: f64| {
            let d = (h2 - h1).rem_euclid(360.0);
            d.min(360.0 - d)
        };
        let lane = |e: u32| ap.edges[e as usize].kind != EdgeKind::StandLink;
        let lanes: Vec<(u32, u32)> = g.adj[junction as usize].iter().copied().filter(|&(e, _)| e != ei && lane(e)).collect();
        let exit = ap.runway_exits().first().copied();
        let mut out = Vec::new();
        for (e0, v0) in lanes {
            let (mut nodes, mut edges, mut got) = (vec![stand, junction, v0], vec![ei, e0], len(e0));
            while got < want {
                let (cur, prev, last) = (nodes[nodes.len() - 1], nodes[nodes.len() - 2], edges[edges.len() - 1]);
                let h = bearing(prev, cur);
                let next = g.adj[cur as usize]
                    .iter()
                    .filter(|&&(e, v)| e != last && lane(e) && !nodes.contains(&v))
                    .min_by(|a, b| turn(h, bearing(cur, a.1)).total_cmp(&turn(h, bearing(cur, b.1))));
                let Some(&(e, v)) = next else { break };
                nodes.push(v);
                edges.push(e);
                got += len(e);
            }
            // A lane that leaves the nose pointing at a dead end traps the
            // aircraft: from the node ahead, with the pushed edge off limits,
            // it must still be able to reach a runway.
            let (ahead, pushed) = (nodes[nodes.len() - 2], edges[edges.len() - 1]);
            let free = exit.is_some_and(|to| g.plan(ap, &Request { from: ahead, to, via: Vec::new(), avoid: vec![pushed], ..Default::default() }).is_some());
            if !free {
                continue;
            }
            const POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
            let b = bearing(junction, nodes[nodes.len() - 1]);
            out.push(TailOption {
                label: format!("tail {}", POINTS[((b + 22.5) / 45.0) as usize % 8]),
                taxiway: ap.edges[e0 as usize].name.clone(),
                route: Route { nodes, edges },
                push_mdm: len(ei) + got.min(want),
            });
        }
        // Fewer than two ways out is no choice: `push` stops at the junction.
        if out.len() < 2 {
            out.clear();
        }
        out
    }

    /// The edge joining this aircraft's stand to a taxiway, and the taxiway end.
    fn stand_link(&self, ap: &Airport) -> Option<(u32, u32)> {
        let stand = self.route.end();
        let (ei, e) = ap.edges.iter().enumerate().find(|(_, e)| e.kind == EdgeKind::StandLink && (e.a == stand || e.b == stand))?;
        Some((ei as u32, if e.a == stand { e.b } else { e.a }))
    }

    /// C2: push back along `opt`.
    pub fn push_tail(&mut self, opt: &TailOption, ap: &Airport, g: &Graph) -> Result<(), String> {
        if self.phase != Phase::OnStand {
            return Err("only an aircraft on stand can push back".into());
        }
        self.set_route(opt.route.clone(), ap, g);
        self.start_rolling();
        self.push_to = Some(opt.push_mdm);
        self.phase = Phase::Pushback;
        Ok(())
    }

    /// Taxi along `route`, exactly as given. On an edge, the aircraft
    /// finishes it first, so the route must start where that edge ends.
    pub fn taxi(&mut self, route: Route, ap: &Airport, g: &Graph) -> Result<(), String> {
        match &self.phase {
            Phase::OnStand => return Err("push back first".into()),
            Phase::Pushback | Phase::LineUp(_) | Phase::TakeoffRoll(_) | Phase::Airborne(_) | Phase::Departed(_) | Phase::Landing(_) | Phase::Final(_) | Phase::GoAround(_) | Phase::ShutDown => {
                return Err("can't taxi now".into());
            }
            _ if self.exit.as_ref().is_some_and(|x| self.pos_mdm < x.vacate_mdm) => {
                return Err("still vacating the runway: T once clear".into());
            }
            // Lined up means stopped at the entry node, not rolling down the run.
            Phase::LinedUp(_) => {
                let at = self.route.nodes[0];
                self.set_route(Route::at(at), ap, g);
            }
            _ => {}
        }
        route.check(ap)?;
        let route = self.catch_up(route)?;
        if self.on_edge() && route.edges.first() == self.route.edges.get(self.leg) {
            return Err("that is a U-turn: an aircraft can't turn back along the edge it is on".into());
        }
        self.stop = None;
        self.cross_ahead = None;
        if self.on_edge() {
            // Keep the current edge and where the aircraft is on it.
            let (n, e) = (self.route.nodes[self.leg], self.route.edges[self.leg]);
            let along = self.along_mdm;
            self.route = Route { nodes: [&[n][..], &route.nodes].concat(), edges: [&[e][..], &route.edges].concat() };
            self.leg = 0;
            self.pos_mdm = along;
            // A crossing whose hold line is already behind was cleared, or
            // the aircraft would have stopped at it: keep it cleared. Clearing
            // it here parked a crosser on 10/28 "holding short" (R1.1).
            self.cleared = crossings(ap, g, &self.route, 0, 0).into_iter().filter(|c| c.lim < self.pos_mdm).map(|c| c.first).collect();
            self.limit = self.next_hold(ap, g);
        } else {
            self.set_route(route, ap, g);
            self.start_rolling();
        }
        self.phase = if self.on_edge() { Phase::Taxi } else { Phase::Holding };
        Ok(())
    }

    /// C3 reroute: a route planned from a node the aircraft has since rolled
    /// through still applies when the aircraft got there along the route's
    /// own first edges; it is cut to start where the aircraft now continues.
    /// At 10 m/s the median IGI edge (38 m) takes 1.3 s of real time, so a
    /// preview built while moving is often one node stale by Enter. Any
    /// other start is refused: the aircraft went another way.
    fn catch_up(&self, route: Route) -> Result<Route, String> {
        let next = self.next_node();
        if route.nodes[0] == next {
            return Ok(route);
        }
        // Edges rolled into, up to and including the one ending at `next`.
        let done = if self.on_edge() { self.leg + 1 } else { self.leg };
        for k in 1..route.nodes.len() {
            if route.nodes[k] == next && k <= done && self.route.edges[done - k..done] == route.edges[..k] {
                return Ok(Route { nodes: route.nodes[k..].to_vec(), edges: route.edges[k..].to_vec() });
            }
        }
        Err(format!("route starts at node {}, aircraft continues from {}: plan again", route.nodes[0], next))
    }

    /// H: stop where you are; H again: continue. Either way it replaces any
    /// hold point or give way: a give way released later must not undo an H.
    pub fn hold(&mut self) -> Result<(), String> {
        self.phase = match self.phase {
            Phase::Taxi => Phase::HoldPosition,
            Phase::HoldPosition => Phase::Taxi,
            _ => return Err("only a taxiing aircraft can hold position".into()),
        };
        self.stop = None;
        Ok(())
    }

    /// C3, Enter: continue. Ends a hold position (H, a hold point, or a give
    /// way not yet released) and drops a hold point or give way still ahead.
    pub fn resume(&mut self) -> Result<(), String> {
        match &self.phase {
            Phase::HoldPosition => self.phase = Phase::Taxi,
            Phase::Taxi if self.stop.is_some() => {}
            Phase::HoldShort(r) => return Err(format!("holding short of {r}: cross (X) or line up (L)")),
            _ => return Err("nothing to continue: not holding".into()),
        }
        self.stop = None;
        Ok(())
    }

    /// C3: nodes a hold point can be set at. Ahead on the route, before its
    /// end (it stops there anyway), and off the runways (their hold-short is
    /// automatic).
    pub fn hold_points(&self, g: &Graph) -> Vec<u32> {
        if !matches!(self.phase, Phase::Taxi | Phase::HoldPosition) || !self.on_edge() {
            return Vec::new();
        }
        let last = self.route.nodes.len() - 1;
        self.route.nodes[self.leg + 1..last].iter().copied().filter(|&n| g.runway_at[n as usize].is_none()).collect()
    }

    /// C3, J: stop at `node`, ahead on the route, and wait for Enter (or H,
    /// or a new T). Replaces any hold point or give way already set.
    pub fn hold_at(&mut self, node: u32, g: &Graph) -> Result<(), String> {
        if !matches!(self.phase, Phase::Taxi | Phase::HoldPosition) || !self.on_edge() {
            return Err("only a taxiing aircraft can hold at a point".into());
        }
        let last = self.route.nodes.len() - 1;
        let Some(k) = (self.leg + 1..=last).find(|&k| self.route.nodes[k] == node) else {
            return Err(format!("node {node} is not ahead on the route"));
        };
        if k == last {
            return Err("the route ends there: it stops there anyway".into());
        }
        if g.runway_at[node as usize].is_some() {
            return Err("that point is on a runway: it holds short of runways anyway".into());
        }
        self.stop = Some(Stop { node, at_mdm: self.dist_to(k, g), give_way: None });
        Ok(())
    }

    /// C3: set the taxi speed. Allowed on the ground from holding to lined
    /// up, so it can be set before T or before a backtrack.
    pub fn set_taxi_speed(&mut self, s: TaxiSpeed) -> Result<(), String> {
        if !matches!(self.phase, Phase::Holding | Phase::Taxi | Phase::HoldPosition | Phase::HoldShort(_) | Phase::LineUp(_) | Phase::LinedUp(_)) {
            return Err("taxi speed is for an aircraft taxiing or about to".into());
        }
        if self.taxi_speed == s {
            return Err(format!("already taxiing at {}", s.name()));
        }
        self.taxi_speed = s;
        Ok(())
    }

    /// C3, X before the hold line: cross the next runway without stopping if
    /// it is clear when the aircraft gets there. The world judges that on
    /// arrival (`World::ground_tick`); if busy, the aircraft holds short as
    /// always and the clearance lapses, so X is needed again.
    pub fn cross_ahead(&mut self, ap: &Airport, g: &Graph) -> Result<(), String> {
        if !matches!(self.phase, Phase::Taxi | Phase::HoldPosition) {
            return Err("not holding short of a runway".into());
        }
        let c = self.held_crossing(ap, g).ok_or("no runway ahead")?;
        if c.last == self.route.nodes.len() - 1 {
            return Err(format!("the route ends on {}: line up (L) or taxi elsewhere", c.name));
        }
        if self.cross_ahead.as_ref().is_some_and(|(f, _)| *f == c.first) {
            return Err(format!("already cleared to cross {} if clear", c.name));
        }
        self.cross_ahead = Some((c.first, c.name));
        Ok(())
    }

    /// The crossing this hold-short guards: the next one not yet cleared.
    pub(crate) fn held_crossing(&self, ap: &Airport, g: &Graph) -> Option<Crossing> {
        crossings(ap, g, &self.route, 0, 0).into_iter().find(|c| c.last > self.leg && !self.cleared.contains(&c.first))
    }

    /// X: cross the runway being held short of; the route continues past it.
    pub fn cross(&mut self, ap: &Airport, g: &Graph) -> Result<(), String> {
        let Phase::HoldShort(rwy) = &self.phase else { return Err("not holding short of a runway".into()) };
        let c = self.held_crossing(ap, g).ok_or("no runway ahead")?;
        if c.last == self.route.nodes.len() - 1 {
            return Err(format!("the route ends on {rwy}: line up (L) or taxi elsewhere"));
        }
        self.cleared.push(c.first);
        self.limit = self.next_hold(ap, g);
        self.phase = Phase::Taxi;
        Ok(())
    }

    /// L: line up. Held short where the route ends on the runway, or
    /// stopped at a holding point beside one. Checks everything before
    /// changing anything, so a refusal leaves the aircraft as it was.
    pub fn line_up(&mut self, ap: &Airport, g: &Graph) -> Result<(), String> {
        let (rwy, entry, approach) = self.line_up_target(ap, g)?;
        let (run, d) = g.runway_run_facing(ap, entry, self.assigned).ok_or("no runway here")?;
        let run_m = g.run_len_dm(&run) / 10;
        let need = min_takeoff_run_m(self.wake);
        if run_m < need {
            return Err(format!("only {run_m} m of {d} ahead from here; need {need}"));
        }
        self.roll_to_entry(approach, ap, g);
        if self.on_edge() {
            self.phase = Phase::LineUp(rwy);
        } else {
            // Already at the entry node: line up at once.
            self.line_up_on(run, ap, g);
            self.phase = Phase::LinedUp(d);
        }
        Ok(())
    }

    /// Where L (and B) would take the aircraft onto a runway: the runway's
    /// name, the entry node, and the short route to it from a holding point.
    fn line_up_target(&self, ap: &Airport, g: &Graph) -> Result<(String, u32, Option<Route>), String> {
        // A vacated arrival holds within reach of the runway it left, which
        // B3's holding-point line-up would accept (found by verify_b5).
        if self.exit.is_some() {
            return Err("an arrival taxis to a stand (T)".into());
        }
        match &self.phase {
            Phase::HoldShort(rwy) => {
                let c = self.held_crossing(ap, g).ok_or("no runway ahead")?;
                if c.last != self.route.nodes.len() - 1 {
                    return Err(format!("the route crosses {rwy}: cross (X) instead"));
                }
                Ok((rwy.clone(), self.route.end(), None))
            }
            Phase::Holding => {
                let r = g.nearest_runway(self.route.end(), HOLD_SEARCH_M * 10).ok_or("no runway within reach")?;
                let entry = r.end();
                let rwy = ap.runways[g.runway_at[entry as usize].expect("nearest_runway ends on one")].name.clone();
                Ok((rwy, entry, Some(r)))
            }
            _ => Err("hold short of a runway first".into()),
        }
    }

    /// Take `approach` (if any) and clear the crossing the route ends on.
    fn roll_to_entry(&mut self, approach: Option<Route>, ap: &Airport, g: &Graph) {
        if let Some(r) = approach {
            self.set_route(r, ap, g);
            self.start_rolling();
        }
        // The route now ends on the entry: clear the crossing that ends there.
        let end = self.route.nodes.len() - 1;
        if let Some(c) = crossings(ap, g, &self.route, 0, 0).into_iter().find(|c| c.last == end) {
            self.cleared.push(c.first);
        }
        self.limit = self.next_hold(ap, g);
    }

    /// C3, B: backtrack. Enter the runway, taxi along it against the
    /// departure direction to its end, turn there and line up facing the
    /// whole length. Allowed where L is (held short where the route ends on
    /// the runway, or at a holding point) and when already lined up. The
    /// departure direction is the one L would face: the assigned runway if
    /// this is it, else the longer way. Refused at the runway's end, and when
    /// even the whole length is too short for the type.
    pub fn backtrack(&mut self, ap: &Airport, g: &Graph) -> Result<(), String> {
        let (entry, d, ahead, approach) = match &self.phase {
            Phase::LinedUp(d) => (self.route.nodes[0], d.clone(), self.route.clone(), None),
            _ => {
                let (_, entry, approach) = self.line_up_target(ap, g)?;
                let (run, d) = g.runway_run_facing(ap, entry, self.assigned).ok_or("no runway here")?;
                (entry, d, run, Some(approach))
            }
        };
        let back = backtrack_run(ap, g, entry, &d).ok_or(format!("already at the end of {d}: line up (L)"))?;
        let full_m = (g.run_len_dm(&back) + g.run_len_dm(&ahead)) / 10;
        let need = min_takeoff_run_m(self.wake);
        if full_m < need {
            return Err(format!("{d} is {full_m} m end to end; need {need}"));
        }
        match approach {
            None => self.start_backtrack(back, d, ap, g),
            Some(approach) => {
                self.roll_to_entry(approach, ap, g);
                if self.on_edge() {
                    let rwy = ap.runways[g.runway_at[entry as usize].expect("an entry")].name.clone();
                    self.phase = Phase::LineUp(rwy);
                    self.backtrack = Some(d);
                } else {
                    self.start_backtrack(back, d, ap, g);
                }
            }
        }
        Ok(())
    }

    /// K: cleared for takeoff.
    pub fn takeoff(&mut self) -> Result<(), String> {
        let Phase::LinedUp(d) = &self.phase else { return Err("line up first".into()) };
        self.phase = Phase::TakeoffRoll(d.clone());
        self.start_rolling();
        Ok(())
    }

    /// Keys whose verb would succeed now, for the strip's hints. Built by
    /// asking each verb on a copy, so a hint can never promise a refusal.
    /// One copy without the trail, which only grows and no verb reads, and a
    /// fresh one only after a verb succeeds: every verb refuses before it
    /// changes anything. It cloned the whole aircraft 7 times a call (R1.20).
    pub fn verbs(&self, ap: &Airport, g: &Graph) -> Vec<char> {
        let mut out = Vec::new();
        let mut s = self.scratch();
        let mut ok = |f: &dyn Fn(&mut Aircraft) -> Result<(), String>| {
            let yes = f(&mut s).is_ok();
            if yes {
                s = self.scratch();
            }
            yes
        };
        if ok(&|a| a.push(ap, g)) {
            out.push('P');
        }
        let vacating = self.exit.as_ref().is_some_and(|x| self.pos_mdm < x.vacate_mdm);
        if !vacating && !matches!(self.phase, Phase::OnStand | Phase::Pushback | Phase::LineUp(_) | Phase::TakeoffRoll(_) | Phase::Airborne(_) | Phase::Departed(_) | Phase::Landing(_) | Phase::Final(_) | Phase::GoAround(_) | Phase::ShutDown) {
            out.push('T');
        }
        if ok(&|a| a.hold()) {
            out.push('H');
        }
        if ok(&|a| a.cross(ap, g)) {
            out.push('X');
        }
        if ok(&|a| a.line_up(ap, g)) {
            out.push('L');
        }
        if ok(&|a| a.takeoff()) {
            out.push('K');
        }
        if ok(&|a| a.clear_to_land()) {
            out.push('C');
        }
        if ok(&|a| a.go_around()) {
            out.push('G');
        }
        // E only when another exit would be accepted (R1.24): past the
        // braking point of every later one, E led only to refusals.
        if let (Phase::Final(d) | Phase::Landing(d), Some(cur)) = (&self.phase, &self.exit)
            && arrival::exits(ap, g, d).iter().any(|(x, _)| x.node != cur.node && self.exit_room(x).is_ok())
        {
            out.push('E');
        }
        out
    }

    /// A copy to try verbs on: every field but the trail. Listed in full,
    /// so a new field fails to compile here rather than go missing.
    pub(crate) fn scratch(&self) -> Aircraft {
        let Aircraft { callsign, wake, type_code, assigned, ready, phase, route, leg, along_mdm, pos_mdm, speed_mdm, limit, cleared, trail: _, exit, air_mdm, cleared_to_land, pilot_ga, weather_ga, decide_mdm, went_around, circuit, arrival, stand_since, push_to, stop, taxi_speed, cross_ahead, backtrack } = self;
        Aircraft {
            callsign: callsign.clone(),
            wake: *wake,
            type_code,
            assigned: *assigned,
            ready: *ready,
            phase: phase.clone(),
            route: route.clone(),
            leg: *leg,
            along_mdm: *along_mdm,
            pos_mdm: *pos_mdm,
            speed_mdm: *speed_mdm,
            limit: limit.clone(),
            cleared: cleared.clone(),
            trail: Vec::new(),
            exit: exit.clone(),
            air_mdm: *air_mdm,
            cleared_to_land: *cleared_to_land,
            pilot_ga: *pilot_ga,
            weather_ga: *weather_ga,
            decide_mdm: *decide_mdm,
            went_around: *went_around,
            circuit: *circuit,
            arrival: *arrival,
            stand_since: *stand_since,
            push_to: *push_to,
            stop: stop.clone(),
            taxi_speed: *taxi_speed,
            cross_ahead: cross_ahead.clone(),
            backtrack: backtrack.clone(),
        }
    }

    /// C: cleared to land. Only on final; never refused for separation.
    pub fn clear_to_land(&mut self) -> Result<(), String> {
        if !matches!(self.phase, Phase::Final(_)) {
            return Err("only an aircraft on final can be cleared to land".into());
        }
        if self.cleared_to_land {
            return Err("already cleared to land".into());
        }
        self.cleared_to_land = true;
        Ok(())
    }

    /// G: go around. On the runway it is too late.
    pub fn go_around(&mut self) -> Result<(), String> {
        match self.phase {
            Phase::Final(_) => {
                self.go_around_now(GoAround::Instructed);
                Ok(())
            }
            Phase::Landing(_) => Err("on the runway: too late to go around".into()),
            _ => Err("only an aircraft on final can go around".into()),
        }
    }

    /// E: leave the runway at the exit from runway node `node`. Refused,
    /// with the reason, if it is behind, points back up the runway, or
    /// leaves too little room to slow down.
    pub fn set_exit(&mut self, node: u32, ap: &Airport, g: &Graph) -> Result<(), String> {
        let d = match &self.phase {
            Phase::Final(d) | Phase::Landing(d) if self.exit.is_some() => d.clone(),
            _ => return Err("only a landing aircraft can be given an exit".into()),
        };
        let Some((x, route)) = arrival::exits(ap, g, &d).into_iter().find(|(x, _)| x.node == node) else {
            let reverse = arrival::candidates(ap, g, &d).into_iter().any(|c| c.node == node);
            return Err(if reverse { format!("that exit turns back against {d}") } else { format!("not an exit off {d}") });
        };
        self.exit_room(&x)?;
        // Same run from the threshold up to either exit, so the aircraft's
        // place on it (leg, along, pos) stays valid.
        self.route = route;
        self.exit = Some(x);
        Ok(())
    }

    /// Is exit `x` ahead with room to slow for it? On final the aircraft is
    /// still to touch down at the threshold.
    fn exit_room(&self, x: &Exit) -> Result<(), String> {
        let pos = self.pos_mdm;
        if x.at_mdm <= pos {
            return Err(format!("{} is behind", x.label()));
        }
        let need = brake_mdm(self.speed_mdm);
        let ahead = x.at_mdm - pos;
        if ahead < need {
            return Err(format!("{} is {} m ahead; needs {} m to slow", x.label(), ahead / MDM_PER_M, need.div_ceil(MDM_PER_M)));
        }
        Ok(())
    }
}

/// Where a route stops for each runway it meets, from node index `from_k`
/// onward: (route distance to stop at, runway name), in route order.
/// `start_mdm` is the route distance at `nodes[from_k]`.
pub fn hold_limits(ap: &Airport, g: &Graph, route: &Route, from_k: usize, start_mdm: u64) -> Vec<(u64, String)> {
    crossings(ap, g, route, from_k, start_mdm).into_iter().map(|c| (c.lim, c.name)).collect()
}

/// One crossing of a runway on a route: where to stop for it, the runway,
/// the route indices of its first and last runway nodes, and how far past
/// the last one its protected stretch runs (as far as the stop sits before
/// the first). Route distances in milli-decimetres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crossing {
    pub lim: u64,
    pub name: String,
    pub runway: usize,
    pub first: usize,
    pub last: usize,
    pub exit: u64,
}

/// Every crossing on `route` from node index `from_k` on, in route order.
/// Two meetings with one runway count as one crossing when the second's
/// stop would fall inside the first's protected stretch: there is nowhere
/// to wait between them, so one X must cover both.
pub fn crossings(ap: &Airport, g: &Graph, route: &Route, from_k: usize, start_mdm: u64) -> Vec<Crossing> {
    let mut cum = vec![0u64; route.nodes.len()];
    cum[from_k] = start_mdm;
    for k in from_k + 1..route.nodes.len() {
        cum[k] = cum[k - 1] + u64::from(g.len_dm[route.edges[k - 1] as usize]) * MDM_PER_DM;
    }
    let mut out: Vec<Crossing> = Vec::new();
    let mut prev_rwy: Option<usize> = None;
    for k in from_k + 1..route.nodes.len() {
        let Some(r) = g.runway_at[route.nodes[k] as usize] else {
            prev_rwy = None;
            continue;
        };
        // Consecutive nodes on one runway are one crossing, not two.
        if prev_rwy == Some(r) {
            let c = out.last_mut().expect("open crossing");
            c.exit += cum[k] - cum[c.last];
            c.last = k;
            continue;
        }
        prev_rwy = Some(r);
        let search = HOLD_SEARCH_M * 10 * MDM_PER_DM;
        // The node the route starts from counts too: an aircraft parked on a
        // holding point is already where it must stop.
        let osm_hold = (from_k..k).rev()
            .take_while(|&j| cum[k] - cum[j] <= search)
            .find(|&j| g.is_hold[route.nodes[j] as usize]);
        let lim = match osm_hold {
            Some(j) => cum[j],
            None => cum[k].saturating_sub(HOLD_SHORT_M * 10 * MDM_PER_DM).max(cum[from_k]),
        };
        if let Some(c) = out.last_mut().filter(|c| c.runway == r && lim <= c.exit) {
            c.exit = cum[k] + (c.exit - cum[c.last]);
            c.last = k;
            continue;
        }
        out.push(Crossing { lim, name: ap.runways[r].name.clone(), runway: r, first: k, last: k, exit: cum[k] + (cum[k] - lim) });
    }
    out
}

/// C3: the runway run from `at` against departure direction `d`: where a
/// backtrack goes. None at the runway's end, where there is nothing behind.
pub fn backtrack_run(ap: &Airport, g: &Graph, at: u32, d: &str) -> Option<Route> {
    g.runway_runs(ap, at).into_iter().find(|(_, dd)| dd != d).map(|(r, _)| r)
}

/// Point at route distance `d_mdm`, metres east and north.
pub fn point_at(ap: &Airport, g: &Graph, route: &Route, mut d_mdm: u64) -> (f64, f64) {
    let n = |i: u32| {
        let p = ap.nodes[i as usize];
        (f64::from(p.x) / 10.0, f64::from(p.y) / 10.0)
    };
    for (k, &e) in route.edges.iter().enumerate() {
        let len = u64::from(g.len_dm[e as usize]) * MDM_PER_DM;
        if d_mdm <= len {
            let (a, b) = (n(route.nodes[k]), n(route.nodes[k + 1]));
            let t = d_mdm as f64 / len as f64;
            return (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        }
        d_mdm -= len;
    }
    n(route.end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Request;
    use crate::{Rng, World};

    /// Step until `done` or `max` ticks; returns ticks used.
    fn run_until(w: &mut World, max: u32, done: impl Fn(&Aircraft) -> bool) -> u32 {
        for t in 0..max {
            if done(&w.aircraft[0]) {
                return t;
            }
            w.step_ticks(1);
        }
        panic!("not done after {max} ticks: {:?}", w.aircraft[0].phase);
    }

    fn on_runway_edge(w: &World, a: &Aircraft) -> bool {
        a.on_edge() && w.airport.edges[a.route.edges[a.leg] as usize].kind == EdgeKind::Runway
    }

    /// PLAN-twr.md § B2, kept under B3's rules: for 1,000 random
    /// stand-to-runway requests the path followed equals the path drawn, edge
    /// for edge. Every hold-short met is cleared (X to cross, L at the end),
    /// so the clearance logic runs 1,000 times too.
    #[test]
    fn followed_path_equals_drawn_path_1000_times() {
        let mut w = World::new(1);
        let exits = w.airport.runway_exits();
        let mut rng = Rng::new(99);
        let (mut lined_up, mut short_run, mut crossings) = (0, 0, 0);
        for n in 0..1000 {
            let stand = w.airport.stands[(rng.next_u64() % w.airport.stands.len() as u64) as usize].node;
            let to = exits[(rng.next_u64() % exits.len() as u64) as usize];
            w.aircraft[0] = Aircraft { phase: Phase::Holding, ..Aircraft::parked("T", stand) };
            let drawn = w.plan(0, to, &[], &[]).expect("route");
            w.taxi(0, drawn.clone()).unwrap();
            loop {
                run_until(&mut w, 2_000_000, |a| !matches!(a.phase, Phase::Taxi | Phase::LineUp(_)));
                // Lined up stands on the runway by design; before that, never.
                let a = &w.aircraft[0];
                assert!(matches!(a.phase, Phase::LinedUp(_)) || !on_runway_edge(&w, a), "request {n}: on a runway edge in {:?}", a.phase);
                match w.aircraft[0].phase.clone() {
                    Phase::HoldShort(_) if w.cross(0).is_ok() => crossings += 1,
                    Phase::HoldShort(_) => {
                        if w.line_up(0).is_err() {
                            short_run += 1;
                            break;
                        }
                    }
                    Phase::LinedUp(_) => {
                        lined_up += 1;
                        break;
                    }
                    Phase::Holding => break,
                    p => panic!("request {n}: stopped in {p:?}"),
                }
            }
            let trail = &w.aircraft[0].trail;
            if matches!(w.aircraft[0].phase, Phase::LinedUp(_)) {
                assert_eq!(trail, &drawn.edges, "request {n}");
            } else {
                assert_eq!(trail[..], drawn.edges[..trail.len()], "request {n}: not a prefix");
            }
        }
        println!("1000 requests: {lined_up} lined up, {short_run} held short with too little runway ahead, {crossings} crossings");
        assert!(lined_up > 500 && crossings > 0, "{lined_up} lined up, {crossings} crossings");
    }

    /// One whole departure, stand to departed, crossing a runway on the way.
    #[test]
    fn departure_with_a_crossing_from_stand_to_airborne() {
        let mut w = World::new(5);
        let (ap, g) = (&w.airport, &w.graph);
        // Find a stand and an entry with a long run whose route crosses
        // another runway first.
        let mut pick = None;
        'search: for s in &ap.stands {
            let pushed = ap.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s.node || e.b == s.node)).unwrap();
            let from = if pushed.a == s.node { pushed.b } else { pushed.a };
            for to in ap.runway_exits() {
                let Some((run, _)) = g.runway_run(ap, to) else { continue };
                if g.run_len_dm(&run) < 25_000 {
                    continue;
                }
                let Some(r) = g.plan(ap, &Request { from, to, ..Default::default() }) else { continue };
                let limits = hold_limits(ap, g, &r, 0, 0);
                if limits.len() >= 2 && limits[0].1 != limits[limits.len() - 1].1 {
                    pick = Some((s.node, to));
                    break 'search;
                }
            }
        }
        let (stand, entry) = pick.expect("IGI has a departure that crosses a runway");
        w.aircraft[0] = Aircraft::parked("IGO512", stand);
        assert_eq!(w.verbs(0), ['P']);
        assert_eq!(w.taxi(0, Route::at(stand)).unwrap_err(), "push back first");

        w.push(0).unwrap();
        assert_eq!(w.aircraft[0].phase, Phase::Pushback);
        run_until(&mut w, 100_000, |a| a.phase == Phase::Holding);
        assert_eq!(w.verbs(0), ['T']);

        let route = w.plan(0, entry, &[], &[]).unwrap();
        w.taxi(0, route.clone()).unwrap();
        run_until(&mut w, 1_000_000, |a| a.phase != Phase::Taxi);
        let Phase::HoldShort(first) = w.aircraft[0].phase.clone() else { panic!("{:?}", w.aircraft[0].phase) };
        assert_eq!(w.verbs(0), ['T', 'X'], "a crossing offers X, not L");
        assert!(w.line_up(0).unwrap_err().contains("cross (X) instead"));
        // Held short means the next node ahead is not yet on the runway.
        assert!(w.aircraft[0].limit.as_ref().unwrap().0 == w.aircraft[0].pos_mdm);

        w.cross(0).unwrap();
        run_until(&mut w, 1_000_000, |a| a.phase != Phase::Taxi);
        let Phase::HoldShort(dep) = w.aircraft[0].phase.clone() else { panic!("{:?}", w.aircraft[0].phase) };
        assert_ne!(first, dep);
        assert_eq!(w.verbs(0), ['T', 'L'], "the route's end offers L, not X");

        w.line_up(0).unwrap();
        run_until(&mut w, 100_000, |a| matches!(a.phase, Phase::LinedUp(_)));
        assert_eq!(w.verbs(0), ['T', 'K']);
        assert_eq!(w.aircraft[0].trail, [&w.aircraft[0].trail[..1], &route.edges[..]].concat(), "push edge, then the drawn route");

        w.takeoff(0).unwrap();
        run_until(&mut w, 100_000, |a| matches!(a.phase, Phase::Airborne(_)));
        let roll_m = w.aircraft[0].pos_mdm as f64 / 10_000.0;
        // v² / 2a = 75² / 4 = 1,406 m; integer ticks land within one tick's travel.
        assert!((roll_m - 1406.0).abs() < 4.0, "roll {roll_m:.1} m");
        assert!(w.aircraft[0].trail.iter().skip(route.edges.len() + 1).all(|&e| w.airport.edges[e as usize].name == dep));
        run_until(&mut w, 100_000, |a| matches!(a.phase, Phase::Departed(_)));
        assert!(!w.aircraft[0].visible());
        assert!(w.verbs(0).is_empty());
    }

    /// An OSM holding point on the route is where the aircraft stops; with
    /// none, it stops HOLD_SHORT_M before the runway node.
    #[test]
    fn hold_short_uses_the_osm_holding_point_when_there_is_one() {
        let w = World::new(1);
        let (ap, g) = (&w.airport, &w.graph);
        let mut checked = (0, 0);
        for h in &ap.holds {
            let Some(tail) = g.nearest_runway(h.node, HOLD_SEARCH_M * 10) else { continue };
            if tail.edges.is_empty() {
                continue;
            }
            let lims = hold_limits(ap, g, &tail, 0, 0);
            assert_eq!(lims[0].0, 0, "hold node {}: stop at the hold itself", h.node);
            checked.0 += 1;
            // Same tail without its first node: no hold on it any more, so
            // the default distance applies when the tail is long enough.
            let cut = Route { nodes: tail.nodes[1..].to_vec(), edges: tail.edges[1..].to_vec() };
            let len: u64 = cut.edges.iter().map(|&e| u64::from(g.len_dm[e as usize]) * MDM_PER_DM).sum();
            let cut_lims = hold_limits(ap, g, &cut, 0, 0);
            if !cut.edges.is_empty() && !cut.nodes[..cut.nodes.len() - 1].iter().any(|&n| g.is_hold[n as usize]) {
                assert_eq!(cut_lims[0].0, len.saturating_sub(HOLD_SHORT_M * 10 * MDM_PER_DM));
                checked.1 += 1;
            }
        }
        assert!(checked.0 > 20 && checked.1 > 0, "{checked:?}");
    }

    #[test]
    fn line_up_refuses_too_little_runway_and_changes_nothing() {
        let w = World::new(1);
        let (ap, g) = (&w.airport, &w.graph);
        let short = ap.runway_exits().into_iter()
            .find(|&n| g.runway_run(ap, n).is_some_and(|(r, _)| g.run_len_dm(&r) / 10 < MIN_TAKEOFF_RUN_M))
            .expect("IGI has an exit with under 2,030 m ahead");
        // Parked right at that entry, as if taxied there.
        let mut a = Aircraft { phase: Phase::Holding, ..Aircraft::parked("T", short) };
        let before = format!("{a:?}");
        assert!(a.line_up(ap, g).unwrap_err().contains("need 2030"));
        assert_eq!(format!("{a:?}"), before);
    }

    /// Re-cleared while rolling: the aircraft keeps its current edge, then
    /// follows the new route exactly.
    #[test]
    fn reclearing_mid_edge_keeps_the_current_edge() {
        let mut w = World::new(3);
        w.aircraft[0].phase = Phase::Holding;
        let stands: Vec<u32> = w.airport.stands.iter().map(|s| s.node).collect();
        let first = w.plan(0, stands[10], &[], &[]).unwrap();
        w.taxi(0, first.clone()).unwrap();
        w.step_ticks(3);
        assert!(w.aircraft[0].on_edge() && w.aircraft[0].along_mdm > 0);
        let second = w.plan(0, stands[60], &[], &[]).unwrap();
        assert_eq!(second.nodes[0], first.nodes[1]);
        w.taxi(0, second.clone()).unwrap();
        run_until(&mut w, 2_000_000, |a| a.phase != Phase::Taxi);
        // The second route may stop short of a runway it crosses: compare
        // as far as the aircraft got.
        let want: Vec<u32> = [&first.edges[..1], &second.edges[..]].concat();
        let n = w.aircraft[0].trail.len().min(want.len());
        assert!(n > 1);
        assert_eq!(w.aircraft[0].trail[..n], want[..n]);
    }
}
