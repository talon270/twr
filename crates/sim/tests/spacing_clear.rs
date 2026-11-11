//! A follower on final must not reach the threshold while its leader is still
//! on the runway. The final spacing is the approach's job (A12), so a player
//! who clears every arrival at the standard 2 km point must never be charged
//! a runway incident for it. The "player" here is as naive as it gets: C at
//! 2,000 m for every arrival, no look at the runway, no go-around, and each
//! arrival that has vacated simply leaves. Bar: 0 `Occupied` incidents over
//! ten Peak shifts of 30 real minutes each.

use twr_sim::aircraft::Phase;
use twr_sim::conflict::Rule;
use twr_sim::traffic::{TICKS_PER_REAL_H, TICKS_PER_REAL_S};
use twr_sim::{Mode, World};

#[test]
fn clearing_every_arrival_at_2_km_never_raises_a_runway_incident() {
    let (mut worst, mut landed_all, mut bad) = (0usize, 0u32, Vec::new());
    for seed in 1..=10u64 {
        let mut w = World::shift(seed, Mode::Peak);
        w.halt_on_end = false;
        let (mut landed, mut gaps, mut most_queued, mut queued_ticks) = (0u32, Vec::<u64>::new(), 0usize, 0u64);
        let mut last_cross: std::collections::HashMap<String, u64> = Default::default();
        for _ in 0..(TICKS_PER_REAL_H / 2) {
            w.step_ticks(1);
            let q = w.traffic.as_ref().map_or(0, |t| t.final_queue.len());
            most_queued = most_queued.max(q);
            queued_ticks += q as u64;
            if w.tick % 20 != 0 {
                continue;
            }
            for i in 0..w.aircraft.len() {
                let a = &w.aircraft[i];
                if matches!(a.phase, Phase::Final(_)) && !a.cleared_to_land && a.final_m() <= 2_000.0 {
                    let _ = w.clear_to_land(i);
                }
            }
            for a in &w.aircraft {
                if let Phase::Landing(d) = &a.phase {
                    if a.pos_mdm < 20_000_000 && !last_cross.contains_key(&a.callsign) {
                        if let Some(&prev) = last_cross.values().max() {
                            let _ = prev;
                        }
                        last_cross.insert(a.callsign.clone(), w.tick);
                        landed += 1;
                        let _ = d;
                    }
                }
            }
            // An arrival that has vacated leaves; departures stay parked.
            w.aircraft.retain(|a| !(a.arrival && matches!(a.phase, Phase::Holding | Phase::OnStand | Phase::ShutDown)));
        }
        let occ: Vec<_> = w.monitor.incidents.iter().filter(|i| i.rule == Rule::Occupied).collect();
        for i in occ.iter().take(2) {
            println!("  seed {seed} tick {}: {}", i.tick, i.text);
        }
        println!("seed {seed}: {landed} landings, {} Occupied incidents, {} incidents of all rules; arrivals waiting for a final slot: at most {most_queued}, {:.1} aircraft-minutes (real)", occ.len(), w.monitor.incidents.len(), queued_ticks as f64 / TICKS_PER_REAL_S as f64 / 60.0);
        gaps.sort();
        landed_all += landed;
        worst = worst.max(occ.len());
        if !occ.is_empty() {
            bad.push(seed);
        }
    }
    println!("ten Peak shifts, naive C at 2 km: {landed_all} landings, worst seed {worst} Occupied incidents; {} s per real second", TICKS_PER_REAL_S);
    assert!(bad.is_empty(), "runway incidents on seeds {bad:?}");
}
