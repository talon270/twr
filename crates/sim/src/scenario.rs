//! TWR-SIM · SCENARIO
//!   · finders: a runway-free taxi route, a runway entry by designator, a
//!     route that crosses a given runway
//!   · World::scenario: named set-ups that put a conflict one key-press
//!     away, shared by the B4 tests and the page's test hook
//!
//! Finders search IGI's graph in a fixed order, so a scenario is the same
//! on every run and every platform.

use crate::aircraft::{hold_limits, Aircraft, Phase, Wake, MIN_TAKEOFF_RUN_M};
use crate::airport::EdgeKind;
use crate::conflict::INTERMEDIATE_M;
use crate::route::{Request, Route};
use crate::World;

/// Taxiway nodes that stands link to, in stand order: the starting points
/// every finder searches from.
fn stand_junctions(w: &World) -> Vec<u32> {
    let mut out = Vec::new();
    for s in &w.airport.stands {
        if let Some(e) = w.airport.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s.node || e.b == s.node)) {
            let j = if e.a == s.node { e.b } else { e.a };
            if !out.contains(&j) {
                out.push(j);
            }
        }
    }
    out
}

pub fn route_len_m(w: &World, r: &Route) -> f64 {
    r.edges.iter().map(|&e| f64::from(w.graph.len_dm[e as usize])).sum::<f64>() / 10.0
}

/// A taxiway-only route at least `min_m` long that meets no runway.
pub fn taxi_route(w: &World, min_m: f64) -> Option<Route> {
    let js = stand_junctions(w);
    for &a in &js {
        for &b in js.iter().rev() {
            let Some(r) = w.graph.plan(&w.airport, &Request { from: a, to: b, ..Default::default() }) else { continue };
            let clean = r.nodes.iter().all(|&n| w.graph.runway_at[n as usize].is_none())
                && r.edges.iter().all(|&e| w.airport.edges[e as usize].kind == EdgeKind::Taxiway);
            if clean && route_len_m(w, &r) >= min_m {
                return Some(r);
            }
        }
    }
    None
}

/// A runway entry whose takeoff run goes down `desig`. `full`: the one
/// nearest the runway's start; otherwise an intermediate one, more than
/// INTERMEDIATE_M along, with at least MIN_TAKEOFF_RUN_M ahead (an M's run;
/// every scenario aircraft is an M).
pub fn entry(w: &World, desig: &str, full: bool) -> Option<u32> {
    let dir = w.monitor.dirs.iter().find(|d| d.desig == desig)?;
    let mut best: Option<(f64, u32)> = None;
    for at in w.airport.runway_exits() {
        let Some((run, d)) = w.graph.runway_run(&w.airport, at) else { continue };
        if d != desig || w.graph.run_len_dm(&run) / 10 < MIN_TAKEOFF_RUN_M {
            continue;
        }
        let p = w.airport.nodes[at as usize];
        let along = (f64::from(p.x) / 10.0 - dir.from.0) * dir.u.0 + (f64::from(p.y) / 10.0 - dir.from.1) * dir.u.1;
        if !full && along <= INTERMEDIATE_M + 50.0 {
            continue;
        }
        // Full: least distance along; intermediate: the first one past the line.
        if best.is_none_or(|(b, _)| along < b) {
            best = Some((along, at));
        }
    }
    best.filter(|&(along, _)| !full || along < INTERMEDIATE_M).map(|(_, at)| at)
}

/// A route from a stand junction toward a runway exit whose first runway
/// is `desig`'s, crossed and left behind, with the crossing more than
/// `beyond_m` along `desig`. Stand-to-stand routes at IGI rarely cross a
/// runway; departure routes do.
pub fn crossing(w: &World, desig: &str, beyond_m: f64) -> Option<Route> {
    let dir = w.monitor.dirs.iter().find(|d| d.desig == desig)?;
    let js = stand_junctions(w);
    let exits = w.airport.runway_exits();
    for &a in &js {
        for &b in &exits {
            let Some(r) = w.graph.plan(&w.airport, &Request { from: a, to: b, ..Default::default() }) else { continue };
            let lims = hold_limits(&w.airport, &w.graph, &r, 0, 0);
            let Some((lim, name)) = lims.first() else { continue };
            if *name != w.airport.runways[dir.rwy].name || *lim == 0 {
                continue;
            }
            let k = r.nodes.iter().position(|&n| w.graph.runway_at[n as usize] == Some(dir.rwy)).expect("route meets it");
            if k + 1 == r.nodes.len() {
                continue;
            }
            let p = w.airport.nodes[r.nodes[k] as usize];
            let along = (f64::from(p.x) / 10.0 - dir.from.0) * dir.u.0 + (f64::from(p.y) / 10.0 - dir.from.1) * dir.u.1;
            if along > beyond_m {
                return Some(r);
            }
        }
    }
    None
}

