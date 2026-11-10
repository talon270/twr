//! PLAN-twr.md § B6, proof 1: the traffic stream on its own, no controller.
//!
//! The schedule tests run `Schedule` without a world, so tens of real hours
//! take well under a second. The spacing and stand tests run a real shift
//! with nobody at the controls: every arrival goes around for want of a
//! clearance and comes back, which is the hardest case for A12.

use std::collections::{BTreeMap, BTreeSet};

use twr_sim::aircraft::{Phase, Wake, MDM_PER_M};
use twr_sim::arrival::FINAL_SPAWN_M;
use twr_sim::traffic::{final_wake_nm, Mode, Schedule, FINAL_MIN_NM, OPERATORS, TICKS_PER_REAL_H, TICKS_PER_REAL_S, TYPES};
use twr_sim::World;

/// Long enough that ±10% is about 3σ for Light (1,080 events in 40 h).
/// The plan said 10 h; at 270 events, ±10% is only 1.6σ.
const RATE_HOURS: u64 = 40;

fn run_schedule(seed: u64, mode: Mode, hours: u64) -> Vec<twr_sim::traffic::Due> {
    let mut s = Schedule::new(seed, mode);
    (0..hours * TICKS_PER_REAL_H).flat_map(|t| s.tick(t)).collect()
}

#[test]
fn rates_split_and_unique_callsigns() {
    for (mode, rate) in [(Mode::Light, 27.0), (Mode::Normal, 53.0), (Mode::Peak, 84.0)] {
        for seed in [1, 2, 3] {
            let mut s = Schedule::new(seed, mode);
            // A Shift closes at 30 min. To count hours of it, feed the
            // stream ticks modulo one shift: the rate is flat, so only the
            // closing gate sees the difference.
            let shift = 30 * 60 * TICKS_PER_REAL_S;
            let dues: Vec<_> = (0..RATE_HOURS * TICKS_PER_REAL_H).flat_map(|t| s.tick(t % shift)).collect();
            let per_h = dues.len() as f64 / RATE_HOURS as f64;
            let arr = dues.iter().filter(|d| d.arrival).count() as f64 / dues.len() as f64;
            let names: BTreeSet<_> = dues.iter().map(|d| &d.callsign).collect();
            println!("{:<6} seed {seed}: {per_h:.1} per real hour (target {rate}), arrivals {:.1}%", mode.name(), arr * 100.0);
            assert!((per_h - rate).abs() <= rate * 0.10, "{mode:?} seed {seed}: {per_h:.1}/h");
            assert!((0.45..=0.55).contains(&arr), "{mode:?} seed {seed}: arrivals {arr:.3}");
            assert_eq!(names.len(), dues.len(), "callsigns repeat");
        }
    }
}

#[test]
fn operator_and_wake_mix_follow_the_table() {
    let hours = 100;
    let shift = 30 * 60 * TICKS_PER_REAL_S;
    let mut s = Schedule::new(4, Mode::Peak);
    let dues: Vec<_> = (0..hours * TICKS_PER_REAL_H).flat_map(|t| s.tick(t % shift)).collect();
    let n = dues.len() as f64;
    let mut by_op: BTreeMap<&str, u32> = BTreeMap::new();
    for d in &dues {
        *by_op.entry(&d.callsign[..3]).or_default() += 1;
    }
    for o in &OPERATORS {
        let got = f64::from(by_op.get(o.icao).copied().unwrap_or(0)) / n * 100.0;
        let want = o.share as f64 / 1000.0;
        println!("{} {:>5.1}% (table {want:.1}%)", o.icao, got);
        assert!((got - want).abs() <= 3.0, "{}: {got:.1}% against {want:.1}%", o.icao);
    }
    // B9: the research's 11.1% (H and J); 3 sigma at ~8,000 movements is 1 point.
    let heavy = dues.iter().filter(|d| matches!(d.wake, Wake::H | Wake::J)).count() as f64 / n * 100.0;
    let a380 = dues.iter().filter(|d| d.wake == Wake::J).count();
    println!("{} movements, heavies {heavy:.1}%, A380s {a380}", dues.len());
    assert!((9.5..=12.5).contains(&heavy), "heavies {heavy:.1}%");
    assert!(dues.iter().all(|d| TYPES.iter().any(|t| t.icao == d.type_code && t.wake == d.wake)));
}

#[test]
fn surge_ramps_and_shifts_close() {
    for k in 0..=15u64 {
        let t = k * 120 * TICKS_PER_REAL_S;
        let got = Mode::Surge.rate_milli(t) as f64 / 1000.0;
        let want = 53.0 * 1.1f64.powi(k as i32);
        println!("surge minute {:>2}: {got:.1}/h", k * 2);
        assert!((got - want).abs() <= 1.0, "minute {}: {got} against {want:.1}", k * 2);
    }
    // Nothing falls due after 30 real minutes of a Shift.
    let dues = run_schedule(5, Mode::Peak, 1);
    let end = 30 * 60 * TICKS_PER_REAL_S;
    assert!(dues.iter().any(|d| d.tick < end) && dues.iter().all(|d| d.tick < end));
}

/// Leaders on aircraft `i`'s final and the gap to each, metres; the same
/// geometry `final_slot_free` uses, written out again.
fn gaps(w: &World, i: usize) -> Vec<(f64, f64)> {
    let a = &w.aircraft[i];
    let Phase::Final(d) = &a.phase else { return Vec::new() };
    let me = a.air_mdm;
    w.aircraft
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .filter_map(|(_, b)| {
            let ahead = match &b.phase {
                Phase::Final(x) if x == d && b.air_mdm > me => b.air_mdm,
                Phase::Landing(x) if x == d => b.pos_mdm as i64,
                Phase::GoAround(x) if x == d && b.visible() && b.air_mdm > me => b.air_mdm,
                _ => return None,
            };
            let need = FINAL_MIN_NM.max(final_wake_nm(b.wake, a.wake)) as f64 * 1852.0;
            Some(((ahead - me) as f64 / MDM_PER_M as f64, need))
        })
        .collect()
}

