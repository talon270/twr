//! twr-sim — the deterministic simulation core.
//!
//! No web dependencies. Everything here runs under `cargo test` with no
//! browser, which is how routing, conflicts and pace get verified by running
//! them rather than by reading them.
//!
//! Determinism is a contract: the same seed and the same command log replay
//! the same shift, tick for tick. Nothing in this crate may read wall-clock
//! time, use floating-point accumulation across frames, or draw randomness
//! from anywhere but `Rng`.
//!
//! B0: clock and RNG. B1: the airport graph (`airport`). B2: the planner
//! (`route`). B3: the departure flow (`aircraft`). B4: the conflict model
//! (`conflict`) and named set-ups (`scenario`). B5: arrivals (`arrival`).
//! B6: traffic (`traffic`). B7: the score and the end of a shift (`score`).

pub mod aircraft;
pub mod airport;
pub mod arrival;
pub mod conflict;
pub mod lvp;
pub mod radio;
pub mod route;
pub mod scenario;
pub mod shape;
pub mod score;
pub mod traffic;
pub mod weather;

pub use aircraft::{Aircraft, Phase, TaxiSpeed, Wake};
pub use arrival::GoAround;
pub use conflict::{Incident, Monitor, Rule};
pub use score::Score;
pub use traffic::{Mode, Traffic};
use airport::Airport;
use route::{Graph, Request, Route};

/// Sim ticks per second of game time. At 20 Hz an aircraft taxiing at 30 kt
/// moves 0.77 m per tick, well under any spacing rule we will check.
pub const TICK_HZ: u32 = 20;

/// Game time per real time, in thousandths, applied to movement only.
/// 3000 = 3x. Traffic rates are counted per real hour (PLAN-twr.md § A2), so
/// this shortens taxi time without shortening the gap between decisions.
/// An integer because the clock below must not accumulate float error.
pub const TIME_COMPRESSION_MILLI: u64 = 3000;

/// Real time a single frame may contribute, in microseconds. A tab returning
/// from the background hands us seconds of elapsed time; replaying all of it
/// at once would teleport aircraft through each other and past every conflict
/// check. Anything above this is dropped, which reads as a brief stall.
pub const MAX_FRAME_REAL_US: u64 = 250_000;

/// Length of one tick in game nanoseconds: 50 ms at 20 Hz, exact.
const TICK_GAME_NS: u64 = 1_000_000_000 / TICK_HZ as u64;

/// Fixed-step clock. Real frame time goes in, whole ticks come out, and the
/// remainder carries to the next frame so no time is lost or invented.
///
/// The carry is integer game nanoseconds. The first version carried float
/// milliseconds and turned 1000 ms at 3x into 59 ticks instead of 60, because
/// 1000/60 has no exact float form; integers make that impossible.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    ticks: u64,
    carry_game_ns: u64,
}

