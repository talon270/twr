//! TWR-SIM · LVP
//!   · constants: the LVP numbers, each with its grade (PLAN-lvp.md § B)
//!   · Lsa: the localizer sensitive area of each landing end, from
//!     data/vidp.lsa.csv; `contains` asks whether a point is inside one
//!   · taxiways: the taxiway names the AIP's LVP routes use, per flow, from
//!     data/vidp.lvp.csv
//!
//! Low-visibility procedures are a shift-level flag (PLAN-lvp.md § A1). This
//! module holds the geometry and the published lists; the rules that read
//! them live with the rules they change (conflict, score, route).

use std::collections::BTreeMap;

use crate::conflict::Dir;
use crate::weather::{Flow, Wind};
use crate::Rng;

// ---- Constants (PLAN-lvp.md § B) -------------------------------------------

/// V1: no landing clearance by 2 NM and the arrival goes around. `primary`,
/// AD 2.22 VIII 4.2.1.
pub const LVP_DECISION_M: u64 = 3_704;
/// Share of eligible shifts that are foggy, percent. `authored`: Delhi's
/// fog season is 15 Dec-15 Feb (AD 2.22 VIII preamble), and the game has no
/// date.
pub const LVP_PCT: u64 = 10;
/// Fog needs light, steady wind: no shift over this or gusting gets LVP.
/// `authored`, from memory, unconfirmed (radiation fog). Below CALM_KT (5)
/// IGI keeps west flow, so most fog shifts are west.
pub const LVP_MAX_WIND_KT: u8 = 6;
/// A2: least spacing on final under LVP, NM. Measured by
/// `tests/lvp.rs::measure_lsa_clear_times` (2026-11-10): the slowest
/// landing clears its LSA 3,133 ticks after the threshold (28, taxied on
/// the tick it stops, so a floor), and a follower spawned at FINAL_SPAWN_M
/// reaches 2 NM 1,228 ticks after it appears. No follower may be on final
/// while its leader still has to clear, so 5 NM: the first whole NM past
/// FINAL_SPAWN_M (4.32 NM), which keeps the next arrival off final until
/// the leader is over the threshold. `World::final_free` then holds it
/// until the LSA is empty. One arrival per landing runway at a time.
pub const LVP_FINAL_MIN_NM: u64 = 5;
/// V4: a named taxiway off the flow's published LVP routes costs this many
/// times its length in the planner. `authored`: high enough that the
/// preview follows the published route where one exists within 4x the
/// length, low enough that the player's `via` still works. A cost, not an
/// avoid list: AD 2.22 VIII 4.3.2 allows alternative taxiways.
pub const LVP_OFF_ROUTE_COST: u64 = 4;
/// Salt for the fog stream: its own draw, so every seed's wind, schedule
/// and stands stay byte-identical to the clear-weather build.
const LVP_SALT: u64 = 0xF06_F06F_06F0_0001;

/// Does a shift on `seed` with `wind` run under LVP? One draw, always made.
pub fn draw(seed: u64, wind: Wind) -> bool {
    let p = Rng::new(seed ^ LVP_SALT).next_u64() % 100;
    p < LVP_PCT && wind.gust.is_none() && wind.kt <= LVP_MAX_WIND_KT
}

/// The localizer sensitive areas, extracted from the six AD 2.24 charts by
/// `research/work/vidp/lsa_from_charts.py` (vector paths, scaled to the OSM
/// runway; `research/work/vidp/lsa_from_charts.md` has the cross-checks).
pub const LSA_CSV: &str = include_str!("../../../data/vidp.lsa.csv");
/// Taxiways named in AD 2.22 VIII 4.4.4 Tables 2 and 3, by
/// `research/work/vidp/lvp_routes_from_aip.py`.
pub const ROUTES_CSV: &str = include_str!("../../../data/vidp.lvp.csv");

/// One cut across an LSA: metres along the runway axis from the end the
/// arrival crosses first (`Dir::from`), and the edge's distance left and
/// right of the centreline as the landing aircraft sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Station {
    pub along: f64,
    pub left: f64,
    pub right: f64,
}

/// Every landing end's LSA, keyed by designator ("29L"). Straight edges
/// between stations, as the charts draw them.
#[derive(Debug, Clone, Default)]
pub struct Lsa(pub BTreeMap<String, Vec<Station>>);

impl Lsa {
    pub fn vidp() -> Self {
        Self::parse(LSA_CSV).expect("baked LSA table parses; lvp tests guard this")
    }

    pub fn parse(csv: &str) -> Result<Self, String> {
        let mut out: BTreeMap<String, Vec<Station>> = BTreeMap::new();
        for (n, line) in csv.lines().enumerate().filter(|(_, l)| !l.starts_with('#') && !l.starts_with("end,") && !l.is_empty()) {
            let f: Vec<&str> = line.split(',').collect();
            let num = |k: usize| f.get(k).and_then(|v| v.parse::<f64>().ok()).ok_or(format!("lsa line {}: bad field {k}", n + 1));
            out.entry(f[0].to_string()).or_default().push(Station { along: num(1)?, left: num(2)?, right: num(3)? });
        }
        for (end, st) in &out {
            if st.len() < 2 || st.windows(2).any(|w| w[1].along < w[0].along) {
                return Err(format!("lsa {end}: needs 2 or more stations in along order"));
            }
        }
        Ok(Lsa(out))
    }

