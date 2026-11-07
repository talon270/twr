//! TWR-SIM · ROUTE
//!   · Graph: adjacency and integer edge lengths built once from an Airport
//!   · Request / plan: shortest taxi route with via and avoid edges
//!   · Route: the node and edge list a clearance hands to an aircraft
//!   · runway helpers: which runway a node is on, the takeoff run from an
//!     entry, the nearest runway entry from a holding point
//!
//! The planner runs once, when the controller builds a clearance. Its Route
//! is what the screen draws and, unchanged, what the aircraft receives. The
//! sim never plans: "the preview is the route" (PLAN-twr.md § Input) holds
//! because there is only one plan, not because two planners agree.
//!
//! Costs are integer decimetres and ties break on node index, so the same
//! request gives the same route on every platform.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::airport::{Airport, EdgeKind};

/// A taxi route. `nodes[i]` to `nodes[i + 1]` runs along `edges[i]`, so
/// `nodes.len() == edges.len() + 1`. A route with no edges is a parked
/// aircraft at `nodes[0]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub nodes: Vec<u32>,
    pub edges: Vec<u32>,
}

impl Route {
    pub fn at(node: u32) -> Route {
        Route { nodes: vec![node], edges: Vec::new() }
    }

    pub fn end(&self) -> u32 {
        *self.nodes.last().expect("a route has at least one node")
    }
}

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub from: u32,
    pub to: u32,
    /// Edges the route must use, in this order. Each is entered from
    /// whichever end is nearer.
    pub via: Vec<u32>,
    /// Edges the route must not use.
    pub avoid: Vec<u32>,
}

pub struct Graph {
    /// Per node: (edge, other node), taxi-usable edges only.
    pub(crate) adj: Vec<Vec<(u32, u32)>>,
    /// Per node: (edge, other node), runway edges only.
    rwy_adj: Vec<Vec<(u32, u32)>>,
    /// Per edge: length in decimetres, at least 1.
    pub len_dm: Vec<u32>,
    /// Per node: index into `Airport::runways` when the node lies on one.
    pub runway_at: Vec<Option<usize>>,
    /// Per node: an OSM holding point sits here.
    pub is_hold: Vec<bool>,
}

impl Graph {
    /// Runway edges are left out: a taxi route may cross a runway at a node
    /// but never roll along one. Backtracking is a runway clearance, not a
    /// taxi route, and arrives with the departure and arrival flows.
    pub fn new(ap: &Airport) -> Graph {
        let n = ap.nodes.len();
        let (mut adj, mut rwy_adj) = (vec![Vec::new(); n], vec![Vec::new(); n]);
        let mut runway_at = vec![None; n];
        let mut len_dm = Vec::with_capacity(ap.edges.len());
        for (i, e) in ap.edges.iter().enumerate() {
            let (a, b) = (ap.nodes[e.a as usize], ap.nodes[e.b as usize]);
            len_dm.push((f64::from(a.x - b.x).hypot(f64::from(a.y - b.y)).round() as u32).max(1));
            let side = if e.kind == EdgeKind::Runway { &mut rwy_adj } else { &mut adj };
            side[e.a as usize].push((i as u32, e.b));
            side[e.b as usize].push((i as u32, e.a));
            if e.kind == EdgeKind::Runway {
                let r = ap.runways.iter().position(|r| r.name == e.name);
                runway_at[e.a as usize] = r;
                runway_at[e.b as usize] = r;
            }
        }
        let mut is_hold = vec![false; n];
        for h in &ap.holds {
            is_hold[h.node as usize] = true;
        }
        Graph { adj, rwy_adj, len_dm, runway_at, is_hold }
    }

    /// Does `node` have any taxi-usable edge? Interior runway nodes do not.
    pub fn is_taxi_node(&self, node: u32) -> bool {
        !self.adj[node as usize].is_empty()
    }

    /// Dijkstra from `from` to `to`, skipping `avoid`. None when cut off.
    fn shortest(&self, from: u32, to: u32, avoid: &[u32]) -> Option<(u64, Route)> {
        self.shortest_where(from, |n| n == to, avoid, u64::MAX)
    }