impl Clock {
    /// Feed one frame of real elapsed time in milliseconds (what
    /// requestAnimationFrame gives); returns how many ticks to step. The
    /// input is rounded to whole microseconds, the only float step here.
    pub fn advance(&mut self, real_ms: f64) -> u32 {
        let real_us = if real_ms.is_finite() && real_ms > 0.0 {
            ((real_ms * 1000.0).round() as u64).min(MAX_FRAME_REAL_US)
        } else {
            0
        };
        // real_us * (milli-compression) = game time in nanoseconds.
        self.carry_game_ns += real_us * TIME_COMPRESSION_MILLI;
        let steps = self.carry_game_ns / TICK_GAME_NS;
        self.carry_game_ns %= TICK_GAME_NS;
        self.ticks += steps;
        steps as u32
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Game time since the shift started, from the integer tick count, so it
    /// never drifts from the sim.
    pub fn game_seconds(&self) -> f64 {
        self.ticks as f64 / f64::from(TICK_HZ)
    }
}

/// SplitMix64. Small, fast, and identical on every platform, which is all a
/// replayable game needs; nothing here is security-sensitive.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1), from the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The page's one arrival lands here until the `vidp` research says which
/// runway IGI lands on in each flow (PLAN-twr.md § B5). `authored`.
pub const PAGE_ARRIVAL_RWY: &str = "29L";

/// One go-around, for the strip and, in B7, the score: challenges are
/// counted apart from delay you caused.
#[derive(Debug, Clone, PartialEq)]
pub struct GoAroundLog {
    pub tick: u64,
    pub callsign: String,
    pub why: GoAround,
}

/// IGI's route graph, baked by `cargo run -p twr-osm -- VIDP`. BLR and BOM
/// join in B8.
pub const VIDP: &str = include_str!("../../../data/vidp.twr");

/// The whole simulation state. Everything the screen shows is read from here.
pub struct World {
    pub seed: u64,
    pub clock: Clock,
    rng: Rng,
    pub airport: Airport,
    pub graph: Graph,
    pub aircraft: Vec<Aircraft>,
    /// Ticks stepped since the shift began. Wake timers and the incident
    /// log run on this, so tests (which skip the clock) and the page agree.
    pub tick: u64,
    pub monitor: Monitor,
    pub go_arounds: Vec<GoAroundLog>,
    /// B6: the traffic stream; None in Practice and in tests' worlds.
    pub traffic: Option<Traffic>,
    /// B7: movements, delay, and the tick the shift ended.
    pub score: Score,
    /// The world stops on the tick the shift ends (B7). Long-run tests turn
    /// this off to watch the controller finish what the shift gave it.
    pub halt_on_end: bool,
}

impl World {
    /// Practice: IGO512 on a seeded stand and IGO713 on final for 29L, the
    /// B5 world every verify script from B2 on opens.
    pub fn new(seed: u64) -> Self {
        let mut w = World::empty(seed);
        let stand = w.airport.stands[(w.rng.next_u64() % w.airport.stands.len() as u64) as usize].node;
        w.aircraft.push(Aircraft::parked("IGO512", stand));
        // And one arrival on final, drawn after the stand so B0-B4 seeds
        // still pick the same stand.
        w.spawn_arrival("IGO713", Wake::M, PAGE_ARRIVAL_RWY).expect("29L has a default exit; arrival tests guard this");
        w
    }

    /// IGI with no aircraft and no traffic.
    pub fn empty(seed: u64) -> Self {
        let airport = Airport::parse(VIDP).expect("baked VIDP parses; airport tests guard this");
        let graph = Graph::new(&airport);
        let monitor = Monitor::new(&airport);
        World { seed, clock: Clock::default(), rng: Rng::new(seed), airport, graph, aircraft: Vec::new(), tick: 0, monitor, go_arounds: Vec::new(), traffic: None, score: Score::default(), halt_on_end: true }
    }

    /// Put an arrival on final for `desig`, with its pilot go-around drawn.
    pub fn spawn_arrival(&mut self, callsign: &str, wake: Wake, desig: &str) -> Option<usize> {
        let a = Aircraft::on_final(callsign, wake, desig, &self.airport, &self.graph)?;
        let i = self.add(a);
        self.draw_pilot_ga(i);
        Some(i)
    }

    /// V1: under LVP the landing clearance is due by 2 NM, not 1,160 m.
    fn decide_mdm(&self) -> u64 {
        (if self.monitor.lvp { lvp::LVP_DECISION_M } else { arrival::DECISION_M }) * aircraft::MDM_PER_M
    }

    /// Switch LVP on or off: the Practice toggle (PLAN-lvp.md, question 1).
    /// Arrivals already on final take the new deadline at once, so one
    /// inside 2 NM without C goes around on the next tick, as the rule says.
    pub fn set_lvp(&mut self, on: bool) {
        self.monitor.lvp = on;
        let decide = self.decide_mdm();
        for a in self.aircraft.iter_mut().filter(|a| matches!(a.phase, Phase::Final(_))) {
            a.decide_mdm = decide;
        }
    }

    /// Each time an aircraft comes onto final: does its pilot go around,
    /// where, and why. Three draws from the one stream, every time, so the
    /// stream does not depend on the outcome.
    fn draw_pilot_ga(&mut self, i: usize) {
        self.aircraft[i].decide_mdm = self.decide_mdm();
        let (p, at, why) = (self.rng.next_u64() % 100, self.rng.next_u64(), self.rng.next_u64());
        let span = arrival::PILOT_GA_MAX_M * aircraft::MDM_PER_M - aircraft::LAND_MDM_PER_TICK;
        self.aircraft[i].pilot_ga = (p < arrival::PILOT_GA_PCT).then(|| {
            (aircraft::LAND_MDM_PER_TICK + at % span, arrival::PILOT_REASONS[(why % arrival::PILOT_REASONS.len() as u64) as usize])
        });
        // B9 windshear: drawn on gusty shifts only, so a steady shift's
        // stream is the one B5-B7 tested, draw for draw.
        self.aircraft[i].weather_ga = None;
        if self.wind().gust.is_some() {
            let (p, at) = (self.rng.next_u64() % 100, self.rng.next_u64());
            if self.aircraft[i].pilot_ga.is_none() && p < weather::WINDSHEAR_GA_PCT {
                self.aircraft[i].weather_ga = Some(aircraft::LAND_MDM_PER_TICK + at % span);
            }
        }
    }

