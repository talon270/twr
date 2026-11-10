//! TWR-SIM · CONFLICT
//!   · wake_min_s: the departure wake table (PLAN-twr.md § B4, W0-W2)
//!   · Dir, Link: each runway direction's axis, and which pairs share wake (W3)
//!   · on_runway, runway_spans: where an aircraft counts as on a runway (R4)
//!   · Monitor: per-tick runway (R1-R3) and ground (G1, G2) checks, the wake
//!     check at each lift-off, live conflicts, and the incident log
//!
//! The model flags; it never prevents. No verb refuses on separation grounds,
//! so a wrong clearance plays out and is logged, as on a real frequency.
//!
//! Every pair key is a callsign pair in a BTreeMap or BTreeSet, so the
//! incident log comes out in the same order on every run of a seed.

use std::collections::{BTreeMap, BTreeSet};

use crate::aircraft::{crossings, Aircraft, Phase, Wake, MDM_PER_M, ROLL_TICKS, TAXI_MDM_PER_TICK};
use crate::airport::Airport;
use crate::lvp::{Lsa, LVP_DECISION_M};
use crate::route::{designator, Graph, Route};
use crate::TICK_HZ;

// ---- Constants (PLAN-twr.md § B4, "The rules") -----------------------------

/// W3: parallel runways closer than this share wake as one runway. Doc 4444
/// via SKYbrary and UK SERA.8012 (snippet). IGI's 11L/29R and 11R/29L: 381 m.
pub const PARALLEL_SPACING_M: f64 = 760.0;
/// Headings within this many degrees count as parallel. Authored: IGI's
/// near-parallels are 0.0 and 1.0 degrees apart, its next pair 12.2.
pub const PARALLEL_DEG: f64 = 5.0;
/// W3: how far past the runway end a departure path is projected, straight,
/// to see whether it crosses another. Authored; the game has no SIDs.
pub const PROJECT_M: f64 = 10_000.0;
/// W2: a follower starting this far beyond the leader's start is an
/// intermediate departure. Doc 4444 5.8.3.4 via the Airservices AIP (snippet).
pub const INTERMEDIATE_M: f64 = 150.0;
/// W2: added to the W1 minimum for an intermediate departure (snippet).
pub const INTERMEDIATE_EXTRA_S: u64 = 60;
/// G1: under this, centre to centre along the network, is an incident.
/// Authored: an A320's 37.6 m length (recalled, not checked) plus margin.
pub const GROUND_MIN_M: u64 = 45;
/// G1: under this and closing is a live conflict. Authored: 4.3 s of real
/// time head-on at 2 x 10 m/s before GROUND_MIN_M, 8.5 s behind a stopped
/// aircraft.
pub const GROUND_WARN_M: u64 = 300;
/// G1: a stopped aircraft this far off the mover's route still counts:
/// half an A320's length. Authored.
pub const BODY_M: u64 = 20;
/// How long an incident's square stays on the map, game seconds.
pub const INCIDENT_MARK_S: u64 = 60;
/// Longest wake minimum (3:00 behind J, plus 1:00 intermediate): leaders
/// older than this can't constrain anyone.
const WAKE_LOOKBACK_S: u64 = 240;

/// W1: departure wake minimum in seconds, leader then follower; 0 for none.
/// Doc 4444 Amendment 9 as published in GACA AIC 07/2020 (read 2026-11-05).
pub fn wake_min_s(lead: Wake, follow: Wake) -> u64 {
    use Wake::*;
    match (lead, follow) {
        (J, H) => 120,
        (J, M) | (J, L) => 180,
        (H, M) | (H, L) => 120,
        (M, L) => 120,
        _ => 0,
    }
}

// ---- Runway directions and wake links --------------------------------------

/// One direction of one runway: its designator, start and unit heading, in
/// metres east and north.
#[derive(Debug, Clone)]
pub struct Dir {
    pub rwy: usize,
    pub desig: String,
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub u: (f64, f64),
}

