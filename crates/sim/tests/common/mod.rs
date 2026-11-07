//! The auto-controller shared by the long runs (PLAN-twr.md § B4, B5, B6).
#![allow(dead_code)]
//!
//! The auto-controller below spaces traffic by its own method, not the
//! monitor's, so the monitor is not checking itself:
//!   · ground: a push only with the whole taxi route out planned and
//!     reserved at once, and only if every point of push and route is more
//!     than RESERVE_M in a straight line from every other ground aircraft,
//!     everything still ahead of it, and every route reserved but not yet
//!     started. Cruder and stricter than G1: no aircraft ever waits on a
//!     taxilane for a route.
//!   · wake: its own table, its own runway pairs and its own intermediate
//!     test, timed from lift-offs it sees itself. K goes on the exact tick
//!     the rule allows, so an off-by-one in the monitor would show.
//!   · runway: K, X and line-up only with the runway clear in the ways the
//!     rules require, and no arrival on final for it within PROTECT_M; K the
//!     tick the leader passes the end, or the tick an arrival vacates.
//!   · arrivals (B5): one at a time per runway, spawned on final. C at
//!     2 km if the runway is clear and the exit route is free by the same
//!     reservation as a taxi route; G at 1.5 km if not. Two arrivals in ten
//!     have C withheld, one in ten is sent around by G at 4 km, and two in
//!     ten have a pilot go-around forced, on top of the world's own 5%
//!     draw, so each kind of go-around happens often enough to count.
//!     Vacated arrivals taxi to a free stand under the same reservation.

use std::collections::{BTreeMap, HashMap, HashSet};

use twr_sim::aircraft::{Aircraft, Phase, Wake, ROLL_TICKS};
use twr_sim::airport::EdgeKind;
use twr_sim::conflict::on_runway;
use twr_sim::route::Route;
use twr_sim::{Rng, World};

/// Straight-line buffer between any two ground routes, metres. G1 only
/// measures a gap when one aircraft is on the other's route ahead, within
/// BODY_M (20 m) of it, or both routes share a node; routes kept 60 m apart
/// (less 10 m either side for the 20 m sampling) can't do any of that. The
/// first version used 350 m and gridlocked the apron: 3 departures in 10 h.
pub const RESERVE_M: f64 = 60.0;
/// A departure every 60 s game. The first run used 90 s and reached the
/// wake boundary 45 times in 10 h, under the 50 floor: too few queues.
pub const SPAWN_EVERY: u64 = 60 * 20;
/// An arrival attempt every 2 min game, onto a runway with no other arrival.
pub const ARRIVAL_EVERY: u64 = 120 * 20;
/// No line-up, crossing or takeoff on a runway with an arrival on final
/// for it this close. A takeoff from rest clears IGI's longest runway in
/// about 75 s; an arrival 6 km out is 86 s from the threshold.
pub const PROTECT_M: f64 = 6_000.0;
pub const MAX_ACTIVE: usize = 14;
pub const HOUR: u64 = 3600 * 20;
pub const WEST: [&str; 4] = ["27", "28", "29R", "29L"];
pub const EAST: [&str; 4] = ["09", "10", "11L", "11R"];

/// The controller's own wake table (AIC 07/2020), written out again on
/// purpose rather than imported.
pub fn own_wake_s(l: Wake, f: Wake) -> u64 {
    match (l, f) {
        (Wake::J, Wake::H) | (Wake::H, Wake::M) | (Wake::H, Wake::L) | (Wake::M, Wake::L) => 120,
        (Wake::J, Wake::M) | (Wake::J, Wake::L) => 180,
        _ => 0,
    }
}