    /// Advance by one frame of real time. Returns the ticks stepped.
    pub fn frame(&mut self, real_ms: f64) -> u32 {
        // An ended shift stops its clock too, or the game clock on screen
        // would run on over a frozen map.
        if self.halt_on_end && self.score.over.is_some() {
            return 0;
        }
        let steps = self.clock.advance(real_ms);
        self.step_ticks(steps);
        steps
    }

    /// Step whole ticks with no clock involved; the frame loop and tests
    /// share this, so a test that fast-forwards runs the same movement code.
    /// After every tick the conflict monitor checks the new positions, and
    /// each lift-off gets its wake check on the tick it happens.
    pub fn step_ticks(&mut self, n: u32) {
        for _ in 0..n {
            // A score that changes after "Shift over" is not a score.
            if self.halt_on_end && self.score.over.is_some() {
                return;
            }
            self.tick += 1;
            let (mut landed, mut around) = (Vec::new(), Vec::new());
            for (i, a) in self.aircraft.iter_mut().enumerate() {
                let was = matches!(a.phase, Phase::Airborne(_));
                let was_final = matches!(a.phase, Phase::Final(_));
                a.step(&self.airport, &self.graph);
                if !was && matches!(a.phase, Phase::Airborne(_)) {
                    self.score.movements += 1;
                    let at = a.pos_m(&self.airport, &self.graph);
                    self.monitor.on_airborne(&a.callsign, self.tick, at);
                }
                match (&a.phase, was_final) {
                    (Phase::Landing(_), true) => landed.push(i),
                    (Phase::GoAround(_), true) => around.push(i),
                    _ => {}
                }
            }
            for i in around {
                // A3: a clearance withheld because the LSA was busy was the
                // rule. The aircraft only sees itself, so the world decides.
                if self.aircraft[i].went_around == Some(GoAround::NoClearance) && self.lsa_blocker(i).is_some() {
                    self.aircraft[i].went_around = Some(GoAround::LsaOccupied);
                }
                let a = &self.aircraft[i];
                let why = a.went_around.expect("a go-around has a cause");
                self.go_arounds.push(GoAroundLog { tick: self.tick, callsign: a.callsign.clone(), why });
            }
            // R5 before the per-tick check, so the roll that follows is
            // not counted a second time for the same pair.
            self.score.movements += landed.len() as u32;
            for i in landed {
                self.monitor.on_threshold(&self.aircraft, i, &self.airport, &self.graph, self.tick);
            }
            self.ground_tick();
            self.monitor.check(&self.aircraft, &self.airport, &self.graph, self.tick);
            self.shut_down_arrivals();
            self.rejoin_go_arounds();
            self.traffic_tick();
            self.score_tick();
        }
    }

    /// Go-arounds whose circuit is done rejoin final only into a free slot
    /// (A12), judged one at a time so two cannot take the same one. It runs
    /// after movement and before the traffic queue: run before movement, it
    /// saw every slot only after the queue had taken it, and GA1 waited
    /// 20,018 ticks behind a standing 29L queue (R1.3). A rejoin does not
    /// move on the tick it rejoins.
    fn rejoin_go_arounds(&mut self) {
        for i in 0..self.aircraft.len() {
            let a = &self.aircraft[i];
            let Phase::GoAround(d) = &a.phase else { continue };
            if a.circuit == 0 && !a.visible() && self.final_free(d, a.wake) {
                let d = d.clone();
                self.aircraft[i].back_on_final(&d);
                self.draw_pilot_ga(i);
            }
        }
    }

    /// An arrival on a stand shuts down and leaves after ON_STAND_REAL_S. It
    /// runs in every world, Practice included: the timer is the aircraft's
    /// own, not the traffic stream's.
    fn shut_down_arrivals(&mut self) {
        let (now, wait) = (self.tick, traffic::ON_STAND_REAL_S * traffic::TICKS_PER_REAL_S);
        for a in self.aircraft.iter_mut().filter(|a| a.arrival && a.phase == Phase::OnStand) {
            let at = *a.stand_since.get_or_insert(now);
            if now - at >= wait {
                a.phase = Phase::ShutDown;
            }
        }
    }