/// Why two departures share a wake minimum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Same,
    /// Parallel and under PARALLEL_SPACING_M apart.
    Parallel,
    /// Projected paths cross within PROJECT_M of the runway end.
    Crossing,
}

fn m(ap: &Airport, n: u32) -> (f64, f64) {
    let p = ap.nodes[n as usize];
    (f64::from(p.x) / 10.0, f64::from(p.y) / 10.0)
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(o: (f64, f64), p: (f64, f64), q: (f64, f64)) -> f64 {
    (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0)
}

fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0
}

/// Both directions of every runway.
pub fn runway_dirs(ap: &Airport) -> Vec<Dir> {
    let mut out = Vec::new();
    for (i, r) in ap.runways.iter().enumerate() {
        let (a, b) = (m(ap, r.ends[0]), m(ap, r.ends[1]));
        for (from, to) in [(a, b), (b, a)] {
            let v = sub(to, from);
            let len = v.0.hypot(v.1);
            let bearing = v.0.atan2(v.1).to_degrees().rem_euclid(360.0);
            if let Some(desig) = designator(&r.name, bearing) {
                out.push(Dir { rwy: i, desig, from, to, u: (v.0 / len, v.1 / len) });
            }
        }
    }
    out
}

/// Wake links between directions of different runways, both orders.
/// Opposite flows are never linked: B4 runs one flow at a time, and
/// opposite-direction wake (Doc 4444 5.8.5) is out of scope.
pub fn wake_links(dirs: &[Dir]) -> BTreeMap<(String, String), Link> {
    let mut out = BTreeMap::new();
    for p in dirs {
        for q in dirs {
            if p.rwy == q.rwy {
                continue;
            }
            let cos = dot(p.u, q.u).clamp(-1.0, 1.0);
            if cos <= 0.0 {
                continue;
            }
            let apart_deg = cos.acos().to_degrees();
            let mid = ((q.from.0 + q.to.0) / 2.0, (q.from.1 + q.to.1) / 2.0);
            let off = cross(p.from, p.to, mid).abs() / sub(p.to, p.from).0.hypot(sub(p.to, p.from).1);
            let far = |d: &Dir| (d.to.0 + d.u.0 * PROJECT_M, d.to.1 + d.u.1 * PROJECT_M);
            let link = if apart_deg < PARALLEL_DEG && off < PARALLEL_SPACING_M {
                Link::Parallel
            } else if segments_cross(p.from, far(p), q.from, far(q)) {
                Link::Crossing
            } else {
                continue;
            };
            out.insert((p.desig.clone(), q.desig.clone()), link);
        }
    }
    out
}

// ---- Where an aircraft is ---------------------------------------------------

/// The stretch of `route` that counts as on each runway it crosses: from
/// its hold-short limit to as far past the runway as that limit sits before
/// it. (start, end, runway), route distances in milli-decimetres.
pub fn runway_spans(ap: &Airport, g: &Graph, route: &Route) -> Vec<(u64, u64, usize)> {
    crossings(ap, g, route, 0, 0).into_iter().map(|c| (c.lim, c.exit, c.runway)).collect()
}

/// R4: the runway an aircraft is on, if any. An aircraft holding short sits
/// exactly at its limit, which is not on the runway; one step past it is.
/// B5: an arrival is on its runway from the threshold until it passes the
/// vacate point at the end of its exit route; a go-around while it is over
/// the runway, up to the far end (R2 treats it as a departure).
pub fn on_runway(a: &Aircraft, ap: &Airport, g: &Graph) -> Option<usize> {
    let first = g.runway_at[a.route.nodes[0] as usize];
    match &a.phase {
        Phase::OnStand | Phase::Departed(_) | Phase::Final(_) | Phase::ShutDown => None,
        Phase::GoAround(_) => first.filter(|_| a.over_runway()),
        Phase::LinedUp(_) | Phase::TakeoffRoll(_) | Phase::Airborne(_) | Phase::Landing(_) => first,
        // C3: a backtrack rolls along the runway itself, end to end. Its
        // route has no hold-short span, so the span test below misses it.
        Phase::LineUp(_) if a.on_runway_route(ap) => first,
        _ if a.exit.is_some() => first.filter(|_| a.pos_mdm < a.exit.as_ref().expect("guarded").vacate_mdm),
        _ => runway_spans(ap, g, &a.route).into_iter().find(|&(s, e, _)| a.pos_mdm > s && a.pos_mdm <= e).map(|x| x.2),
    }
}

