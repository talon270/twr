//! TWR-SIM · ARRIVAL
//!   · constants: where arrivals appear, the decision point, the circuit
//!     after a go-around, the pilot go-around draw (PLAN-twr.md § B5)
//!   · threshold: the runway end a landing direction starts from
//!   · exits: every taxiway leaving one landing direction, each with its
//!     vacate route, in order down the runway
//!   · GoAround: why an aircraft went around, and whether it counts as a
//!     challenge (pilot, weather) or as delay you caused
//!
//! A landing route is one `Route`: the runway from the threshold to the
//! exit node, then the exit taxiway to the vacate point. The aircraft rolls
//! it like any other route, so the screen, the trail and the conflict
//! monitor all read the same thing.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::aircraft::{HOLD_SHORT_M, LAND_DECEL_MDM, LAND_MDM_PER_TICK, MDM_PER_M, TAXI_MDM_PER_TICK};
use crate::airport::{Airport, EdgeKind};
use crate::route::{Graph, Route};

/// Where an arrival appears: 8 km before the threshold. At 70 m/s that is
/// 114 game s, 38 real s at 3x, to give C. `authored`.
pub const FINAL_SPAWN_M: u64 = 8_000;
/// No landing clearance by here and the aircraft goes around: 200 ft, the
/// CAT I decision height, on a 3° path is 61 m / tan 3° = 1,160 m.
/// `authored` from those two nominal values.
pub const DECISION_M: u64 = 1_160;
/// Game seconds from leaving the runway's far end on a go-around to being
/// back at FINAL_SPAWN_M: 6 minutes, 2 real minutes. `authored`.
pub const GA_CIRCUIT_S: u64 = 360;
/// Share of arrivals whose pilot goes around on their own, in percent.
/// Real rates are a few per 1,000 approaches, too rare to be a game event;
/// 5% makes it one about every 20 arrivals. `authored`.
pub const PILOT_GA_PCT: u64 = 5;
/// A pilot go-around happens somewhere from this far out to the threshold.
pub const PILOT_GA_MAX_M: u64 = 3_000;
/// An exit whose first segment turns more than this off the landing
/// direction points back up the runway: IGI's reverse-angled exits sit at
/// 155-177°, the right-angle ones at 85-91°. `authored`.
pub const EXIT_MAX_TURN_DEG: f64 = 95.0;

/// What a pilot gives as the reason, drawn from the seeded stream.
pub const PILOT_REASONS: [&str; 3] = ["unstable approach", "bird strike", "long landing"];

/// Why an aircraft went around. The Debug text is shown and read by tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoAround {
    /// The pilot chose to: a challenge, never counted against you.
    Pilot(&'static str),
    /// B9 brings weather; the variant exists so the score never has to
    /// tell two kinds of challenge apart.
    Weather(&'static str),
    /// You pressed G.
    Instructed,
    /// No landing clearance by the decision point.
    NoClearance,
    /// No landing clearance by 2 NM under LVP because the localizer
    /// sensitive area was occupied (PLAN-lvp.md § A3). Withholding C was
    /// the rule, so it is neither delay you caused nor a challenge; the
    /// lost movement still shows in the score.
    LsaOccupied,
}

impl GoAround {
    /// Pilot and weather go-arounds are challenges; the other two are
    /// delay you caused. Neither kind is an incident.
    pub fn challenge(self) -> bool {
        matches!(self, GoAround::Pilot(_) | GoAround::Weather(_))
    }

    pub fn text(self) -> String {
        match self {
            GoAround::Pilot(r) => format!("pilot: {r}"),
            GoAround::Weather(r) => format!("weather: {r}"),
            GoAround::Instructed => "instructed".into(),
            GoAround::NoClearance => "no landing clearance".into(),
            GoAround::LsaOccupied => "LSA occupied".into(),
        }
    }
}

/// Where a landing leaves its runway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    /// The runway node the exit taxiway leaves from.
    pub node: u32,
    /// Taxiway name; empty when OSM leaves it unnamed.
    pub name: String,
    /// Route distance from the threshold to `node`.
    pub at_mdm: u64,
    /// Route distance to the vacate point, which is the route's end.
    pub vacate_mdm: u64,
    /// Runway length this way, threshold to far end: how far a go-around
    /// flies over it.
    pub run_mdm: u64,
}