impl World {
    /// Add an aircraft; returns its index.
    pub fn add(&mut self, a: Aircraft) -> usize {
        self.aircraft.push(a);
        self.aircraft.len() - 1
    }

    /// An aircraft lined up at `entry`, as if it had taxied there.
    pub fn add_lined_up(&mut self, callsign: &str, wake: Wake, entry: u32) -> Result<usize, String> {
        let i = self.add(Aircraft { wake, phase: Phase::Holding, ..Aircraft::parked(callsign, entry) });
        self.line_up(i)?;
        Ok(i)
    }

    /// An aircraft cleared along `route` from its first node, stepped
    /// (with the whole world) until it stops.
    pub fn add_taxied(&mut self, callsign: &str, route: Route) -> Result<usize, String> {
        let i = self.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked(callsign, route.nodes[0]) });
        self.taxi(i, route)?;
        while matches!(self.aircraft[i].phase, Phase::Taxi) {
            self.step_ticks(1);
        }
        Ok(i)
    }

    /// Named set-ups for the page's test hook: each leaves one conflict a
    /// key-press away. The world's own aircraft are replaced.
    ///   ground: IGO602 taxis toward IGO601, stopped 500 m ahead (H holds it)
    ///   wake:   AIC101 (H) has just departed 29R; IGO512 (M) lined up behind
    ///   runway: IGO512 lined up on 29R; IGO701 held short of a crossing ahead
    ///   arrival: the page's own world, with IGO713's pilot go-around taken
    ///     out, so a script can fly it from final to stand every time
    ///   pilot: the same, with IGO713's pilot going around 2 km out
    ///   threshold: IGO512 lined up on 29L; IGO713 2 km out on final for
    ///     29L, not cleared (C lands it over IGO512: R5)
    pub fn scenario(seed: u64, name: &str) -> Option<World> {
        let mut w = World::new(seed);
        if name == "arrival" || name == "pilot" {
            let i = w.aircraft.iter().position(|a| a.callsign == "IGO713")?;
            w.aircraft[i].pilot_ga = (name == "pilot").then_some((2_000 * crate::aircraft::MDM_PER_M, "unstable approach"));
            return Some(w);
        }
        w.aircraft.clear();
        match name {
            "threshold" => {
                w.add_lined_up("IGO512", Wake::M, entry(&w, "29L", true)?).ok()?;
                let i = w.spawn_arrival("IGO713", Wake::M, "29L")?;
                w.aircraft[i].pilot_ga = None;
                w.aircraft[i].air_mdm = -2_000 * crate::aircraft::MDM_PER_M as i64;
            }
            "ground" => {
                let r = taxi_route(&w, 900.0)?;
                // Stop the leader at the first node 500 m or more along.
                let mut s = 0.0;
                let k = (0..r.edges.len()).find(|&k| {
                    s += f64::from(w.graph.len_dm[r.edges[k] as usize]) / 10.0;
                    s >= 500.0
                })? + 1;
                let lead = Route { nodes: r.nodes[..=k].to_vec(), edges: r.edges[..k].to_vec() };
                w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked("IGO601", lead.end()) });
                let i = w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked("IGO602", r.nodes[0]) });
                w.taxi(i, r).ok()?;
            }
            "wake" => {
                let e = entry(&w, "29R", true)?;
                let h = w.add_lined_up("AIC101", Wake::H, e).ok()?;
                w.takeoff(h).ok()?;
                while !matches!(w.aircraft[h].phase, Phase::Departed(_)) {
                    w.step_ticks(1);
                }
                w.add_lined_up("IGO512", Wake::M, e).ok()?;
            }
            "runway" => {
                let e = entry(&w, "29R", true)?;
                let r = crossing(&w, "29R", 600.0)?;
                w.add_taxied("IGO701", r).ok()?;
                w.add_lined_up("IGO512", Wake::M, e).ok()?;
            }
            _ => return None,
        }
        Some(w)
    }
}