/// R5: everyone on the runway aircraft `i` is landing on (or on final for).
pub fn occupants(ac: &[Aircraft], i: usize, ap: &Airport, g: &Graph) -> Vec<usize> {
    let Some(r) = g.runway_at[ac[i].route.nodes[0] as usize] else { return Vec::new() };
    (0..ac.len()).filter(|&j| j != i && on_runway(&ac[j], ap, g) == Some(r)).collect()
}

fn departing(a: &Aircraft) -> bool {
    matches!(a.phase, Phase::TakeoffRoll(_) | Phase::Airborne(_))
}

/// Moving fast along a runway: a takeoff or landing roll.
pub fn rolling(a: &Aircraft) -> bool {
    departing(a) || (matches!(a.phase, Phase::Landing(_)) && a.speed_mdm > 0)
}

/// G2: phases the ground check sees at all. Lined up and on stand are
/// stopped on the network too: without them a second aircraft lined up into
/// an occupied spot with nothing logged (R1.4). Stopped pairs are never
/// compared, so two parked aircraft stay clean.
fn on_ground_net(a: &Aircraft) -> bool {
    matches!(a.phase, Phase::OnStand | Phase::Pushback | Phase::Holding | Phase::Taxi | Phase::HoldPosition | Phase::HoldShort(_) | Phase::LineUp(_) | Phase::LinedUp(_))
}

/// G2: phases in which an aircraft is moving along its route.
fn ground_mover(a: &Aircraft) -> bool {
    a.on_edge() && matches!(a.phase, Phase::Pushback | Phase::Taxi | Phase::LineUp(_))
}

/// Where an aircraft is on the network: on an edge, oriented as its route
/// runs, with its offsets from both ends; or at a node.
struct Spot {
    a: u32,
    b: u32,
    edge: Option<u32>,
    off_a: u64,
    off_b: u64,
}

fn spot(a: &Aircraft, g: &Graph) -> Spot {
    if a.on_edge() {
        let e = a.route.edges[a.leg];
        let len = u64::from(g.len_dm[e as usize]) * 1000;
        Spot { a: a.route.nodes[a.leg], b: a.route.nodes[a.leg + 1], edge: Some(e), off_a: a.along_mdm, off_b: len - a.along_mdm }
    } else {
        let n = a.route.end();
        Spot { a: n, b: n, edge: None, off_a: 0, off_b: 0 }
    }
}

/// Nodes on a mover's route ahead, each with its distance from the mover
/// and the edge the route takes next, out to `cap`.
fn path_ahead(a: &Aircraft, g: &Graph, cap: u64) -> Vec<(u32, u64, Option<u32>)> {
    let mut out = Vec::new();
    let mut k = a.leg + 1;
    let mut s = u64::from(g.len_dm[a.route.edges[a.leg] as usize]) * 1000 - a.along_mdm;
    while k < a.route.nodes.len() {
        let next = a.route.edges.get(k).copied();
        out.push((a.route.nodes[k], s, next));
        match next {
            Some(e) if s <= cap => s += u64::from(g.len_dm[e as usize]) * 1000,
            _ => break,
        }
        k += 1;
    }
    out
}

