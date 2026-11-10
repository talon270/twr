//! PLAN-twr.md § C4 "Not run": two Peak long runs the runway picker left
//! open. Each is two real hours, four seeds, the shared auto-controller.
//!   · east flow forced by the wind (100/06, a headwind on 11R 10 11L 09);
//!   · 5 to 10 kt of tailwind: an easterly at 6 to 9 kt with the plan
//!     switched to west, which the picker allows up to 10 kt.
//! Both must raise nothing, get every departure out and park every arrival.
//! The per-seed line prints movements and delay so the plan can quote them.

mod common;

use common::*;
use twr_sim::traffic::{TICKS_PER_REAL_H, TICKS_PER_REAL_S};
use twr_sim::weather::{Flow, Wind, TAILWIND_LIMIT_KT};
use twr_sim::{Mode, World};

fn shift(seed: u64, wind: Wind) -> World {
    let mut w = World::shift(seed, Mode::Peak);
    w.traffic.as_mut().unwrap().schedule.wind = wind;
    w.halt_on_end = false;
    w
}

/// Run two real hours; assert nothing was raised and every movement finished;
/// print the numbers. `runways` are the ends the log must be drawn from.
fn long_run(seed: u64, mut w: World, runways: &[&str]) {
    let mut c = Ctl::new(&w, seed);
    c.spawn = false;
    for _ in 0..2 * TICKS_PER_REAL_H {
        w.step_ticks(1);
        c.act(&mut w);
    }
    let t = w.traffic.as_ref().unwrap();
    let (arr, dep) = (t.log.iter().filter(|d| d.arrival).count(), t.log.iter().filter(|d| !d.arrival).count());
    let waits = &w.score.waits;
    println!(
        "seed {seed}: {arr} arrivals {dep} departures; score {} movements in the first 30 min; delay {:.1} min over {} aircraft, longest wait {:.0} s, shortest {:.0} s; incidents {}",
        w.score.movements,
        w.score.delay_min(),
        waits.len(),
        waits.values().max().copied().unwrap_or(0) as f64 / TICKS_PER_REAL_S as f64,
        waits.values().min().copied().unwrap_or(0) as f64 / TICKS_PER_REAL_S as f64,
        w.monitor.incidents.len()
    );
    for i in w.monitor.incidents.iter().take(5) {
        println!("  tick {} {:?}: {}", i.tick, i.rule, i.text);
    }
    assert!(w.monitor.incidents.is_empty(), "seed {seed}: {} incidents", w.monitor.incidents.len());
    assert_eq!(c.stats.departed as usize, dep, "seed {seed}: every departure got out");
    assert_eq!(c.stats.on_stand as usize, arr, "seed {seed}: every arrival parked");
    assert!(t.log.iter().all(|d| runways.contains(&d.rwy)), "seed {seed}: a movement drew a runway outside {runways:?}");
}

#[test]
fn two_real_hours_at_peak_in_east_flow_raise_nothing() {
    for seed in 1..=4 {
        let w = shift(seed, Wind { dir: 100, kt: 6, gust: None });
        assert_eq!(w.traffic.as_ref().unwrap().schedule.plan().flow, Flow::East, "the wind forces east");
        long_run(seed, w, &["11R", "10", "11L", "09"]);
    }
}

#[test]
fn two_real_hours_at_peak_with_five_to_ten_kt_of_tailwind_raise_nothing() {
    for (seed, kt) in [(1, 6), (2, 7), (3, 8), (4, 9)] {
        let mut w = shift(seed, Wind { dir: 100, kt, gust: None });
        w.runway_direction(Flow::West).unwrap();
        let tw = w.traffic.as_ref().unwrap().schedule.wind.tailwind_kt(Flow::West);
        assert!((5.0..=TAILWIND_LIMIT_KT).contains(&tw), "seed {seed}: {tw} kt is outside 5..=10");
        w.step_ticks(1); // an empty airport: the waiting west plan takes over at once
        assert_eq!(w.traffic.as_ref().unwrap().schedule.plan().flow, Flow::West, "seed {seed}: west is in force");
        long_run(seed, w, &["29L", "28", "29R", "27"]);
    }
}
