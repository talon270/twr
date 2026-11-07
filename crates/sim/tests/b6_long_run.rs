//! PLAN-twr.md § B6, proof 2: the shared auto-controller on the world's own
//! traffic. It spawns nothing; it controls what the schedule sends.
//!
//! Peak must run clean. Surge is printed only: the controller is a test
//! tool, and how long it holds says nothing about what a player can hold.

mod common;

use common::*;
use twr_sim::traffic::{TICKS_PER_REAL_H, TICKS_PER_REAL_S};
use twr_sim::{Mode, World};

fn run(seed: u64, mode: Mode, ticks: u64, each: impl FnMut(&World)) -> (World, Stats, usize) {
    run_shift(seed, mode, None, ticks, each)
}

#[test]
fn two_real_hours_at_peak_raise_nothing() {
    // Ten seeds, not the plan's three: a 30-minute Peak shift offers only
    // about 42 movements, and each run takes about 3 s.
    for seed in 1..=10 {
        // Peak's shift closes at 30 min; the 90 that follow let the
        // controller finish everything it was given.
        let (w, s, conflicts) = run(seed, Mode::Peak, 2 * TICKS_PER_REAL_H, |_| {});
        let t = w.traffic.as_ref().unwrap();
        let (arr, dep) = (t.log.iter().filter(|d| d.arrival).count(), t.log.iter().filter(|d| !d.arrival).count());
        let shift_h = 0.5;
        println!(
            "seed {seed}, wind {} ({} flow): offered {:.0}/real h ({arr} arrivals, {dep} departures in 30 min); handled: {} departed, {} landed, {} shut down on stand; {} still out; go-arounds {}; most live conflicts {conflicts}, incidents {}",
            w.wind().text(),
            w.wind().flow().name(),
            (arr + dep) as f64 / shift_h,
            s.departed,
            s.landed,
            s.on_stand,
            backlog(&w),
            w.go_arounds.len(),
            w.monitor.incidents.len()
        );
        for i in w.monitor.incidents.iter().take(10) {
            println!("  tick {} {:?}: {}", i.tick, i.rule, i.text);
        }
        assert_eq!(conflicts, 0, "seed {seed}: a conflict was live");
        assert!(w.monitor.incidents.is_empty(), "seed {seed}: {} incidents", w.monitor.incidents.len());
        assert_eq!(s.departed as usize, dep, "seed {seed}: every departure got out");
        assert_eq!(s.on_stand as usize, arr, "seed {seed}: every arrival reached a stand and shut down");
        // B7: the score's movements are the controller's takeoffs plus landings.
        println!("  score: {} movements, delay {:.1} game min over {} aircraft", w.score.movements, w.score.delay_min(), w.score.delayed());
        assert_eq!(w.score.movements, s.departed + s.landed, "seed {seed}: movements");
    }
}

#[test]
fn surge_backlog_is_printed() {
    for seed in [1, 2, 3] {
        let mut first_over = None;
        let (w, s, _) = run(seed, Mode::Surge, TICKS_PER_REAL_H / 2, |w| {
            if first_over.is_none() && backlog(w) > 10 {
                first_over = Some(w.tick / TICKS_PER_REAL_S);
            }
        });
        println!(
            "seed {seed}: backlog first over 10 at {}; at 30 min: rate {:.0}/h, {} departed, {} on stand, backlog {}, incidents {}",
            first_over.map_or("never".into(), |s| format!("{}:{:02}", s / 60, s % 60)),
            Mode::Surge.rate_milli(w.tick) as f64 / 1000.0,
            s.departed,
            s.on_stand,
            backlog(&w),
            w.monitor.incidents.len()
        );
    }
}
