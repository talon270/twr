//! A landing aircraft stops at the end of its exit route and waits for a
//! taxi clearance, so that stop must be clear of the runway it left. The
//! exit route is cut at a path length (`vacate`: more than HOLD_SHORT_M of
//! taxiway); a taxiway that curves back along the runway can be that long
//! and still end beside it. Bar: every exit of every landing direction ends
//! at least HOLD_SHORT_M in a straight line from its runway's centreline.

use twr_sim::aircraft::HOLD_SHORT_M;
use twr_sim::{arrival, World};

fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    (p.0 - (a.0 + vx * t)).hypot(p.1 - (a.1 + vy * t))
}

#[test]
fn every_exit_ends_clear_of_its_runway() {
    let w = World::new(1);
    let (ap, g) = (&w.airport, &w.graph);
    let pos = |n: u32| (f64::from(ap.nodes[n as usize].x) / 10.0, f64::from(ap.nodes[n as usize].y) / 10.0);
    let (mut total, mut bad, mut bad_default) = (0, Vec::new(), Vec::new());
    for r in &ap.runways {
        let (a, b) = (pos(r.ends[0]), pos(r.ends[1]));
        for d in r.name.split('/') {
            let def = arrival::default_exit(ap, g, d).map(|(x, _)| x.node);
            for (x, route) in arrival::exits(ap, g, d) {
                total += 1;
                let gap = seg_dist(pos(route.end()), a, b);
                if gap < HOLD_SHORT_M as f64 {
                    let line = format!("{d} exit {:?} at {} m: stops {gap:.0} m from the centreline", x.name, x.at_mdm / 10_000);
                    if def == Some(x.node) {
                        bad_default.push(line.clone());
                    }
                    bad.push(line);
                }
            }
        }
    }
    println!("{} of {total} exits end within {HOLD_SHORT_M} m of the centreline; {} of them are a default exit", bad.len(), bad_default.len());
    for l in &bad_default {
        println!("  DEFAULT {l}");
    }
    assert!(bad.is_empty(), "{} of {total} exits stop beside the runway", bad.len());
}