    /// Plan a route for aircraft `i` from where a new clearance must start.
    /// The screen draws the result; `taxi` takes it back unchanged.
    pub fn plan(&self, i: usize, to: u32, via: &[u32], avoid: &[u32]) -> Option<Route> {
        let from = self.start_node(i);
        let mut avoid = avoid.to_vec();
        // Mid-edge, the aircraft can't turn back along it (Aircraft::taxi refuses).
        let a = &self.aircraft[i];
        if a.on_edge() && !matches!(a.phase, Phase::LinedUp(_)) {
            avoid.push(a.route.edges[a.leg]);
        }
        // V4: under LVP the preview prefers the flow's published routes.
        let off_lvp = if self.monitor.lvp { lvp::off_route_mask(&self.airport, self.flow()) } else { Vec::new() };
        let req = Request { from, to, via: via.to_vec(), avoid, off_lvp };
        self.graph.plan(&self.airport, &req)
    }

    /// Where a taxi route for aircraft `i` begins. A lined-up aircraft
    /// leaves from its runway entry, not from the run ahead of it.
    pub fn start_node(&self, i: usize) -> u32 {
        let a = &self.aircraft[i];
        match a.phase {
            Phase::LinedUp(_) => a.route.nodes[0],
            _ => a.next_node(),
        }
    }

    /// Where a planned route would stop short of runways, metres, assuming
    /// no clearance yet: the bars the preview draws.
    pub fn hold_marks(&self, route: &Route) -> Vec<(f64, f64)> {
        aircraft::hold_limits(&self.airport, &self.graph, route, 0, 0)
            .into_iter()
            .map(|(d, _)| aircraft::point_at(&self.airport, &self.graph, route, d))
            .collect()
    }

    pub fn push(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].push(&self.airport, &self.graph)
    }

    /// C2: the ways aircraft `i` can push back, each with who is in the way.
    pub fn tail_options(&self, i: usize) -> Vec<(aircraft::TailOption, Option<String>)> {
        self.aircraft[i]
            .tail_options(&self.airport, &self.graph)
            .into_iter()
            .map(|o| {
                let by = self.tail_blocker(i, &o);
                (o, by)
            })
            .collect()
    }

    /// C2: another aircraft too close to where `opt`'s push would take
    /// aircraft `i`: its name and the gap, as the inline refusal words it.
    /// Too close is the two bodies overlapping: half of each length, summed.
    fn tail_blocker(&self, i: usize, opt: &aircraft::TailOption) -> Option<String> {
        let (ap, g) = (&self.airport, &self.graph);
        let half = |a: &Aircraft| shape::dims(a.type_code).0 / 2.0;
        let me = half(&self.aircraft[i]);
        let link = u64::from(g.len_dm[opt.route.edges[0] as usize]) * 1000;
        let mut d = link;
        loop {
            let d_here = d.min(opt.push_mdm);
            let p = aircraft::point_at(ap, g, &opt.route, d_here);
            for (j, o) in self.aircraft.iter().enumerate().filter(|(j, o)| *j != i && o.visible()) {
                let q = o.pos_m(ap, g);
                let gap = (p.0 - q.0).hypot(p.1 - q.1);
                if gap < me + half(o) {
                    return Some(format!("{} is {:.0} m from the tail", self.aircraft[j].callsign, gap));
                }
            }
            if d_here == opt.push_mdm {
                return None;
            }
            d += 50_000;
        }
    }

    /// C2: push aircraft `i` back along its `k`th tail option.
    pub fn push_tail(&mut self, i: usize, k: usize) -> Result<(), String> {
        let opts = self.tail_options(i);
        let (opt, by) = opts.get(k).ok_or("no such tail direction")?;
        if let Some(by) = by {
            return Err(format!("{}: {by}", opt.label));
        }
        self.aircraft[i].push_tail(opt, &self.airport, &self.graph)
    }