/// G1 from one mover's side: distance along its route ahead to the other
/// aircraft, which may sit up to BODY_M off that route.
fn gap_ahead(mv: &Aircraft, path: &[(u32, u64, Option<u32>)], o: &Spot) -> Option<u64> {
    let body = BODY_M * MDM_PER_M;
    let mut best: Option<u64> = None;
    let mut take = |d: u64| best = Some(best.map_or(d, |b| b.min(d)));
    let me = mv.route.edges[mv.leg];
    if o.edge == Some(me) {
        let from_my_a = if o.a == mv.route.nodes[mv.leg] { o.off_a } else { o.off_b };
        if from_my_a > mv.along_mdm {
            take(from_my_a - mv.along_mdm);
        }
    }
    for &(n, s, next) in path {
        match o.edge {
            None if o.a == n => take(s),
            Some(e) => {
                let on_path = next == Some(e);
                if o.a == n && (on_path || o.off_a <= body) {
                    take(s + o.off_a);
                }
                if o.b == n && (on_path || o.off_b <= body) {
                    take(s + o.off_b);
                }
            }
            None => {}
        }
    }
    best
}

// ---- The monitor ------------------------------------------------------------

/// One takeoff, for the wake checks that follow it.
#[derive(Debug, Clone)]
pub struct Departure {
    pub callsign: String,
    pub wake: Wake,
    pub rwy: usize,
    pub desig: String,
    pub start: (f64, f64),
    pub roll_tick: u64,
    pub airborne_tick: Option<u64>,
    /// Who was already on the runway when the roll began: R1 for them,
    /// R3 for anyone who enters later.
    pub there_at_roll: Vec<String>,
}

/// What an incident broke. The Debug text is what the page shows and the
/// scripts read: renaming a variant breaks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    Ground,
    Wake,
    Occupied,
    Departure,
    Incursion,
    /// V3 (PLAN-lvp.md): under LVP, anything in the localizer sensitive
    /// area of an arrival inside 2 NM, until its landing roll ends.
    Lsa,
}

/// A live G1 conflict this tick: aircraft indices into `World::aircraft`.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub a: usize,
    pub b: usize,
    pub d_m: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Incident {
    pub tick: u64,
    pub rule: Rule,
    pub a: String,
    pub b: String,
    pub text: String,
    pub at: (f64, f64),
}

pub struct Monitor {
    pub dirs: Vec<Dir>,
    pub links: BTreeMap<(String, String), Link>,
    pub departures: Vec<Departure>,
    pub conflicts: Vec<Conflict>,
    pub incidents: Vec<Incident>,
    /// Pairs with an incident counted and not yet over: (kind, a, b).
    open: BTreeSet<(&'static str, String, String)>,
    /// Ground distance per pair last tick, to tell closing from opening.
    prev: BTreeMap<(String, String), u64>,
    /// LVP for the whole shift (PLAN-lvp.md § A1): the draw sets it for a
    /// scored shift, the Practice toggle sets it there.
    pub lvp: bool,
    /// Each landing end's localizer sensitive area.
    pub lsa: Lsa,
}

fn pair(a: &str, b: &str) -> (String, String) {
    if a <= b { (a.into(), b.into()) } else { (b.into(), a.into()) }
}

fn clock(ticks: u64) -> String {
    let s = ticks / u64::from(TICK_HZ);
    format!("{}:{:02}", s / 60, s % 60)
}

impl Monitor {
    pub fn new(ap: &Airport) -> Monitor {
        let dirs = runway_dirs(ap);
        let links = wake_links(&dirs);
        Monitor { dirs, links, departures: Vec::new(), conflicts: Vec::new(), incidents: Vec::new(), open: BTreeSet::new(), prev: BTreeMap::new(), lvp: false, lsa: Lsa::vidp() }
    }

    /// Under LVP, the first aircraft other than `except` whose body is in
    /// `desig`'s LSA: on the ground, or rolling. None with LVP off, or
    /// with no LSA for that end. A same-runway departure counts until it
    /// is airborne, about 10 s early against AD 2.22 VIII 4.2.3's "passed
    /// over the localizer" (930 m past the end at 90 m/s).
    pub fn lsa_occupant(&self, ac: &[Aircraft], ap: &Airport, g: &Graph, desig: &str, except: Option<usize>) -> Option<usize> {
        if !self.lvp {
            return None;
        }
        let d = self.dir(desig)?;
        (0..ac.len()).find(|&j| Some(j) != except && self.in_lsa(&ac[j], ap, g, d))
    }