#[test]
fn spacing_on_final_and_free_stands_with_nobody_at_the_controls() {
    for seed in [1, 2, 3] {
        // Surge, because a Shift closes at 30 min: by then Peak has put 21
        // departures on IGI's 115 stands. Surge passes 115 inside the hour.
        let mut w = World::shift(seed, Mode::Surge);
        let mut was_final = BTreeSet::new();
        let (mut entries, mut rejoins, mut most_queued, mut stands_full) = (0, 0, 0, 0);
        let spawn = -((FINAL_SPAWN_M * MDM_PER_M) as i64);
        // B10: 270 chart stands take longer to fill than OSM's 115, so run
        // past the hour until the stand queue has been used (2 h at most).
        while w.tick < TICKS_PER_REAL_H || (stands_full == 0 && w.tick < 2 * TICKS_PER_REAL_H) {
            let ga_before: BTreeSet<String> = w.aircraft.iter().filter(|a| matches!(a.phase, Phase::GoAround(_))).map(|a| a.callsign.clone()).collect();
            w.step_ticks(1);
            for i in 0..w.aircraft.len() {
                let a = &w.aircraft[i];
                if !matches!(a.phase, Phase::Final(_)) {
                    was_final.remove(&a.callsign);
                    continue;
                }
                if !was_final.insert(a.callsign.clone()) {
                    continue;
                }
                // Just came onto final, at the spawn point: check every leader.
                assert_eq!(a.air_mdm, spawn, "{} entered final away from the spawn point", a.callsign);
                entries += 1;
                rejoins += u32::from(ga_before.contains(&a.callsign));
                for (gap, need) in gaps(&w, i) {
                    assert!(gap >= need, "seed {seed} tick {}: {} onto final {gap:.0} m behind its leader, needs {need:.0}", w.tick, a.callsign);
                }
            }
            // One aircraft per stand, ever.
            let mut on: BTreeMap<u32, u32> = BTreeMap::new();
            for a in w.aircraft.iter().filter(|a| a.visible()) {
                if w.airport.stands.iter().any(|s| s.node == a.route.end()) {
                    *on.entry(a.route.end()).or_default() += 1;
                }
            }
            assert!(on.values().all(|&n| n == 1), "seed {seed} tick {}: two aircraft on one stand", w.tick);
            let t = w.traffic.as_ref().unwrap();
            most_queued = most_queued.max(t.final_queue.len());
            stands_full = stands_full.max(t.stand_queue.len());
        }
        println!("seed {seed}: ran {:.2} real h; {entries} onto final ({rejoins} rejoins after a go-around), final queue peaked at {most_queued}, stand queue at {stands_full}, incidents {}", w.tick as f64 / TICKS_PER_REAL_H as f64, w.monitor.incidents.len());
        assert!(rejoins >= 50, "seed {seed}: the go-around path ran {rejoins} times");
        assert!(most_queued > 0 && stands_full > 0, "seed {seed}: both queues must have been used");
    }
}

#[test]
fn a12_go_around_waits_for_its_slot() {
    // An arrival goes around for want of C. While it is on the circuit,
    // a second arrival comes onto the same final. When the circuit ends the
    // second is 1-2 km in: the first must wait, then rejoin 3 NM behind.
    let mut w = World::empty(1);
    let a = w.spawn_arrival("IGO1", Wake::M, "29L").unwrap();
    w.aircraft[a].pilot_ga = None;
    while !matches!(w.aircraft[a].phase, Phase::GoAround(_)) || w.aircraft[a].visible() || w.aircraft[a].circuit_left() > 20 * 20 {
        w.step_ticks(1);
    }
    let b = w.spawn_arrival("IGO2", Wake::M, "29L").unwrap();
    w.aircraft[b].pilot_ga = None;
    w.clear_to_land(b).unwrap();
    let mut waited = 0;
    while !matches!(w.aircraft[a].phase, Phase::Final(_)) {
        if w.aircraft[a].circuit_left() == 0 {
            waited += 1;
        }
        w.step_ticks(1);
    }
    let lead = match w.aircraft[b].phase {
        Phase::Final(_) => w.aircraft[b].air_mdm,
        _ => w.aircraft[b].pos_mdm as i64,
    };
    let gap = (lead - w.aircraft[a].air_mdm) as f64 / MDM_PER_M as f64;
    println!("IGO1 waited {waited} ticks past its circuit, rejoined {gap:.0} m behind IGO2");
    assert!(waited > 0, "it rejoined on time, on top of IGO2");
    assert!(gap >= 3.0 * 1852.0 && gap < 3.0 * 1852.0 + 40.0, "rejoined at {gap:.0} m, not the first tick 3 NM opened");
    assert!(w.monitor.incidents.is_empty());
}

#[test]
fn same_seed_same_shift() {
    let run = |seed| {
        let mut w = World::shift(seed, Mode::Normal);
        w.step_ticks((TICKS_PER_REAL_H / 2) as u32);
        let t = w.traffic.as_ref().unwrap();
        (format!("{:?}", t.log), format!("{:?}", w.aircraft))
    };
    let (a, b) = (run(11), run(11));
    assert_eq!(a, b);
    assert_ne!(a.0, run(12).0);
}