impl Exit {
    /// For the strip: the taxiway name, or the distance when unnamed.
    pub fn label(&self) -> String {
        if self.name.is_empty() { format!("at {} m", self.at_mdm / MDM_PER_M) } else { self.name.clone() }
    }
}

/// Braking distance from `v` to exit speed at LAND_DECEL_MDM, mdm.
pub fn brake_mdm(v: u64) -> u64 {
    let ve = TAXI_MDM_PER_TICK;
    if v <= ve { 0 } else { (v * v - ve * ve) / (2 * LAND_DECEL_MDM) }
}

/// The runway end that landing direction `desig` starts from, and the run
/// from it to the far end.
pub fn threshold(ap: &Airport, g: &Graph, desig: &str) -> Option<(u32, Route)> {
    ap.runways
        .iter()
        .filter(|r| r.name.split('/').any(|d| d == desig))
        .flat_map(|r| r.ends)
        .find_map(|end| g.runway_run(ap, end).filter(|(_, d)| d == desig).map(|(run, _)| (end, run)))
}

/// One candidate exit: the runway node, the taxiway edge leaving it, and
/// its turn off the landing direction. Reverse ones are kept here so a
/// refusal can say why.
pub struct Candidate {
    pub node: u32,
    pub edge: u32,
    pub turn_deg: f64,
    pub at_mdm: u64,
    k: usize,
    next: u32,
}

/// Every taxiway edge leaving the run of `desig`, past the threshold, in
/// order down the runway.
pub fn candidates(ap: &Airport, g: &Graph, desig: &str) -> Vec<Candidate> {
    let Some((_, run)) = threshold(ap, g, desig) else { return Vec::new() };
    let p = |n: u32| (f64::from(ap.nodes[n as usize].x), f64::from(ap.nodes[n as usize].y));
    let (a, b) = (p(run.nodes[0]), p(run.end()));
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let mut out = Vec::new();
    let mut cum = 0u64;
    for k in 1..run.nodes.len() {
        cum += u64::from(g.len_dm[run.edges[k - 1] as usize]) * 1000;
        let n = run.nodes[k];
        for &(e, next) in &g.adj[n as usize] {
            if ap.edges[e as usize].kind != EdgeKind::Taxiway {
                continue;
            }
            let (q, r) = (p(n), p(next));
            let v = (r.0 - q.0, r.1 - q.1);
            let vl = v.0.hypot(v.1);
            if vl == 0.0 {
                continue;
            }
            let turn_deg = ((v.0 * u.0 + v.1 * u.1) / vl).clamp(-1.0, 1.0).acos().to_degrees();
            out.push(Candidate { node: n, edge: e, turn_deg, at_mdm: cum, k, next });
        }
    }
    out
}

/// Shortest taxiway path from runway node `from` out along edge `e0` to the
/// first node more than HOLD_SHORT_M away, meeting no runway on the way.
/// More than, not equal: at exactly the hold distance the aircraft would
/// still count as on the runway (`Crossing::exit`).
fn vacate(ap: &Airport, g: &Graph, from: u32, e0: u32, n1: u32) -> Option<Route> {
    if g.runway_at[n1 as usize].is_some() {
        return None;
    }
    let need = HOLD_SHORT_M * 10;
    let mut dist = vec![u64::MAX; ap.nodes.len()];
    let mut prev: Vec<Option<(u32, u32)>> = vec![None; ap.nodes.len()];
    let mut heap = BinaryHeap::new();
    dist[n1 as usize] = u64::from(g.len_dm[e0 as usize]);
    heap.push(Reverse((dist[n1 as usize], n1)));
    while let Some(Reverse((d, n))) = heap.pop() {
        if d > dist[n as usize] {
            continue;
        }
        if d > need {
            let mut nodes = vec![n];
            let mut edges = Vec::new();
            let mut at = n;
            while let Some((e, p)) = prev[at as usize] {
                edges.push(e);
                nodes.push(p);
                at = p;
            }
            edges.push(e0);
            nodes.push(from);
            nodes.reverse();
            edges.reverse();
            return Some(Route { nodes, edges });
        }
        for &(e, m) in &g.adj[n as usize] {
            if ap.edges[e as usize].kind != EdgeKind::Taxiway || g.runway_at[m as usize].is_some() || m == from {
                continue;
            }
            let nd = d + u64::from(g.len_dm[e as usize]);
            if nd < dist[m as usize] {
                dist[m as usize] = nd;
                prev[m as usize] = Some((e, n));
                heap.push(Reverse((nd, m)));
            }
        }
    }
    None
}