    /// On the ground or rolling, with its body inside `d`'s LSA.
    fn in_lsa(&self, a: &Aircraft, ap: &Airport, g: &Graph, d: &Dir) -> bool {
        (on_ground_net(a) || matches!(a.phase, Phase::Landing(_) | Phase::TakeoffRoll(_))) && self.lsa.contains(d, a.pos_m(ap, g), BODY_M as f64)
    }

    fn dir(&self, desig: &str) -> Option<&Dir> {
        self.dirs.iter().find(|d| d.desig == desig)
    }

    pub fn link(&self, lead_rwy: usize, lead: &str, fol_rwy: usize, fol: &str) -> Option<Link> {
        if lead_rwy == fol_rwy {
            return (lead == fol).then_some(Link::Same);
        }
        self.links.get(&(lead.to_string(), fol.to_string())).copied()
    }

    /// W1-W3: ticks required from the leader's lift-off to the follower's.
    pub fn required(&self, lead: &Departure, wake: Wake, rwy: usize, desig: &str, start: (f64, f64)) -> Option<u64> {
        let link = self.link(lead.rwy, &lead.desig, rwy, desig)?;
        let base = wake_min_s(lead.wake, wake);
        if base == 0 {
            return None;
        }
        let u = self.dir(&lead.desig)?.u;
        let extra = if link != Link::Crossing && dot(sub(start, lead.start), u) > INTERMEDIATE_M { INTERMEDIATE_EXTRA_S } else { 0 };
        Some((base + extra) * u64::from(TICK_HZ))
    }

    /// A lined-up aircraft's takeoff, as a record (not yet logged).
    fn record(a: &Aircraft, ap: &Airport, g: &Graph, tick: u64) -> Option<Departure> {
        let (Phase::LinedUp(d) | Phase::TakeoffRoll(d)) = &a.phase else { return None };
        let entry = a.route.nodes[0];
        let rwy = g.runway_at[entry as usize]?;
        Some(Departure { callsign: a.callsign.clone(), wake: a.wake, rwy, desig: d.clone(), start: m(ap, entry), roll_tick: tick, airborne_tick: None, there_at_roll: Vec::new() })
    }

    /// Called when K succeeds on aircraft `i`.
    pub fn on_takeoff(&mut self, ac: &[Aircraft], i: usize, ap: &Airport, g: &Graph, tick: u64) {
        if let Some(mut d) = Self::record(&ac[i], ap, g, tick) {
            d.there_at_roll = ac.iter().enumerate().filter(|&(j, y)| j != i && on_runway(y, ap, g) == Some(d.rwy)).map(|(_, y)| y.callsign.clone()).collect();
            self.departures.push(d);
        }
    }

    /// Called on the tick an aircraft lifts off: the wake check (W1-W3),
    /// timed lift-off to lift-off.
    pub fn on_airborne(&mut self, callsign: &str, tick: u64, at: (f64, f64)) {
        let Some(k) = self.departures.iter().rposition(|d| d.callsign == callsign && d.airborne_tick.is_none()) else { return };
        self.departures[k].airborne_tick = Some(tick);
        let f = self.departures[k].clone();
        let lookback = WAKE_LOOKBACK_S * u64::from(TICK_HZ);
        let mut found = Vec::new();
        for l in &self.departures {
            let Some(t) = l.airborne_tick else { continue };
            if l.callsign == f.callsign || t > tick || tick - t > lookback {
                continue;
            }
            if let Some(req) = self.required(l, f.wake, f.rwy, &f.desig, f.start) {
                if tick - t < req {
                    let text = format!("WAKE · {} ({:?}) {} behind {} ({:?}), min {}", f.callsign, f.wake, clock(tick - t), l.callsign, l.wake, clock(req));
                    found.push(Incident { tick, rule: Rule::Wake, a: f.callsign.clone(), b: l.callsign.clone(), text, at });
                }
            }
        }
        self.incidents.extend(found);
        // A leader airborne longer ago than the lookback constrains no one,
        // here or in `wake_wait`, which scanned every takeoff of the shift
        // each tick a departure stood lined up (R1.21).
        self.departures.retain(|d| d.airborne_tick.is_none_or(|t| tick - t <= lookback));
    }

