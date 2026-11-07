//! PLAN-twr.md § B4, proof 2, and § B5: ten game hours of correctly spaced
//! traffic, departures and real arrivals, raise no conflict and no incident.
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

mod common;

use common::*;
use twr_sim::{GoAround, World};

fn shift(seed: u64, hours: u64) -> (World, Stats) {
    let mut w = World::new(seed);
    w.aircraft.clear();
    let mut c = Ctl::new(&w, seed);
    while w.tick < hours * HOUR {
        w.step_ticks(1);
        c.act(&mut w);
    }
    for g in &w.go_arounds {
        match g.why {
            GoAround::Pilot(_) | GoAround::Weather(_) => c.stats.ga_pilot += 1,
            GoAround::Instructed => c.stats.ga_instructed += 1,
            GoAround::NoClearance => c.stats.ga_no_clearance += 1,
        }
    }
    (w, c.stats)
}

#[test]
fn ten_hours_of_spaced_traffic_raise_nothing() {
    let runs: Vec<(u64, World, Stats)> = [1, 2, 3].into_iter().map(|seed| {
        let (w, s) = shift(seed, 10);
        println!("seed {seed}: {s:?}, incidents {}", w.monitor.incidents.len());
        for i in w.monitor.incidents.iter().take(10) {
            println!("  tick {} {:?}: {}", i.tick, i.rule, i.text);
        }
        (seed, w, s)
    }).collect();
    for (seed, w, s) in runs {
        assert!(w.monitor.incidents.is_empty(), "seed {seed}: {} incidents", w.monitor.incidents.len());
        // Floors from PLAN-twr.md § B4, set before the first run.
        assert!(s.departed >= 300, "seed {seed}: {} departures", s.departed);
        // A coverage floor, not a safety bar. 50 until B9; since heavies need
        // 3,100 m they no longer queue on 27 or 09, fewer mixed queues form,
        // and the three seeds read 67 / 48 / 73 (2026-11-07). 40 keeps it a
        // check that the boundary is reached often.
        assert!(s.wake_boundary >= 40, "seed {seed}: {} wake-boundary takeoffs", s.wake_boundary);
        // Coverage too, lowered with the one above for the same reason (B9):
        // 31 / 17 / 26 across the parallels, against 39 / 29 / 21 in B5.
        assert!(s.parallel_boundary >= 15, "seed {seed}: {} across 11L/11R or 29L/29R", s.parallel_boundary);
        assert!(s.after_arrival >= 30, "seed {seed}: {} takeoffs on the tick an arrival vacated", s.after_arrival);
        assert!(s.crossings >= 50, "seed {seed}: {} crossings", s.crossings);
        // Floors from PLAN-twr.md § B5, set before the first run.
        assert!(s.on_stand >= 30, "seed {seed}: {} arrivals from final to on-stand", s.on_stand);
        assert!(s.ga_pilot >= 10 && s.ga_instructed >= 10 && s.ga_no_clearance >= 10, "seed {seed}: go-arounds {s:?}");
    }
}

/// Any shift replays exactly from its seed (PLAN-twr.md § Stack).
#[test]
fn one_hour_replays_identically() {
    let (a, sa) = shift(9, 1);
    let (b, sb) = shift(9, 1);
    assert_eq!(format!("{sa:?}"), format!("{sb:?}"));
    assert_eq!(format!("{:?}", a.aircraft), format!("{:?}", b.aircraft));
    assert_eq!(a.monitor.incidents, b.monitor.incidents);
    assert!(sa.departed > 0, "{sa:?}");
}