/// Every usable exit off `desig` with its whole landing route, in order
/// down the runway. Usable: turns EXIT_MAX_TURN_DEG or less and has a
/// vacate route. Braking room is the caller's question, since it depends
/// on where the aircraft is and how fast.
pub fn exits(ap: &Airport, g: &Graph, desig: &str) -> Vec<(Exit, Route)> {
    let Some((_, run)) = threshold(ap, g, desig) else { return Vec::new() };
    let run_mdm = g.run_len_dm(&run) * 1000;
    let mut out = Vec::new();
    for c in candidates(ap, g, desig) {
        if c.turn_deg > EXIT_MAX_TURN_DEG {
            continue;
        }
        let Some(v) = vacate(ap, g, c.node, c.edge, c.next) else { continue };
        let mut route = Route { nodes: run.nodes[..=c.k].to_vec(), edges: run.edges[..c.k].to_vec() };
        route.nodes.extend_from_slice(&v.nodes[1..]);
        route.edges.extend_from_slice(&v.edges);
        let vacate_mdm = c.at_mdm + g.run_len_dm(&v) * 1000;
        let name = ap.edges[c.edge as usize].name.clone();
        out.push((Exit { node: c.node, name, at_mdm: c.at_mdm, vacate_mdm, run_mdm }, route));
    }
    out
}

/// The exit a pilot takes with no instruction: the first one far enough
/// down to slow from touchdown speed.
pub fn default_exit(ap: &Airport, g: &Graph, desig: &str) -> Option<(Exit, Route)> {
    let need = brake_mdm(LAND_MDM_PER_TICK);
    exits(ap, g, desig).into_iter().find(|(x, _)| x.at_mdm >= need)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;

    #[test]
    fn braking_70_to_10_takes_1500_m() {
        assert_eq!(brake_mdm(LAND_MDM_PER_TICK), 1_500 * MDM_PER_M);
    }

    /// Every landing direction at IGI has a default exit, and every usable
    /// exit's route leaves the runway and meets no other.
    #[test]
    fn every_direction_has_exits_that_vacate_cleanly() {
        let w = World::new(1);
        let (ap, g) = (&w.airport, &w.graph);
        let mut report = Vec::new();
        for r in &ap.runways {
            for d in r.name.split('/') {
                let all = exits(ap, g, d);
                let (x, _) = default_exit(ap, g, d).unwrap_or_else(|| panic!("{d}: no default exit"));
                report.push(format!("{d}: {} usable, default {} ({} m)", all.len(), x.label(), x.at_mdm / MDM_PER_M));
                let rwy = g.runway_at[threshold(ap, g, d).unwrap().0 as usize];
                for (x, route) in &all {
                    let k = route.nodes.iter().position(|&n| n == x.node).unwrap();
                    assert!(route.nodes[..=k].iter().all(|&n| g.runway_at[n as usize] == rwy), "{d}: run part");
                    assert!(route.nodes[k + 1..].iter().all(|&n| g.runway_at[n as usize].is_none()), "{d} exit {}: meets a runway", x.label());
                    assert!(x.vacate_mdm - x.at_mdm > HOLD_SHORT_M * MDM_PER_M);
                    assert_eq!(x.vacate_mdm, g.run_len_dm(route) * 1000);
                }
            }
        }
        println!("{}", report.join("\n"));
    }
}