    /// Is `p` (metres, map frame) inside `d`'s LSA, grown by `margin_m` on
    /// every side? AD 2.22 VIII 4.3.4 wants the entire aircraft clear, so
    /// callers pass half a body. An end with no table has no LSA.
    pub fn contains(&self, d: &Dir, p: (f64, f64), margin_m: f64) -> bool {
        let Some(st) = self.0.get(&d.desig) else { return false };
        let v = (p.0 - d.from.0, p.1 - d.from.1);
        let along = v.0 * d.u.0 + v.1 * d.u.1;
        // Map y points north, so the left of travel u is (-u.y, u.x).
        let lat = -v.0 * d.u.1 + v.1 * d.u.0;
        let (first, last) = (st[0], st[st.len() - 1]);
        if along < first.along - margin_m || along > last.along + margin_m {
            return false;
        }
        let a = along.clamp(first.along, last.along);
        let k = st.windows(2).position(|w| a <= w[1].along).unwrap_or(st.len() - 2);
        let (s0, s1) = (st[k], st[k + 1]);
        let t = if s1.along > s0.along { (a - s0.along) / (s1.along - s0.along) } else { 0.0 };
        let left = s0.left + (s1.left - s0.left) * t;
        let right = s0.right + (s1.right - s0.right) * t;
        -right - margin_m <= lat && lat <= left + margin_m
    }
}

/// The taxiway names the published LVP routes use in `flow`. Arrivals and
/// departures together: which way a route runs is out of scope (PLAN-lvp.md).
pub fn taxiways(flow: Flow) -> Vec<&'static str> {
    let key = match flow {
        Flow::West => "west",
        Flow::East => "east",
    };
    ROUTES_CSV
        .lines()
        .filter_map(|l| l.split_once(','))
        .filter(|(f, _)| *f == key)
        .map(|(_, n)| n)
        .collect()
}

/// Per edge of `ap`: a named taxiway the published LVP routes of `flow` do
/// not use. Unnamed taxilanes (apron lanes, 29 km of them), runways and
/// stand links are never off-route: no stand could be reached otherwise.
pub fn off_route_mask(ap: &crate::airport::Airport, flow: Flow) -> Vec<bool> {
    let on: std::collections::BTreeSet<&str> = taxiways(flow).into_iter().collect();
    ap.edges.iter().map(|e| e.kind == crate::airport::EdgeKind::Taxiway && !e.name.is_empty() && !on.contains(e.name.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::airport::Airport;
    use crate::conflict::runway_dirs;
    use std::collections::BTreeSet;

    fn names_in_graph(ap: &Airport) -> BTreeSet<String> {
        ap.edges.iter().filter(|e| e.kind == crate::airport::EdgeKind::Taxiway).map(|e| e.name.clone()).collect()
    }

    /// The AIP names IGI's graph does not have, measured 2026-11-10. An OSM
    /// refresh that adds one (H6 matters: the west route to T1 crosses 10/28
    /// on it) fails here loudly instead of changing routes silently.
    #[test]
    fn missing_taxiway_names_are_the_known_ones() {
        let ap = Airport::parse(crate::VIDP).unwrap();
        let have = names_in_graph(&ap);
        let missing = |f| taxiways(f).into_iter().filter(|n| !have.contains(*n)).collect::<Vec<_>>();
        assert_eq!(taxiways(Flow::West).len(), 89);
        assert_eq!(taxiways(Flow::East).len(), 92);
        assert_eq!(missing(Flow::West), ["F6", "H1", "H6", "Link 19"]);
        assert_eq!(missing(Flow::East), ["D8", "D9", "F6", "G1", "G2", "Link 19"]);
    }

    /// Six ends, each with its own centreline inside from where its LSA
    /// begins to the far runway end, and each beginning no later than
    /// 1,500 m in: IGI's largest displaced threshold is 29L's 1,460 m.
    #[test]
    fn every_lsa_holds_its_own_centreline() {
        let ap = Airport::parse(crate::VIDP).unwrap();
        let lsa = Lsa::vidp();
        assert_eq!(lsa.0.keys().cloned().collect::<Vec<_>>(), ["10", "11L", "11R", "28", "29L", "29R"]);
        for d in runway_dirs(&ap).iter().filter(|d| lsa.0.contains_key(&d.desig)) {
            let st = &lsa.0[&d.desig];
            let len = (d.to.0 - d.from.0).hypot(d.to.1 - d.from.1);
            assert!(st[0].along < 1500.0, "{}: LSA starts {} m in", d.desig, st[0].along);
            assert!(st[st.len() - 1].along > len, "{}: LSA stops short of the far end", d.desig);
            let mut s = st[0].along.max(0.0) + 1.0;
            while s <= len {
                let p = (d.from.0 + d.u.0 * s, d.from.1 + d.u.1 * s);
                assert!(lsa.contains(d, p, 0.0), "{}: centreline at {s:.0} m outside", d.desig);
                s += 50.0;
            }
        }
    }

    /// The edges are where the table says, and the margin grows them.
    #[test]
    fn contains_reads_the_edges_and_the_margin() {
        let lsa = Lsa::parse("end,along_m,left_m,right_m,grade\nX,0,100,50,t\nX,1000,200,50,t\n").unwrap();
        let d = Dir { rwy: 0, desig: "X".into(), from: (0.0, 0.0), to: (1000.0, 0.0), u: (1.0, 0.0) };
        // Travelling east, left is north (+y).
        assert!(lsa.contains(&d, (500.0, 149.0), 0.0));
        assert!(!lsa.contains(&d, (500.0, 151.0), 0.0));
        assert!(lsa.contains(&d, (500.0, 151.0), 20.0));
        assert!(lsa.contains(&d, (500.0, -49.0), 0.0));
        assert!(!lsa.contains(&d, (500.0, -51.0), 0.0));
        assert!(!lsa.contains(&d, (-1.0, 0.0), 0.0));
        assert!(lsa.contains(&d, (-1.0, 0.0), 20.0));
        assert!(!lsa.contains(&d, (1001.0, 0.0), 0.0));
    }
}
