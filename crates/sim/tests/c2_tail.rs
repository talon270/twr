//! PLAN-twr.md § C2: tail direction on pushback. Every stand, every option:
//! the push goes one aircraft length along its lane (or to the lane's end),
//! the aircraft ends facing back toward the junction, a taxi away from the
//! tail is accepted and a taxi back along the pushed edge is refused as a
//! U-turn, and a branch with another aircraft in it is refused inline.

use twr_sim::route::Route;
use twr_sim::shape::dims;
use twr_sim::{Aircraft, Phase, World};

/// Ticks the push took. The aircraft rolls PUSH_MDM_PER_TICK a tick, so
/// ticks times that is the distance it really rolled, independent of the
/// distance the option claims.
fn pushed(w: &mut World, i: usize) -> u64 {
    for t in 0..40_000 {
        if w.aircraft[i].phase != Phase::Pushback {
            return t;
        }
        w.step_ticks(1);
    }
    panic!("push never ended: {:?}", w.aircraft[i].phase);
}

/// Every stand, every option, an A20N (37.6 m) and a B77W (73.9 m).
#[test]
fn every_stand_every_option_pushes_one_length_and_faces_back() {
    let ap = World::empty(1).airport;
    let (mut options, mut stands_without, mut dead_ends, mut u_turns, mut onward) = (0, 0, 0u64, 0, 0);
    let exit = ap.runway_exits()[0];
    for ty in ["A20N", "B77W"] {
        let want_m = dims(ty).0;
        for s in &ap.stands {
            let mut w = World::empty(1);
            let mut a = Aircraft::parked("T1", s.node);
            a.type_code = ty;
            w.aircraft.push(a);
            let n = w.tail_options(0).len();
            if n == 0 {
                stands_without += 1;
                continue;
            }
            for k in 0..n {
                let mut w = World::empty(1);
                let mut a = Aircraft::parked("T1", s.node);
                a.type_code = ty;
                w.aircraft.push(a);
                let (opt, by) = w.tail_options(0).swap_remove(k);
                assert!(by.is_none(), "empty world, stand {}: {by:?}", s.name);
                opt.route.check(&w.airport).unwrap();
                w.push_tail(0, k).unwrap();
                assert_eq!(w.aircraft[0].phase, Phase::Pushback);
                let rolled = pushed(&mut w, 0) * twr_sim::aircraft::PUSH_MDM_PER_TICK;
                options += 1;
                let a = &w.aircraft[0];
                assert_eq!(a.phase, Phase::Holding, "stand {} option {k}", s.name);
                assert!(a.on_edge(), "stopped on the last pushed edge");
                // Pushed distance from the junction, along the lane.
                let junction = opt.route.nodes[1];
                let link = u64::from(w.graph.len_dm[opt.route.edges[0] as usize]) * 1000;
                let lane_mdm = rolled - link;
                let full = (want_m * 10_000.0) as u64;
                // The lane the option offers, summed from the graph: a lane shorter
                // than the aircraft ends the push at the lane's end.
                let offered: u64 = opt.route.edges[1..].iter().map(|&e| u64::from(w.graph.len_dm[e as usize]) * 1000).sum();
                let want = full.min(offered);
                dead_ends += u64::from(offered < full);
                assert!(lane_mdm.abs_diff(want) <= 500, "stand {} option {k}: rolled {lane_mdm} mdm of lane, wanted {want}", s.name);
                // The tail is within one length of the junction, in a straight line.
                let (p, j) = (a.pos_m(&w.airport, &w.graph), w.airport.nodes[junction as usize]);
                let chord = (p.0 - f64::from(j.x) / 10.0).hypot(p.1 - f64::from(j.y) / 10.0);
                assert!(chord <= lane_mdm as f64 / 10_000.0 + 0.5, "stand {}: chord {chord:.1} m", s.name);
                // Facing back: the node ahead is on the junction's side of the aircraft.
                let ahead = a.next_node();
                assert!(opt.route.nodes[1..].contains(&ahead) && ahead != *opt.route.nodes.last().unwrap(), "stand {}: ahead {ahead}", s.name);
                // A taxi back along the pushed edge is a U-turn.
                let back = a.route.nodes[0];
                let e = a.route.edges[0];
                let turn = Route { nodes: vec![ahead, back], edges: vec![e] };
                let err = w.taxi(0, turn).unwrap_err();
                assert!(err.contains("U-turn"), "stand {}: {err}", s.name);
                u_turns += 1;
                // A planned taxi never offers the U-turn, and goes on.
                if let Some(r) = w.plan(0, exit, &[], &[]) {
                    assert_ne!(r.edges.first(), Some(&e), "stand {}: plan reuses the pushed edge", s.name);
                    w.taxi(0, r).unwrap_or_else(|m| panic!("stand {} option {k}: {m}", s.name));
                    assert_eq!(w.aircraft[0].phase, Phase::Taxi);
                    onward += 1;
                }
            }
        }
    }
    println!("{options} options pushed, {stands_without} stand/type pairs with none, {dead_ends} cut short by a dead end, {u_turns} U-turns refused, {onward} taxied on");
    // Every offered option leads somewhere: that is the filter's job.
    assert_eq!(onward, options, "an offered tail left an aircraft with no route to a runway");
    assert!(options > 400 && stands_without < 100, "{options} {stands_without}");
}

/// Another aircraft where the tail would go: refused inline, naming it.
#[test]
fn a_branch_with_an_aircraft_in_it_is_refused() {
    let mut w = World::empty(1);
    let stand = w.airport.stands.iter().find(|s| {
        let mut w2 = World::empty(1);
        w2.aircraft.push(Aircraft::parked("T1", s.node));
        w2.tail_options(0).len() >= 2
    }).unwrap().node;
    w.aircraft.push(Aircraft::parked("T1", stand));
    let opts = w.tail_options(0);
    let end = *opts[0].0.route.nodes.last().unwrap();
    w.aircraft.push(Aircraft { phase: Phase::Holding, ..Aircraft::parked("BLOCK", end) });
    let err = w.push_tail(0, 0).unwrap_err();
    assert!(err.starts_with("tail ") && err.contains("BLOCK is"), "{err}");
    assert_eq!(w.aircraft[0].phase, Phase::OnStand, "refused: still on stand");
    // The other branch is free, and the blocker does not sit in it.
    assert!(opts[1].1.is_none(), "{:?}", opts[1].1);
    w.push_tail(0, 1).unwrap();
}