    /// Ticks until a lined-up aircraft may press K and still lift off
    /// clear of every leader's wake, and the leader that binds. None when
    /// no wake applies now.
    pub fn wake_wait(&self, a: &Aircraft, ap: &Airport, g: &Graph, now: u64) -> Option<(u64, String)> {
        let f = Self::record(a, ap, g, now)?;
        let mut worst: Option<(u64, String)> = None;
        for l in &self.departures {
            if l.callsign == f.callsign {
                continue;
            }
            // Every roll takes the same ROLL_TICKS to lift-off in B4, so a
            // rolling leader's lift-off tick is known before it happens.
            let t = l.airborne_tick.unwrap_or(l.roll_tick + ROLL_TICKS);
            let Some(req) = self.required(l, f.wake, f.rwy, &f.desig, f.start) else { continue };
            let earliest = (t + req).saturating_sub(ROLL_TICKS);
            if earliest > now && worst.as_ref().is_none_or(|(w, _)| earliest - now > *w) {
                worst = Some((earliest - now, l.callsign.clone()));
            }
        }
        worst
    }

    /// Who would make a takeoff roll by `a` break R1 or R2 right now.
    pub fn runway_blocker(&self, ac: &[Aircraft], i: usize, ap: &Airport, g: &Graph) -> Option<String> {
        let a = &ac[i];
        let r = on_runway(a, ap, g)?;
        let u = sub(m(ap, a.route.end()), m(ap, a.route.nodes[0]));
        let pa = a.pos_m(ap, g);
        ac.iter().enumerate().find(|&(j, y)| {
            j != i && on_runway(y, ap, g) == Some(r) && (matches!(y.phase, Phase::Landing(_)) || dot(sub(y.pos_m(ap, g), pa), u) > 0.0)
        }).map(|(_, y)| y.callsign.clone())
    }

    /// R5, on the tick aircraft `i` crosses its threshold: anyone still on
    /// the runway is an incident. Keyed like R1-R3, so the roll that follows
    /// does not count the same pair again.
    pub fn on_threshold(&mut self, ac: &[Aircraft], i: usize, ap: &Airport, g: &Graph, tick: u64) {
        let x = &ac[i];
        let Phase::Landing(d) = &x.phase else { return };
        for j in occupants(ac, i, ap, g) {
            let y = &ac[j];
            let name = &ap.runways[on_runway(y, ap, g).expect("an occupant")].name;
            let text = format!("RUNWAY OCCUPIED · {} crossed the {d} threshold with {} on {name}", x.callsign, y.callsign);
            self.incident("rwy", tick, Rule::Occupied, &x.callsign, &y.callsign, text, y.pos_m(ap, g));
        }
    }

    /// Run every per-tick check. `tick` is the tick just stepped.
    pub fn check(&mut self, ac: &[Aircraft], ap: &Airport, g: &Graph, tick: u64) {
        self.runway(ac, ap, g, tick);
        self.ground(ac, ap, g, tick);
    }

    fn incident(&mut self, kind: &'static str, tick: u64, rule: Rule, a: &str, b: &str, text: String, at: (f64, f64)) {
        let (x, y) = pair(a, b);
        if self.open.insert((kind, x, y)) {
            self.incidents.push(Incident { tick, rule, a: a.into(), b: b.into(), text, at });
        }
    }