    /// Shortest taxi route from `from` to the nearest runway node no more
    /// than `max_dm` away: how a holding point finds the runway it guards.
    pub fn nearest_runway(&self, from: u32, max_dm: u64) -> Option<Route> {
        self.shortest_where(from, |n| self.runway_at[n as usize].is_some(), &[], max_dm).map(|(_, r)| r)
    }

    /// Dijkstra from `from` to the first node `goal` accepts, within
    /// `max_dm`, skipping `avoid`. None when there is none.
    fn shortest_where(&self, from: u32, goal: impl Fn(u32) -> bool, avoid: &[u32], max_dm: u64) -> Option<(u64, Route)> {
        let n = self.adj.len();
        let mut dist = vec![u64::MAX; n];
        let mut back: Vec<Option<(u32, u32)>> = vec![None; n];
        let mut heap = BinaryHeap::new();
        dist[from as usize] = 0;
        heap.push(Reverse((0u64, from)));
        let mut to = None;
        while let Some(Reverse((d, u))) = heap.pop() {
            if d > max_dm {
                break;
            }
            if goal(u) {
                to = Some(u);
                break;
            }
            if d > dist[u as usize] {
                continue;
            }
            for &(e, v) in &self.adj[u as usize] {
                if avoid.contains(&e) {
                    continue;
                }
                let nd = d + u64::from(self.len_dm[e as usize]);
                if nd < dist[v as usize] {
                    dist[v as usize] = nd;
                    back[v as usize] = Some((e, u));
                    heap.push(Reverse((nd, v)));
                }
            }
        }
        let to = to?;
        let (mut nodes, mut edges, mut at) = (vec![to], Vec::new(), to);
        while let Some((e, prev)) = back[at as usize] {
            edges.push(e);
            nodes.push(prev);
            at = prev;
        }
        nodes.reverse();
        edges.reverse();
        Some((dist[to as usize], Route { nodes, edges }))
    }

    /// Plan a taxi route through `via` in order. Each via can be entered
    /// from either end; both choices are carried forward and the cheaper
    /// survives at each step (two states per via, so a handful of Dijkstra
    /// runs). The leg into a via may not use that via, and the leg out may
    /// not use the one just left, so a route never doubles back along a via.
    /// None when no route exists without a U-turn.
    pub fn plan(&self, ap: &Airport, req: &Request) -> Option<Route> {
        if req.via.iter().any(|&v| ap.edges[v as usize].kind == EdgeKind::Runway) {
            return None;
        }
        // Cheapest route so far ending at the exit of the last via, per entry end.
        let mut best: Vec<(u64, Route)> = vec![(0, Route::at(req.from))];
        let mut prev: Option<u32> = None;
        let extend = |best: &[(u64, Route)], to: u32, also_avoid: &[u32]| -> Option<(u64, Route)> {
            let mut avoid = req.avoid.clone();
            avoid.extend_from_slice(also_avoid);
            best.iter()
                .filter_map(|(c, r)| {
                    let (lc, leg) = self.shortest(r.end(), to, &avoid)?;
                    let mut r = r.clone();
                    r.append(&leg);
                    Some((c + lc, r))
                })
                .min_by_key(|(c, _)| *c)
        };
        for &v in &req.via {
            let e = &ap.edges[v as usize];
            let no_back: Vec<u32> = std::iter::once(v).chain(prev).collect();
            best = [(e.a, e.b), (e.b, e.a)]
                .into_iter()
                .filter_map(|(entry, exit)| {
                    let (c, mut r) = extend(&best, entry, &no_back)?;
                    r.edges.push(v);
                    r.nodes.push(exit);
                    Some((c + u64::from(self.len_dm[v as usize]), r))
                })
                .collect();
            prev = Some(v);
        }
        let (_, route) = extend(&best, req.to, &prev.into_iter().collect::<Vec<_>>())?;
        // Safety net: a reversal along one edge is a 180-degree turn no
        // airliner makes on a taxiway, so it is refused rather than drawn.
        if route.edges.windows(2).any(|w| w[0] == w[1]) {
            return None;
        }
        Some(route)
    }
}

