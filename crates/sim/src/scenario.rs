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
    ///   lsa: LVP on; IGO713 landed on 29L and held at Z2, inside the 29L
    ///     localizer sensitive area; IGO714 on final for 29L, not cleared
    ///     (PLAN-lvp.md step 8)
    pub fn scenario(seed: u64, name: &str) -> Option<World> {
        if name == "c3-give-way" { return c3_give_way(); }
        if name == "c3-cross" { return c3_cross(); }
        if name == "c3-backtrack" { return c3_backtrack(); }
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
                w.auto_stop = false;
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
            "lsa" => {
                w.set_lvp(true);
                let a = w.spawn_arrival("IGO713", Wake::M, "29L")?;
                w.aircraft[a].pilot_ga = None;
                w.clear_to_land(a).ok()?;
                while w.aircraft[a].phase != Phase::Holding {
                    w.step_ticks(1);
                }
                let b = w.spawn_arrival("IGO714", Wake::M, "29L")?;
                w.aircraft[b].pilot_ga = None;
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


/// C3 browser fixture uses the same graph search as the junction regression.
fn c3_give_way() -> Option<World> {
    use crate::aircraft::TAXI_MDM_PER_TICK;
    let base = World::empty(1);
    let (ap, g) = (&base.airport, &base.graph);
    let taxi_edges = |n: u32| -> Vec<(u32, u32)> {
        if g.runway_at[n as usize].is_some() {
            return Vec::new();
        }
        let mut v: Vec<(u32, u32)> = ap
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EdgeKind::Taxiway && (e.a == n || e.b == n))
            .map(|(k, e)| (k as u32, if e.a == n { e.b } else { e.a }))
            .filter(|&(_, v)| g.runway_at[v as usize].is_none())
            .collect();
        v.sort();
        v
    };
    // Out from `n` along edge `e`, then on along the lowest-numbered taxiway
    // edge each time, never revisiting a node, until `min_m` is covered.
    let walk = |n: u32, e: u32, min_m: u64| -> Option<Route> {
        let first = taxi_edges(n).into_iter().find(|x| x.0 == e)?;
        let mut r = Route { nodes: vec![n, first.1], edges: vec![e] };
        while g.run_len_dm(&r) < min_m * 10 {
            let next = taxi_edges(r.end()).into_iter().find(|(_, v)| !r.nodes.contains(v))?;
            r.edges.push(next.0);
            r.nodes.push(next.1);
        }
        Some(r)
    };
    let join = |into: &Route, out: &Route| {
        let mut nodes: Vec<u32> = into.nodes.iter().rev().copied().collect();
        nodes.extend_from_slice(&out.nodes[1..]);
        let mut edges: Vec<u32> = into.edges.iter().rev().copied().collect();
        edges.extend_from_slice(&out.edges);
        Route { nodes, edges }
    };
    // A three-way junction: IGO601 in by e1, IGO602 in by e2, both out by e3.
    // IGO601 stops at least 150 m on; IGO602 goes 150 m further, so it is
    // never parked in front of IGO601.
    let (ra, rb) = (0..ap.nodes.len() as u32)
        .find_map(|n| {
            let out = taxi_edges(n);
            if out.len() < 3 {
                return None;
            }
            let (e1, e2, e3) = (out[0].0, out[1].0, out[2].0);
            let (a_in, b_in) = (walk(n, e1, 120)?, walk(n, e2, 120)?);
            let a_out = walk(n, e3, 150)?;
            let b_out = walk(n, e3, g.run_len_dm(&a_out) / 10 + 150)?;
            let (ra, rb) = (join(&a_in, &a_out), join(&b_in, &b_out));
            let mut all: Vec<u32> = ra.nodes.iter().chain(&b_in.nodes[1..]).copied().collect();
            all.sort();
            let unique = all.windows(2).all(|w| w[0] != w[1]) && rb.check(ap).is_ok() && ra.check(ap).is_ok();
            unique.then_some((ra, rb))
        })
        .expect("IGI has a three-way taxiway junction with 120 m of taxiway on each side");
    // The in-walks share no node, so the first node the routes share is it.
    let junction = *ra.nodes.iter().find(|n| rb.nodes.contains(n)).unwrap();
    let ka = ra.nodes.iter().position(|&n| n == junction).unwrap();
    let kb = rb.nodes.iter().position(|&n| n == junction).unwrap();
    // The farther one starts first, so both reach the junction on one tick.
    let ticks = |r: &Route, k: usize| (g.run_len_dm(&Route { nodes: r.nodes[..=k].to_vec(), edges: r.edges[..k].to_vec() }) * 1000).div_ceil(TAXI_MDM_PER_TICK) as u32;
    let (ta, tb) = (ticks(&ra, ka), ticks(&rb, kb));
    let set_up = || {
        let mut w = World::empty(1);
        let a = w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked("IGO601", ra.nodes[0]) });
        let b = w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked("IGO602", rb.nodes[0]) });
        let ((f, rf), (sc, rs)) = if ta >= tb { ((a, &ra), (b, &rb)) } else { ((b, &rb), (a, &ra)) };
        w.taxi(f, rf.clone()).unwrap();
        w.step_ticks(ta.abs_diff(tb));
        w.taxi(sc, rs.clone()).unwrap();
        (w, a, b)
    };

    Some(set_up().0)
}

/// The first runway is crossed; the route ends on a different runway.
fn c3_cross() -> Option<World> {
    let mut w = World::empty(1);
    let (ap, g) = (&w.airport, &w.graph);
    let (from, r) = ap.stands.iter().flat_map(|s| ap.runway_exits().into_iter().map(move |to| (s.node, to))).find_map(|(from, to)| {
        let r = g.plan(ap, &Request { from, to, ..Default::default() })?;
        let l = hold_limits(ap, g, &r, 0, 0);
        (l.len() == 2 && l[0].1 != l[1].1).then_some((from, r))
    })?;
    let i = w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked("IGO520", from) });
    w.taxi(i, r).ok()?;
    Some(w)
}

/// Entry 5 leaves too little of 27 ahead; backtrack reaches its full length.
fn c3_backtrack() -> Option<World> {
    let mut w = World::empty(1);
    let (ap, g) = (&w.airport, &w.graph);
    let r = ap.stands.iter().find_map(|s| {
        let r = g.plan(ap, &Request { from: s.node, to: 5, ..Default::default() })?;
        (hold_limits(ap, g, &r, 0, 0).len() == 1).then_some(r)
    })?;
    let i = w.add(Aircraft { phase: Phase::Holding, assigned: Some("27"), ..Aircraft::parked("IGO530", r.nodes[0]) });
    w.taxi(i, r).ok()?;
    while w.aircraft[i].phase == Phase::Taxi { w.step_ticks(1); }
    Some(w)
}