/// IGI pairs that share wake, from PLAN-twr.md § B4's measurements.
pub fn own_linked(a: &str, b: &str) -> Option<bool> {
    // Some(true): may be intermediate (same runway or parallel).
    if a == b {
        return Some(true);
    }
    let p = |x: &str, y: &str| (a == x && b == y) || (a == y && b == x);
    if p("29R", "29L") || p("11L", "11R") {
        Some(true)
    } else if p("27", "28") {
        Some(false)
    } else {
        None
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub spawned: u32,
    pub departed: u32,
    pub wake_boundary: u32,
    pub parallel_boundary: u32,
    pub arrivals: u32,
    pub landed: u32,
    pub on_stand: u32,
    pub after_arrival: u32,
    pub crossings: u32,
    pub ga_pilot: u32,
    pub ga_instructed: u32,
    pub ga_no_clearance: u32,
    pub peak_active: usize,
}

pub struct Lift {
    callsign: String,
    desig: String,
    wake: Wake,
    start: (f64, f64),
    tick: Option<u64>,
}

pub struct Ctl {
    pub rng: Rng,
    pub entries: BTreeMap<String, Vec<u32>>,
    /// Taxi routes reserved at push time, not yet started.
    pub planned: HashMap<String, Route>,
    pub pending: HashMap<String, (usize, bool)>,
    pub lifts: Vec<Lift>,
    pub vacated: HashMap<usize, u64>,
    /// Arrivals whose vacate tick has been recorded.
    pub seen_vacate: HashSet<String>,
    /// Arrivals the test sends around: C withheld, or G at 4 km.
    pub withhold: HashSet<String>,
    pub send_around: HashSet<String>,
    pub serial: u32,
    pub stats: Stats,
    /// B4/B5: the controller spawns its own traffic. B6: the world's
    /// traffic spawns, and the controller only controls.
    pub spawn: bool,
}

pub fn pos(w: &World, n: u32) -> (f64, f64) {
    let p = w.airport.nodes[n as usize];
    (f64::from(p.x) / 10.0, f64::from(p.y) / 10.0)
}

/// Points every 20 m along `r` from route distance `from_mdm` on.
pub fn samples(w: &World, r: &Route, from_mdm: u64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut s = 0u64;
    for (k, &e) in r.edges.iter().enumerate() {
        let len = u64::from(w.graph.len_dm[e as usize]) * 1000;
        let (a, b) = (pos(w, r.nodes[k]), pos(w, r.nodes[k + 1]));
        let mut d = 0u64;
        while d <= len {
            if s + d >= from_mdm {
                let t = d as f64 / len as f64;
                out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
            d += 200_000;
        }
        s += len;
    }
    if out.is_empty() {
        out.push(pos(w, r.end()));
    }
    out
}

pub fn ground(a: &Aircraft) -> bool {
    matches!(a.phase, Phase::Pushback | Phase::Holding | Phase::Taxi | Phase::HoldPosition | Phase::HoldShort(_) | Phase::LineUp(_))
}

/// A cleared arrival, or one rolling out: its exit route is reserved.
pub fn inbound(a: &Aircraft) -> bool {
    a.exit.is_some() && (matches!(a.phase, Phase::Landing(_)) || (matches!(a.phase, Phase::Final(_)) && a.cleared_to_land))
}

pub fn rwy(w: &World, a: &Aircraft) -> Option<usize> {
    w.graph.runway_at[a.route.nodes[0] as usize]
}

/// An arrival on final for runway `r` within `m` metres of its threshold,
/// or a go-around that has not yet passed the runway's far end. The first
/// version left go-arounds out: a K went out with one 1.5 km short of the
/// threshold, it overflew the roll, and the monitor logged 74 R2 incidents
/// over 3 seeds, correctly.
pub fn arrival_near(w: &World, r: usize, m: f64) -> bool {
    w.aircraft.iter().any(|a| {
        rwy(w, a) == Some(r)
            && match a.phase {
                Phase::Final(_) => a.final_m() <= m,
                Phase::GoAround(_) => a.air_mdm < a.exit.as_ref().unwrap().run_mdm as i64,
                _ => false,
            }
    })
}

/// Is `mine` clear of every other ground aircraft, its remaining route,
/// and every route reserved but not yet started?
pub fn reserve_ok(w: &World, me: usize, mine: &[(f64, f64)], planned: &HashMap<String, Route>) -> bool {
    let cell = |p: (f64, f64)| ((p.0 / RESERVE_M).floor() as i64, (p.1 / RESERVE_M).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<(f64, f64)>> = HashMap::new();
    for (j, a) in w.aircraft.iter().enumerate() {
        if j == me || !(ground(a) || inbound(a) || planned.contains_key(&a.callsign)) {
            continue;
        }
        let mut pts = vec![a.pos_m(&w.airport, &w.graph)];
        // An inbound reserves its exit route only, not the runway it lands on.
        let from = if inbound(a) { a.exit.as_ref().unwrap().at_mdm } else { a.pos_mdm };
        pts.extend(samples(w, &a.route, from));
        if let Some(r) = planned.get(&a.callsign) {
            pts.extend(samples(w, r, 0));
        }
        for p in pts {
            grid.entry(cell(p)).or_default().push(p);
        }
    }
    mine.iter().all(|&p| {
        let (cx, cy) = cell(p);
        (-1..=1).all(|dx| (-1..=1).all(|dy| grid.get(&(cx + dx, cy + dy)).is_none_or(|v| v.iter().all(|q| (p.0 - q.0).hypot(p.1 - q.1) > RESERVE_M))))
    })
}

pub fn crossing_on(pending: &HashMap<String, (usize, bool)>, r: usize) -> bool {
    pending.values().any(|(x, _)| *x == r)
}

/// Someone other than `me` is lining up onto runway `r`.
pub fn lining(w: &World, r: usize, me: usize) -> bool {
    w.aircraft.iter().enumerate().any(|(j, b)| j != me && matches!(b.phase, Phase::LineUp(_)) && w.graph.runway_at[b.route.end() as usize] == Some(r))
}

impl Ctl {
    pub fn new(w: &World, seed: u64) -> Ctl {
        let mut entries: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for at in w.airport.runway_exits() {
            if let Some((run, d)) = w.graph.runway_run(&w.airport, at) {
                if w.graph.run_len_dm(&run) / 10 >= twr_sim::aircraft::MIN_TAKEOFF_RUN_M {
                    entries.entry(d).or_default().push(at);
                }
            }
        }
        Ctl {
            rng: Rng::new(seed),
            entries,
            planned: HashMap::new(),
            pending: HashMap::new(),
            lifts: Vec::new(),
            vacated: HashMap::new(),
            seen_vacate: HashSet::new(),
            withhold: HashSet::new(),
            send_around: HashSet::new(),
            serial: 0,
            stats: Stats::default(),
            spawn: true,
        }
    }

    pub fn flow(&self, w: &World) -> [&'static str; 4] {
        // B6+ traffic flies the flow its wind picked (B9); the B4 runs, which
        // spawn their own, swap flow every hour.
        if !self.spawn {
            return match w.wind().flow() {
                twr_sim::weather::Flow::West => WEST,
                twr_sim::weather::Flow::East => EAST,
            };
        }
        if (w.tick / HOUR) % 2 == 0 { WEST } else { EAST }
    }

    pub fn pick(&mut self, n: u64) -> u64 {
        self.rng.next_u64() % n
    }

    /// One controller pass, after each tick.
    pub fn act(&mut self, w: &mut World) {
        let now = w.tick;
        // Lift-offs it sees for itself.
        for a in &w.aircraft {
            if matches!(a.phase, Phase::Airborne(_)) {
                if let Some(l) = self.lifts.iter_mut().find(|l| l.callsign == a.callsign && l.tick.is_none()) {
                    l.tick = Some(now);
                }
            }
        }
        self.lifts.retain(|l| l.tick.is_none_or(|t| now - t < 300 * 20));
        // Departed aircraft leave; stopped arrivals vacate.
        let before = w.aircraft.len();
        w.aircraft.retain(|a| !matches!(a.phase, Phase::Departed(_)));
        self.stats.departed += (before - w.aircraft.len()) as u32;
        // Arrivals on stand leave (B6: the world shuts them down itself);
        // record the tick each one vacates.
        let before = w.aircraft.len();
        let spawn = self.spawn;
        w.aircraft.retain(|a| !(if spawn { a.phase == Phase::OnStand && a.callsign.starts_with('A') } else { a.phase == Phase::ShutDown }));
        self.stats.on_stand += (before - w.aircraft.len()) as u32;
        for a in &w.aircraft {
            if let Some(x) = &a.exit {
                if a.phase == Phase::Holding && a.pos_mdm == x.vacate_mdm && self.seen_vacate.insert(a.callsign.clone()) {
                    self.vacated.insert(rwy(w, a).unwrap(), now);
                    self.stats.landed += 1;
                }
            }
        }
        let on: Vec<Option<usize>> = w.aircraft.iter().map(|a| on_runway(a, &w.airport, &w.graph)).collect();
        // Crossings finish once the aircraft has been on the runway and left it.
        let names: Vec<String> = w.aircraft.iter().map(|a| a.callsign.clone()).collect();
        self.pending.retain(|cs, (r, entered)| {
            let Some(i) = names.iter().position(|n| n == cs) else { return false };
            if on[i] == Some(*r) {
                *entered = true;
            }
            !(*entered && on[i] != Some(*r))
        });

        // K: every tick, on the exact tick allowed. Each K joins the lift
        // list before the next aircraft is judged: the first version judged
        // them all first, and two linked runways rolled on the same tick.
        for i in 0..w.aircraft.len() {
            let a = &w.aircraft[i];
            let Phase::LinedUp(d) = &a.phase else { continue };
            let r = on[i].unwrap();
            let busy = (0..w.aircraft.len()).any(|j| j != i && on[j] == Some(r)) || lining(w, r, i) || arrival_near(w, r, PROTECT_M);
            if busy || crossing_on(&self.pending, r) {
                continue;
            }
            let start = pos(w, a.route.nodes[0]);
            let mut earliest = 0u64;
            let mut parallel = false;
            let mut blocked = false;
            for l in &self.lifts {
                let Some(inter) = own_linked(&l.desig, d) else { continue };
                let mut s = own_wake_s(l.wake, a.wake);
                if s == 0 {
                    continue;
                }
                let Some(t) = l.tick else { blocked = true; break };
                let dir = w.monitor.dirs.iter().find(|x| x.desig == l.desig).unwrap();
                if inter && (start.0 - l.start.0) * dir.u.0 + (start.1 - l.start.1) * dir.u.1 > 150.0 {
                    s += 60;
                }
                let e = (t + s * 20).saturating_sub(ROLL_TICKS);
                if e > earliest {
                    earliest = e;
                    parallel = l.desig != *d;
                }
            }
            if blocked || earliest > now {
                continue;
            }
            let (wake_edge, after_arrival) = (earliest == now, self.vacated.get(&r) == Some(&now));
            let d = d.clone();
            self.lifts.push(Lift { callsign: a.callsign.clone(), desig: d, wake: a.wake, start: pos(w, a.route.nodes[0]), tick: None });
            w.takeoff(i).unwrap();
            self.stats.wake_boundary += u32::from(wake_edge);
            self.stats.parallel_boundary += u32::from(wake_edge && parallel);
            self.stats.after_arrival += u32::from(after_arrival);
        }

        if now % 20 != 0 {
            return;
        }
        let flow = self.flow(w);
        self.arrivals(w, flow);
        self.after_arrivals(w, flow);
    }

    /// Arrivals: clearances and go-arounds on final, then a new one onto a
    /// runway with no arrival, preferring one where a departure holds short
    /// at an entry, so the departure-after-arrival boundary is exercised.
    pub fn arrivals(&mut self, w: &mut World, flow: [&'static str; 4]) {
        let now = w.tick;
        let on: Vec<Option<usize>> = w.aircraft.iter().map(|a| on_runway(a, &w.airport, &w.graph)).collect();
        for i in 0..w.aircraft.len() {
            let a = &w.aircraft[i];
            if !matches!(a.phase, Phase::Final(_)) || a.cleared_to_land {
                continue;
            }
            let (cs, out, r) = (a.callsign.clone(), a.final_m(), rwy(w, a).unwrap());
            if self.withhold.contains(&cs) {
                continue;
            }
            if self.send_around.remove(&cs) {
                if out <= 4_000.0 {
                    w.go_around(i).unwrap();
                } else {
                    self.send_around.insert(cs);
                }
                continue;
            }
            if out > 2_000.0 {
                continue;
            }
            let clear = !on.contains(&Some(r)) && !lining(w, r, usize::MAX) && !crossing_on(&self.pending, r);
            let exit = samples(w, &a.route, a.exit.as_ref().unwrap().at_mdm);
            if clear && reserve_ok(w, i, &exit, &self.planned) {
                w.clear_to_land(i).unwrap();
            } else if out <= 1_500.0 {
                w.go_around(i).unwrap();
            }
        }
        // A test-forced go-around clears its flag, so the next approach lands.
        for a in &w.aircraft {
            if matches!(a.phase, Phase::GoAround(_)) {
                self.withhold.remove(&a.callsign);
            }
        }

        if !self.spawn || now % ARRIVAL_EVERY != 0 || w.aircraft.len() >= MAX_ACTIVE + 4 {
            return;
        }
        let busy = |w: &World, d: &str| {
            let r = w.monitor.dirs.iter().find(|x| x.desig == d).unwrap().rwy;
            w.aircraft.iter().enumerate().any(|(j, a)| {
                a.exit.is_some() && rwy(w, a) == Some(r) && (matches!(a.phase, Phase::Final(_) | Phase::Landing(_) | Phase::GoAround(_)) || on[j] == Some(r))
            })
        };
        let waiting: Vec<String> = (0..w.aircraft.len())
            .filter(|&j| matches!(w.aircraft[j].phase, Phase::HoldShort(_)) && !w.verbs(j).contains(&'X'))
            .filter_map(|j| w.graph.runway_run(&w.airport, w.aircraft[j].route.end()).map(|(_, x)| x))
            .filter(|d| !busy(w, d))
            .collect();
        let d = if waiting.is_empty() {
            // Not aimed at a waiting departure: 27/28 or 09/10, leaving the
            // parallels to departures. The first run with real arrivals used
            // all four and reached the parallel wake boundary 18 times on
            // seed 3, under the B4 floor of 20.
            let d = flow[self.pick(2) as usize];
            if busy(w, d) {
                return;
            }
            d.to_string()
        } else {
            waiting[self.pick(waiting.len() as u64) as usize].clone()
        };
        self.serial += 1;
        let cs = format!("A{:04}", self.serial);
        let i = w.spawn_arrival(&cs, Wake::M, &d).unwrap();
        self.stats.arrivals += 1;
        // Two in ten withheld and two in ten pilot go-arounds: at one in ten
        // each, seed 3 had 9 no-clearance go-arounds against the floor of 10
        // once arrivals moved off the parallels (92 arrivals in 10 h).
        match self.pick(10) {
            0 | 1 => {
                self.withhold.insert(cs);
            }
            2 => {
                self.send_around.insert(cs);
            }
            3 | 4 => w.aircraft[i].pilot_ga = Some((1_500 * 10_000 + self.pick(15_000_000), "unstable approach")),
            _ => {}
        }
    }

    /// The rest of the 20-tick pass: line-ups, crossings, pushes, spawns.
    pub fn after_arrivals(&mut self, w: &mut World, flow: [&'static str; 4]) {
        let now = w.tick;
        // X and L at hold-short points.
        let on: Vec<Option<usize>> = w.aircraft.iter().map(|a| on_runway(a, &w.airport, &w.graph)).collect();
        for i in 0..w.aircraft.len() {
            let Phase::HoldShort(name) = w.aircraft[i].phase.clone() else { continue };
            let r = w.airport.runways.iter().position(|x| x.name == name).unwrap();
            let a = &w.aircraft[i];
            let cs = a.callsign.clone();
            // The sim knows whether this hold guards a crossing or the entry.
            let at_entry = !w.verbs(i).contains(&'X');
            let entry = a.route.end();
            let others: Vec<usize> = (0..w.aircraft.len()).filter(|&j| j != i && on[j] == Some(r)).collect();
            if lining(w, r, i) || crossing_on(&self.pending, r) {
                continue;
            }
            // Crossings keep PROTECT_M; a line-up waits for any arrival on
            // final for this runway, so the departure lines up behind its
            // landing roll instead of sending it around.
            if arrival_near(w, r, if at_entry { f64::INFINITY } else { PROTECT_M }) {
                continue;
            }
            if !at_entry {
                if others.is_empty() {
                    w.cross(i).unwrap();
                    self.pending.insert(cs, (r, false));
                    self.stats.crossings += 1;
                }
                continue;
            }
            // Line up behind traffic rolling away down the same designator.
            let Some((run, d)) = w.graph.runway_run(&w.airport, entry) else { continue };
            let u = {
                let (p, q) = (pos(w, run.nodes[0]), pos(w, run.end()));
                let l = (q.0 - p.0).hypot(q.1 - p.1);
                ((q.0 - p.0) / l, (q.1 - p.1) / l)
            };
            let e = pos(w, entry);
            let ok = others.iter().all(|&j| {
                let b = &w.aircraft[j];
                let same = matches!(&b.phase, Phase::TakeoffRoll(x) | Phase::Airborne(x) | Phase::Landing(x) if *x == d);
                let p = b.pos_m(&w.airport, &w.graph);
                same && (p.0 - e.0) * u.0 + (p.1 - e.1) * u.1 > 100.0
            });
            if ok {
                w.line_up(i).unwrap();
            }
        }

        // Pushes and taxi clearances.
        for i in 0..w.aircraft.len() {
            let (phase, end, cs) = (w.aircraft[i].phase.clone(), w.aircraft[i].route.end(), w.aircraft[i].callsign.clone());
            match phase {
                // B6: an arrival on its stand waits to shut down; only
                // departures (the ones with a runway assigned) push.
                Phase::OnStand if !self.spawn && w.aircraft[i].assigned.is_none() => {}
                Phase::OnStand => {
                    let stand = end;
                    let Some(e) = w.airport.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == stand || e.b == stand)) else { continue };
                    let j = if e.a == stand { e.b } else { e.a };
                    let d = w.aircraft[i].assigned.unwrap_or_else(|| flow[self.pick(4) as usize]);
                    // B9: only entries with the run this class needs.
                    let need = twr_sim::aircraft::min_takeoff_run_m(w.aircraft[i].wake);
                    let ok: Vec<u32> = self.entries[d].iter().copied()
                        .filter(|&at| w.graph.runway_run(&w.airport, at).is_some_and(|(r, _)| w.graph.run_len_dm(&r) / 10 >= need))
                        .collect();
                    if ok.is_empty() {
                        continue;
                    }
                    let k = self.pick(ok.len() as u64) as usize;
                    let to = ok[k];
                    let Some(r) = w.graph.plan(&w.airport, &twr_sim::route::Request { from: j, to, ..Default::default() }) else { continue };
                    if r.edges.is_empty() {
                        continue;
                    }
                    let mut pts = vec![pos(w, stand)];
                    pts.extend(samples(w, &r, 0));
                    if reserve_ok(w, i, &pts, &self.planned) {
                        w.push(i).unwrap();
                        self.planned.insert(cs, r);
                    }
                }
                Phase::Holding if w.aircraft[i].exit.is_some() => {
                    // A vacated arrival: to a free stand, reserved like a push.
                    let n = w.airport.stands.len() as u64;
                    let s = w.airport.stands[self.pick(n) as usize].node;
                    let taken = w.aircraft.iter().any(|a| a.route.nodes.contains(&s)) || self.planned.values().any(|r| r.nodes.contains(&s));
                    if taken {
                        continue;
                    }
                    let Some(r) = w.plan(i, s, &[], &[]) else { continue };
                    if reserve_ok(w, i, &samples(w, &r, 0), &self.planned) {
                        w.taxi(i, r).unwrap();
                    }
                }
                Phase::Holding => {
                    // Its route was reserved with the push: send it as is.
                    if let Some(r) = self.planned.remove(&cs) {
                        w.taxi(i, r).unwrap();
                    }
                }
                _ => {}
            }
        }

        // Spawns.
        if self.spawn && now % SPAWN_EVERY == 0 && w.aircraft.len() < MAX_ACTIVE {
            let n = w.airport.stands.len() as u64;
            let s = w.airport.stands[self.pick(n) as usize].node;
            let taken = w.aircraft.iter().any(|a| a.route.nodes.contains(&s));
            let near = w.aircraft.iter().any(|a| ground(a) && {
                let (p, q) = (a.pos_m(&w.airport, &w.graph), pos(w, s));
                (p.0 - q.0).hypot(p.1 - q.1) <= RESERVE_M
            });
            if !taken && !near {
                let wake = match self.pick(100) {
                    0..70 => Wake::M,
                    70..90 => Wake::H,
                    90..95 => Wake::J,
                    _ => Wake::L,
                };
                self.serial += 1;
                w.add(Aircraft { wake, ..Aircraft::parked(&format!("T{:04}", self.serial), s) });
                self.stats.spawned += 1;
            }
        }
        self.stats.peak_active = self.stats.peak_active.max(w.aircraft.len());
    }
}


/// Aircraft the shift has given out that are not done: in the world and
/// not departed or shut down, plus both queues.
pub fn backlog(w: &World) -> usize {
    let t = w.traffic.as_ref().expect("a shift");
    let live = w.aircraft.iter().filter(|a| !matches!(a.phase, Phase::Departed(_) | Phase::ShutDown)).count();
    live + t.final_queue.len() + t.stand_queue.len()
}

/// The controller on a shift's own traffic, past 30:00 until `ticks`.
/// `wind`: force one (B9), or None for the seed's own.
#[allow(dead_code)]
pub fn run_shift(seed: u64, mode: twr_sim::Mode, wind: Option<twr_sim::weather::Wind>, ticks: u64, mut each: impl FnMut(&World)) -> (World, Stats, usize) {
    let mut w = World::shift(seed, mode);
    if let Some(wind) = wind {
        w.traffic.as_mut().expect("a shift").schedule.wind = wind;
    }
    // Watch the controller finish what the shift gave it, past 30:00.
    w.halt_on_end = false;
    let mut c = Ctl::new(&w, seed);
    c.spawn = false;
    let mut most_conflicts = 0;
    while w.tick < ticks {
        w.step_ticks(1);
        c.act(&mut w);
        most_conflicts = most_conflicts.max(w.monitor.conflicts.len());
        each(&w);
    }
    (w, c.stats, most_conflicts)
}