    /// Clear aircraft `i` along `route`, exactly as given. Nothing re-plans
    /// it. A stand another aircraft is on, or bound for, is refused, and so
    /// is one whose code letter is below the type's (B10, A2).
    pub fn taxi(&mut self, i: usize, route: Route) -> Result<(), String> {
        let end = route.end();
        if let Some(st) = self.airport.stands.iter().find(|s| s.node == end) {
            if let Some(j) = self.stand_taken(end, Some(i)) {
                return Err(format!("stand {} occupied by {}", st.name, self.aircraft[j].callsign));
            }
            let ty = self.aircraft[i].type_code;
            if !traffic::stand_fits(st.code, ty) {
                return Err(format!("stand {} takes code {}; {ty} is {}", st.name, st.code, traffic::type_letter(ty).unwrap_or('?')));
            }
        }
        self.aircraft[i].taxi(route, &self.airport, &self.graph)
    }

    /// C. Never refused for separation; R5 logs a threshold crossed with
    /// the runway occupied.
    pub fn clear_to_land(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].clear_to_land()
    }

    /// G, logged as instructed: delay you caused, not a challenge.
    pub fn go_around(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].go_around()?;
        self.go_arounds.push(GoAroundLog { tick: self.tick, callsign: self.aircraft[i].callsign.clone(), why: GoAround::Instructed });
        Ok(())
    }

    /// E: exit at the one leaving runway node `node`.
    pub fn set_exit(&mut self, i: usize, node: u32) -> Result<(), String> {
        self.aircraft[i].set_exit(node, &self.airport, &self.graph)
    }

    pub fn hold(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].hold()
    }

    /// X. Held short: cross now. Still taxiing toward the hold line (C3):
    /// cross without stopping if the runway is clear on arrival.
    pub fn cross(&mut self, i: usize) -> Result<(), String> {
        let a = &mut self.aircraft[i];
        if matches!(a.phase, Phase::HoldShort(_)) {
            a.cross(&self.airport, &self.graph)
        } else {
            a.cross_ahead(&self.airport, &self.graph)
        }
    }

    /// C3, Enter: continue after H, a hold point, or a give way.
    pub fn resume(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].resume()
    }

    /// C3, J: hold at `node`, ahead on the route.
    pub fn hold_at(&mut self, i: usize, node: u32) -> Result<(), String> {
        self.aircraft[i].hold_at(node, &self.graph)
    }

    /// C3: slow, normal or expedite.
    pub fn set_taxi_speed(&mut self, i: usize, s: TaxiSpeed) -> Result<(), String> {
        self.aircraft[i].set_taxi_speed(s)
    }

    /// C3, B: backtrack along the runway and line up at its end.
    pub fn backtrack(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].backtrack(&self.airport, &self.graph)
    }

    /// C3, Y: aircraft `i` gives way to aircraft `j`. The junction is the
    /// first node ahead on `i`'s route that is also ahead on `j`'s; `i`
    /// stops GIVE_WAY_GAP_M short of it and goes on by itself once `j` is
    /// GROUND_MIN_M past it (`ground_tick`). Refused, with the reason, when
    /// the routes do not meet, when `j` would come through `i` to get
    /// there (giving way to a follower stops in its path), and when `i` is
    /// already inside the gap: there, H is the instruction.
    pub fn give_way(&mut self, i: usize, j: usize) -> Result<(), String> {
        let (a, o) = (&self.aircraft[i], &self.aircraft[j]);
        if i == j {
            return Err("an aircraft can't give way to itself".into());
        }
        if !matches!(a.phase, Phase::Taxi | Phase::HoldPosition) || !a.on_edge() {
            return Err("only a taxiing aircraft can give way".into());
        }
        if !o.visible() || !o.on_edge() || matches!(o.phase, Phase::Final(_) | Phase::GoAround(_) | Phase::TakeoffRoll(_) | Phase::Airborne(_)) {
            return Err(format!("{} is not moving on the ground", o.callsign));
        }
        let o_ahead = &o.route.nodes[o.leg + 1..];
        let Some(k) = (a.leg + 1..a.route.nodes.len()).find(|&k| o_ahead.contains(&a.route.nodes[k])) else {
            return Err(format!("{}'s route does not meet {}'s", o.callsign, a.callsign));
        };
        let node = a.route.nodes[k];
        let ko = o.leg + 1 + o_ahead.iter().position(|&n| n == node).expect("found above");
        if o.route.edges[o.leg..ko].contains(&a.route.edges[a.leg]) {
            return Err(format!("{} comes through {} to get there: hold (H) instead", o.callsign, a.callsign));
        }
        let gap = aircraft::GIVE_WAY_GAP_M * aircraft::MDM_PER_M;
        let to_node = a.dist_to(k, &self.graph) - a.pos_mdm;
        if to_node < gap {
            return Err(format!("{} m from the junction; giving way needs {}: hold (H) instead", to_node / aircraft::MDM_PER_M, aircraft::GIVE_WAY_GAP_M));
        }
        let stop = aircraft::Stop { node, at_mdm: a.pos_mdm + to_node - gap, give_way: Some(o.callsign.clone()) };
        self.aircraft[i].stop = Some(stop);
        Ok(())
    }

    /// C3, after movement and before the monitor: settle the two verbs that
    /// wait on other aircraft. An X given ahead crosses if no one is on the
    /// runway (R4's test, `on_runway`) the tick the aircraft reaches the
    /// hold line, else lapses. A give way ends once its aircraft is gone, off
    /// the junction's route, or GROUND_MIN_M past the junction.
    fn ground_tick(&mut self) {
        let (ap, g) = (&self.airport, &self.graph);
        for i in 0..self.aircraft.len() {
            let a = &self.aircraft[i];
            if let (Phase::HoldShort(_), Some((first, _))) = (&a.phase, &a.cross_ahead) {
                let clear = a.held_crossing(ap, g).filter(|c| c.first == *first).is_some_and(|c| {
                    !self.aircraft.iter().enumerate().any(|(j, o)| j != i && conflict::on_runway(o, ap, g) == Some(c.runway))
                });
                let a = &mut self.aircraft[i];
                a.cross_ahead = None;
                if clear {
                    a.cross(ap, g).expect("held short of the crossing X was given for");
                }
            }
            let a = &self.aircraft[i];
            let Some(aircraft::Stop { node, give_way: Some(cs), .. }) = &a.stop else { continue };
            if given_way(self.aircraft.iter().find(|o| &o.callsign == cs), *node, g) {
                let a = &mut self.aircraft[i];
                a.stop = None;
                if a.phase == Phase::HoldPosition {
                    a.phase = Phase::Taxi;
                }
            }
        }
    }

    /// C3 verbs aircraft `i` would accept now, as (key, label). Apart from
    /// `verbs`, so the B2-C4 hint lists, and the scripts that read them, stay
    /// as they were. Each is asked on a copy, as `verbs` does, except give
    /// way, which needs a second aircraft: it is offered whenever `i` could
    /// give way to someone, and a refusal names the reason.
    pub fn ground_verbs(&self, i: usize) -> Vec<(&'static str, &'static str)> {
        let (a, ap, g) = (&self.aircraft[i], &self.airport, &self.graph);
        let ok = |f: &dyn Fn(&mut Aircraft) -> Result<(), String>| f(&mut a.scratch()).is_ok();
        let mut out = Vec::new();
        if ok(&|a| a.resume()) {
            out.push(("Enter", "Continue"));
        }
        if !a.hold_points(g).is_empty() {
            out.push(("J", "Hold at point"));
        }
        if matches!(a.phase, Phase::Taxi | Phase::HoldPosition) && a.on_edge() && self.aircraft.len() > 1 {
            out.push(("Y", "Give way"));
        }
        if !matches!(a.phase, Phase::HoldShort(_)) && ok(&|a| a.cross_ahead(ap, g)) {
            out.push(("X", "Cross if clear"));
        }
        if ok(&|a| a.backtrack(ap, g)) {
            out.push(("B", "Backtrack"));
        }
        if a.taxi_speed.slower().is_some_and(|s| ok(&|a| a.set_taxi_speed(s))) {
            out.push(("-", "Slower"));
        }
        if a.taxi_speed.faster().is_some_and(|s| ok(&|a| a.set_taxi_speed(s))) {
            out.push(("=", "Faster"));
        }
        out
    }

    pub fn line_up(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].line_up(&self.airport, &self.graph)
    }

    /// K. Never refused on separation grounds: the monitor logs what it
    /// breaks (PLAN-twr.md § B4).
    pub fn takeoff(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].takeoff()?;
        self.monitor.on_takeoff(&self.aircraft, i, &self.airport, &self.graph, self.tick);
        Ok(())
    }

    /// Ticks until K would lift off clear of every wake, and who binds.
    pub fn wake_wait(&self, i: usize) -> Option<(u64, String)> {
        self.monitor.wake_wait(&self.aircraft[i], &self.airport, &self.graph, self.tick)
    }

    /// V2 (PLAN-lvp.md): under LVP, who is in the localizer sensitive area
    /// of the end aircraft `i` is on final for (or just went around from).
    /// C still works; the strip says why it should wait. None with LVP off.
    pub fn lsa_blocker(&self, i: usize) -> Option<String> {
        let (Phase::Final(d) | Phase::GoAround(d)) = &self.aircraft[i].phase else { return None };
        self.monitor.lsa_occupant(&self.aircraft, &self.airport, &self.graph, d, Some(i)).map(|j| self.aircraft[j].callsign.clone())
    }

    /// Who would make K break R1 or R2 now, for the strip's neutral note.
    /// On final: anyone on the runway it will land on.
    pub fn runway_blocker(&self, i: usize) -> Option<String> {
        match self.aircraft[i].phase {
            Phase::LinedUp(_) => self.monitor.runway_blocker(&self.aircraft, i, &self.airport, &self.graph),
            Phase::Final(_) => conflict::occupants(&self.aircraft, i, &self.airport, &self.graph).first().map(|&j| self.aircraft[j].callsign.clone()),
            _ => None,
        }
    }

    pub fn verbs(&self, i: usize) -> Vec<char> {
        self.aircraft[i].verbs(&self.airport, &self.graph)
    }

    /// Next random draw; exposed so later slices share the one stream.
    pub fn rand(&mut self) -> f64 {
        self.rng.next_f64()
    }
}