/// The end of runway `name` (e.g. "11L/29R") whose number is nearest to
/// `bearing` (degrees true, as drawn): the designator for travel that way.
pub fn designator(name: &str, bearing: f64) -> Option<String> {
    let off = |d: &str| {
        let num: f64 = d.trim_end_matches(['L', 'R', 'C']).parse().unwrap_or(0.0);
        let diff = (num * 10.0 - bearing).rem_euclid(360.0);
        diff.min(360.0 - diff)
    };
    name.split('/').min_by(|x, y| off(x).total_cmp(&off(y))).map(str::to_string)
}

impl Graph {
    /// The takeoff run from runway node `at`: the runway edges to the far
    /// end in whichever direction is longer, and that direction's
    /// designator. Longer, because every metre ahead is runway available for
    /// the roll; the strip shows the result, so the choice is never hidden.
    pub fn runway_run(&self, ap: &Airport, at: u32) -> Option<(Route, String)> {
        let rwy = &ap.runways[self.runway_at[at as usize]?];
        let mut best: Option<(u64, Route)> = None;
        for &(e0, n0) in &self.rwy_adj[at as usize] {
            let mut r = Route { nodes: vec![at, n0], edges: vec![e0] };
            let mut len = u64::from(self.len_dm[e0 as usize]);
            // A runway is a chain: each node has at most one way onward.
            while let Some(&(e, n)) = self.rwy_adj[r.end() as usize].iter().find(|(e, _)| Some(e) != r.edges.last()) {
                r.edges.push(e);
                r.nodes.push(n);
                len += u64::from(self.len_dm[e as usize]);
            }
            if best.as_ref().is_none_or(|(b, _)| len > *b) {
                best = Some((len, r));
            }
        }
        let (_, run) = best?;
        let (a, b) = (ap.nodes[run.nodes[0] as usize], ap.nodes[run.end() as usize]);
        let bearing = f64::from(b.x - a.x).atan2(f64::from(b.y - a.y)).to_degrees().rem_euclid(360.0);
        Some((run, designator(&rwy.name, bearing)?))
    }

    pub fn run_len_dm(&self, r: &Route) -> u64 {
        r.edges.iter().map(|&e| u64::from(self.len_dm[e as usize])).sum()
    }
}

impl Route {
    fn append(&mut self, leg: &Route) {
        debug_assert_eq!(self.end(), leg.nodes[0]);
        self.nodes.extend_from_slice(&leg.nodes[1..]);
        self.edges.extend_from_slice(&leg.edges);
    }