    /// R1-R3: a roll with anyone ahead of it on its runway, or a takeoff
    /// roll with an arrival anywhere on it.
    fn runway(&mut self, ac: &[Aircraft], ap: &Airport, g: &Graph, tick: u64) {
        let on: Vec<Option<usize>> = ac.iter().map(|a| on_runway(a, ap, g)).collect();
        let mut seen = BTreeSet::new();
        for (xi, x) in ac.iter().enumerate() {
            if !rolling(x) {
                continue;
            }
            let Some(r) = on[xi] else { continue };
            let u = sub(m(ap, x.route.end()), m(ap, x.route.nodes[0]));
            let px = x.pos_m(ap, g);
            let name = &ap.runways[r].name;
            for (yi, y) in ac.iter().enumerate() {
                if yi == xi || on[yi] != Some(r) {
                    continue;
                }
                let py = y.pos_m(ap, g);
                let ahead = dot(sub(py, px), u) > 0.0;
                let y_lands = matches!(y.phase, Phase::Landing(_));
                if !(ahead || (departing(x) && y_lands)) {
                    continue;
                }
                let key = pair(&x.callsign, &y.callsign);
                seen.insert(("rwy", key.0.clone(), key.1.clone()));
                if self.open.contains(&("rwy", key.0, key.1)) {
                    continue;
                }
                let was_there = self.departures.iter().rev().find(|d| d.callsign == x.callsign).is_some_and(|d| d.there_at_roll.contains(&y.callsign));
                let (rule, text) = if departing(x) && y_lands {
                    (Rule::Occupied, format!("RUNWAY OCCUPIED · {} rolled on {name} with arrival {} on it", x.callsign, y.callsign))
                } else if departing(y) || matches!(y.phase, Phase::GoAround(_)) {
                    (Rule::Departure, format!("DEPARTURE · {} rolled before {} passed the end of {name}", x.callsign, y.callsign))
                } else if departing(x) && was_there {
                    (Rule::Occupied, format!("RUNWAY OCCUPIED · {} rolled on {name} with {} ahead", x.callsign, y.callsign))
                } else {
                    let roll = if departing(x) { "takeoff" } else { "landing" };
                    (Rule::Incursion, format!("INCURSION · {} on {name} ahead of {}'s {roll} roll", y.callsign, x.callsign))
                };
                // Mark the aircraft that was in the way: the roll and it can
                // be kilometres apart, and their midpoint marks empty grass.
                self.incident("rwy", tick, rule, &x.callsign, &y.callsign, text, py);
            }
        }
        self.lsa_check(ac, ap, g, tick, &mut seen);
        self.open.retain(|k| k.0 != "rwy" || seen.contains(k));
    }

