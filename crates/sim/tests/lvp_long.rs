//! PLAN-lvp.md step 9: ten Peak shifts forced to LVP, each on its seed's own
//! wind, the shared auto-controller at the controls. Two real hours each:
//! the 30-minute shift, then time to land, park and depart what it gave out.
//! Bar: 0 incidents, every departure out, every arrival parked. The per-seed
//! line prints movements, delay and the go-arounds by cause, so the plan can
//! quote what fog costs the flow.

mod common;

use std::collections::BTreeMap;

use common::*;
use twr_sim::traffic::{TICKS_PER_REAL_H, TICKS_PER_REAL_S};
use twr_sim::{Mode, World};

#[test]
fn ten_peak_shifts_under_lvp_raise_nothing() {
    let mut failed = Vec::new();
    for seed in 1..=10 {
        let mut w = World::shift(seed, Mode::Peak);
        w.set_lvp(true);
        w.halt_on_end = false;
        let mut c = Ctl::new(&w, seed);
        c.spawn = false;
        for _ in 0..2 * TICKS_PER_REAL_H {
            w.step_ticks(1);
            c.act(&mut w);
        }
        let t = w.traffic.as_ref().unwrap();
        let (arr, dep) = (t.log.iter().filter(|d| d.arrival).count(), t.log.iter().filter(|d| !d.arrival).count());
        let mut why: BTreeMap<String, u32> = BTreeMap::new();
        for g in &w.go_arounds {
            *why.entry(g.why.text()).or_default() += 1;
        }
        let waits = &w.score.waits;
        let wind = w.wind();
        println!(
            "seed {seed}: wind {:03}/{} {:?}; {arr} arrivals {dep} departures; departed {} parked {}; score {} movements and {:.1} min delay over {} aircraft in the two hours, longest wait {:.0} s; go-arounds {:?}; incidents {}",
            wind.dir,
            wind.kt,
            w.flow(),
            c.stats.departed,
            c.stats.on_stand,
            w.score.movements,
            w.score.delay_min(),
            waits.len(),
            waits.values().max().copied().unwrap_or(0) as f64 / TICKS_PER_REAL_S as f64,
            why,
            w.monitor.incidents.len()
        );
        for i in w.monitor.incidents.iter().take(5) {
            println!("  tick {} {:?}: {}", i.tick, i.rule, i.text);
        }
        if !w.monitor.incidents.is_empty() || c.stats.departed as usize != dep || c.stats.on_stand as usize != arr {
            failed.push(seed);
        }
    }
    assert!(failed.is_empty(), "seeds that raised an incident or left a movement unfinished: {failed:?}");
}