/// C3: has `o` (None: left the game) passed `node` far enough for a give
/// way to end? Still ahead on its route: no. Behind it on the route: once
/// GROUND_MIN_M past. Not on its route at all (it went another way, or
/// its route was cut by a new clearance): yes.
fn given_way(o: Option<&Aircraft>, node: u32, g: &Graph) -> bool {
    let Some(o) = o.filter(|o| o.visible()) else { return true };
    let ahead_from = if o.on_edge() { o.leg + 1 } else { o.route.nodes.len() };
    if o.route.nodes[ahead_from..].contains(&node) {
        return false;
    }
    match o.route.nodes[..ahead_from].iter().rposition(|&n| n == node) {
        None => true,
        Some(k) => o.pos_mdm - o.dist_to(k, g) >= conflict::GROUND_MIN_M * aircraft::MDM_PER_M,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_real_second_is_three_game_seconds() {
        let mut c = Clock::default();
        // 1000 ms in 4 frames of 250 ms each, the frame cap.
        let steps: u32 = (0..4).map(|_| c.advance(250.0)).sum();
        assert_eq!(steps, TICK_HZ * 3);
        assert_eq!(c.game_seconds(), 3.0);
    }

    #[test]
    fn uneven_frames_lose_no_time() {
        let mut c = Clock::default();
        // 10 s of real time in ragged frames: 16.6, 17.1, 15.9 ms repeating.
        // Summed in integer microseconds so the expectation is exact too.
        let pattern_us: [u64; 3] = [16_600, 17_100, 15_900];
        let mut real_us = 0;
        let mut i = 0;
        while real_us < 10_000_000 {
            let dt = pattern_us[i % 3];
            c.advance(dt as f64 / 1000.0);
            real_us += dt;
            i += 1;
        }
        let expected = real_us * TIME_COMPRESSION_MILLI / TICK_GAME_NS;
        assert_eq!(c.ticks(), expected);
    }

    #[test]
    fn backgrounded_tab_is_capped_not_replayed() {
        let mut c = Clock::default();
        let steps = c.advance(30_000.0);
        // 250 ms real at 3x is 750 ms game: exactly 15 ticks, not 1800.
        assert_eq!(steps, 15);
    }

    #[test]
    fn junk_frame_times_step_nothing() {
        let mut c = Clock::default();
        assert_eq!(c.advance(f64::NAN), 0);
        assert_eq!(c.advance(-50.0), 0);
        assert_eq!(c.advance(f64::INFINITY), 0);
    }

    #[test]
    fn same_seed_same_world() {
        let mut a = World::new(42);
        let mut b = World::new(42);
        assert_eq!(a.aircraft[0].route, b.aircraft[0].route);
        assert_eq!(a.rand(), b.rand());
        let stands = |s| World::new(s).aircraft[0].route.end();
        assert!((0..8).map(stands).collect::<std::collections::HashSet<_>>().len() > 1, "seed picks the stand");
    }

    #[test]
    fn rng_matches_reference_splitmix64() {
        // Reference outputs of SplitMix64 for seed 0.
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }
}