    /// Every edge joins the nodes either side of it, and none is a runway.
    /// The sim checks this before accepting a clearance.
    pub fn check(&self, ap: &Airport) -> Result<(), String> {
        if self.nodes.len() != self.edges.len() + 1 {
            return Err(format!("{} nodes for {} edges", self.nodes.len(), self.edges.len()));
        }
        for (i, &ei) in self.edges.iter().enumerate() {
            let e = ap.edges.get(ei as usize).ok_or(format!("edge {ei} does not exist"))?;
            let (p, q) = (self.nodes[i], self.nodes[i + 1]);
            if !((e.a == p && e.b == q) || (e.a == q && e.b == p)) {
                return Err(format!("edge {ei} does not join nodes {p} and {q}"));
            }
            if e.kind == EdgeKind::Runway {
                return Err(format!("edge {ei} is runway {}", e.name));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rng, VIDP};

    fn vidp() -> (Airport, Graph) {
        let ap = Airport::parse(VIDP).unwrap();
        let g = Graph::new(&ap);
        (ap, g)
    }

    #[test]
    fn every_stand_to_every_kind_of_runway_entry_plans_a_valid_route() {
        let (ap, g) = vidp();
        let exits = ap.runway_exits();
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            let s = &ap.stands[(rng.next_u64() % ap.stands.len() as u64) as usize];
            let to = exits[(rng.next_u64() % exits.len() as u64) as usize];
            let r = g.plan(&ap, &Request { from: s.node, to, ..Default::default() }).expect("route");
            r.check(&ap).unwrap();
            assert_eq!((r.nodes[0], r.end()), (s.node, to));
        }
    }

    #[test]
    fn via_is_used_and_avoid_is_respected() {
        let (ap, g) = vidp();
        let from = ap.stands[0].node;
        let to = ap.runway_exits()[0];
        let direct = g.plan(&ap, &Request { from, to, ..Default::default() }).unwrap();
        // Avoid the middle edge of the direct route: the new one differs and
        // never touches it.
        let mid = direct.edges[direct.edges.len() / 2];
        let around = g.plan(&ap, &Request { from, to, avoid: vec![mid], ..Default::default() }).unwrap();
        assert!(!around.edges.contains(&mid));
        around.check(&ap).unwrap();
        // Force a via far off the direct route: it appears, in place.
        // Every off-route taxiway edge as a via. Refusal is allowed only for
        // a bridge: an edge whose removal cuts the taxi network, the only
        // place a route can be forced to come back the way it went.
        let base = ap.components(|e| e.kind != EdgeKind::Runway);
        let parts = |c: &[u32]| c.iter().collect::<std::collections::HashSet<_>>().len();
        let (mut planned, mut refused) = (0, 0);
        for off in (0..ap.edges.len() as u32).filter(|e| ap.edges[*e as usize].kind == EdgeKind::Taxiway && !direct.edges.contains(e)) {
            match g.plan(&ap, &Request { from, to, via: vec![off], ..Default::default() }) {
                Some(via) => {
                    assert!(via.edges.contains(&off));
                    via.check(&ap).unwrap();
                    planned += 1;
                }
                None => {
                    let cut = ap.components(|e| e.kind != EdgeKind::Runway && !std::ptr::eq(e, &ap.edges[off as usize]));
                    assert!(parts(&cut) > parts(&base), "via {off} refused but is not a bridge");
                    refused += 1;
                }
            }
        }
        println!("vias: {planned} planned, {refused} refused (all bridges)");
        assert!(planned > refused, "{planned} planned, {refused} refused");
        // A runway is never a via.
        let rwy = ap.edges.iter().position(|e| e.kind == EdgeKind::Runway).unwrap() as u32;
        assert!(g.plan(&ap, &Request { from, to, via: vec![rwy], ..Default::default() }).is_none());
    }

    #[test]
    fn a_via_that_needs_a_u_turn_is_refused() {
        let (ap, g) = vidp();
        // A dead-end taxiway edge: one end has no other taxi edge.
        let deg = |n: u32| ap.edges.iter().filter(|e| e.kind == EdgeKind::Taxiway && (e.a == n || e.b == n)).count();
        let dead = (0..ap.edges.len() as u32)
            .find(|&i| { let e = &ap.edges[i as usize]; e.kind == EdgeKind::Taxiway && (deg(e.a) == 1 || deg(e.b) == 1)
                && ap.stands.iter().all(|s| s.node != e.a && s.node != e.b) })
            .unwrap();
        let req = Request { from: ap.stands[0].node, to: ap.runway_exits()[0], via: vec![dead], ..Default::default() };
        assert!(g.plan(&ap, &req).is_none());
    }

    #[test]
    fn check_refuses_a_broken_or_runway_route() {
        let (ap, g) = vidp();
        let r = g.plan(&ap, &Request { from: ap.stands[0].node, to: ap.stands[50].node, ..Default::default() }).unwrap();
        let mut gap = r.clone();
        gap.edges.swap(0, 1);
        assert!(gap.check(&ap).unwrap_err().contains("does not join"));
        let rwy = ap.edges.iter().position(|e| e.kind == EdgeKind::Runway).unwrap();
        let e = &ap.edges[rwy];
        let on_rwy = Route { nodes: vec![e.a, e.b], edges: vec![rwy as u32] };
        assert!(on_rwy.check(&ap).unwrap_err().contains("is runway"));
    }
}