    /// V2, V3 (PLAN-lvp.md): under LVP the LSA ahead of an arrival is
    /// protected from 2 NM until its landing roll slows to taxi speed. It
    /// shares the runway key, so while the pair stays open R5 at the
    /// threshold does not log the same mistake twice.
    fn lsa_check(&mut self, ac: &[Aircraft], ap: &Airport, g: &Graph, tick: u64, seen: &mut BTreeSet<(&'static str, String, String)>) {
        if !self.lvp {
            return;
        }
        for x in ac {
            let d = match &x.phase {
                // Cleared only: an uncleared arrival spends one tick inside
                // 2 NM before its go-around fires, and that tick is not a
                // protected approach (PLAN-lvp.md: protection after C).
                Phase::Final(d) if x.cleared_to_land && x.final_m() <= LVP_DECISION_M as f64 => d,
                Phase::Landing(d) if x.speed_mdm > TAXI_MDM_PER_TICK => d,
                _ => continue,
            };
            let Some(dir) = self.dir(d).cloned() else { continue };
            let inside: Vec<&Aircraft> = ac.iter().filter(|y| y.callsign != x.callsign && self.in_lsa(y, ap, g, &dir)).collect();
            for y in inside {
                let key = pair(&x.callsign, &y.callsign);
                seen.insert(("rwy", key.0, key.1));
                let text = format!("LSA · {} in the {d} localizer sensitive area with {} inside 2 NM", y.callsign, x.callsign);
                self.incident("rwy", tick, Rule::Lsa, &x.callsign, &y.callsign, text, y.pos_m(ap, g));
            }
        }
    }

    /// G1, G2: spacing along the network between ground aircraft, at least
    /// one of them moving.
    fn ground(&mut self, ac: &[Aircraft], ap: &Airport, g: &Graph, tick: u64) {
        let warn = GROUND_WARN_M * MDM_PER_M;
        let min = GROUND_MIN_M * MDM_PER_M;
        let cap = warn + BODY_M * MDM_PER_M;
        let idx: Vec<usize> = (0..ac.len()).filter(|&i| on_ground_net(&ac[i])).collect();
        let paths: Vec<Option<Vec<(u32, u64, Option<u32>)>>> =
            ac.iter().map(|a| (on_ground_net(a) && ground_mover(a)).then(|| path_ahead(a, g, cap))).collect();
        let spots: Vec<Option<Spot>> = ac.iter().map(|a| on_ground_net(a).then(|| spot(a, g))).collect();
        let mut prev = BTreeMap::new();
        self.conflicts.clear();
        for (n, &i) in idx.iter().enumerate() {
            for &j in &idx[n + 1..] {
                if paths[i].is_none() && paths[j].is_none() {
                    continue;
                }
                let (a, b) = (&ac[i], &ac[j]);
                let mut d: Option<u64> = None;
                let mut take = |x: Option<u64>| if let Some(x) = x { d = Some(d.map_or(x, |y| y.min(x))) };
                if let Some(p) = &paths[i] {
                    take(gap_ahead(a, p, spots[j].as_ref().expect("ground aircraft")));
                }
                if let Some(p) = &paths[j] {
                    take(gap_ahead(b, p, spots[i].as_ref().expect("ground aircraft")));
                }
                if let (Some(pa), Some(pb)) = (&paths[i], &paths[j]) {
                    // Converging: both routes run through the same node ahead.
                    for &(na, sa, _) in pa {
                        if let Some(&(_, sb, _)) = pb.iter().find(|&&(nb, _, _)| nb == na) {
                            take(Some(sa + sb));
                        }
                    }
                }
                let key = pair(&a.callsign, &b.callsign);
                let ep = ("gnd", key.0.clone(), key.1.clone());
                let Some(d) = d.filter(|&d| d < warn) else {
                    self.open.remove(&ep);
                    continue;
                };
                let closing = self.prev.get(&key).is_some_and(|&p| d < p);
                prev.insert(key, d);
                if closing {
                    self.conflicts.push(Conflict { a: i, b: j, d_m: d as f64 / MDM_PER_M as f64 });
                }
                if d < min {
                    let (pa, pb) = (a.pos_m(ap, g), b.pos_m(ap, g));
                    // Name the mover first; one decimal, so 44.6 m never
                    // prints as "45 m, min 45".
                    let (m1, m2) = if paths[i].is_none() { (b, a) } else { (a, b) };
                    let text = format!("GROUND · {} {:.1} m from {}, min {GROUND_MIN_M}", m1.callsign, d as f64 / MDM_PER_M as f64, m2.callsign);
                    self.incident("gnd", tick, Rule::Ground, &m1.callsign, &m2.callsign, text, ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0));
                }
            }
        }
        self.prev = prev;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_table_matches_aic_07_2020() {
        use Wake::*;
        let all = [L, M, H, J];
        let want = |l, f| match (l, f) {
            (J, H) => 120,
            (J, M) | (J, L) => 180,
            (H, M) | (H, L) | (M, L) => 120,
            _ => 0,
        };
        for l in all {
            for f in all {
                assert_eq!(wake_min_s(l, f), want(l, f), "{l:?} then {f:?}");
            }
        }
    }

    /// The IGI facts PLAN-twr.md § B4 measured: 11L/11R one wake group,
    /// west-flow 27/28 crossing, nothing else.
    #[test]
    fn igi_wake_links_are_the_measured_ones() {
        let ap = Airport::parse(crate::VIDP).unwrap();
        let links = wake_links(&runway_dirs(&ap));
        let mut got: Vec<String> = links.iter().map(|((a, b), l)| format!("{a}>{b} {l:?}")).collect();
        got.sort();
        assert_eq!(got, [
            "11L>11R Parallel", "11R>11L Parallel", "27>28 Crossing", "28>27 Crossing", "29L>29R Parallel", "29R>29L Parallel",
        ]);
    }
}
