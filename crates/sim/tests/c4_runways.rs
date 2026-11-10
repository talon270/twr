//! PLAN-twr.md § C4: the runway picker. One direction at a time, tailwind
//! warned at 5 kt and refused past 10, at least one landing and one
//! departure end, a direction switch that waits for the old runways to
//! clear, heavies that wait when no picked departure runway is 3,100 m,
//! and a Peak run on a non-default pick that raises nothing.

mod common;

use common::*;
use twr_sim::traffic::{TICKS_PER_REAL_H, TICKS_PER_REAL_S};
use twr_sim::weather::{Flow, Plan, Wind, ENDS};
use twr_sim::{Mode, Phase, World};

const CALM: Wind = Wind::CALM;

fn shift(seed: u64, mode: Mode, wind: Wind) -> World {
    let mut w = World::shift(seed, mode);
    w.traffic.as_mut().unwrap().schedule.wind = wind;
    w.halt_on_end = false;
    w
}

/// Step with the shared controller until `until` returns true or `max` ticks pass.
fn run(w: &mut World, c: &mut Ctl, max: u64, mut until: impl FnMut(&World) -> bool) -> bool {
    let stop = w.tick + max;
    while w.tick < stop {
        w.step_ticks(1);
        c.act(w);
        if until(w) {
            return true;
        }
    }
    false
}

fn controller(w: &World, seed: u64) -> Ctl {
    let mut c = Ctl::new(w, seed);
    c.spawn = false;
    c
}

#[test]
fn the_ends_table_matches_the_airport() {
    let w = World::empty(1);
    for (d, _, len) in ENDS {
        let best = w.airport.nodes.iter().enumerate()
            .filter_map(|(n, _)| w.graph.runway_run(&w.airport, n as u32))
            .filter(|(_, x)| x == d)
            .map(|(r, _)| w.graph.run_len_dm(&r) / 10)
            .max()
            .unwrap();
        assert_eq!(best, len, "{d}");
    }
}

#[test]
fn the_rules_at_their_edges() {
    // 100/08: an easterly, so 8 kt of tailwind on the west ends.
    let mut w = shift(1, Mode::Peak, Wind { dir: 100, kt: 8, gust: None });
    assert_eq!(w.traffic.as_ref().unwrap().schedule.plan().flow, Flow::East, "the wind picks east");
    // One direction at a time: a west end is refused while the plan is east.
    let e = w.runway_set("29L", true, true).unwrap_err();
    assert!(e.contains("faces west") && e.contains("switch direction"), "{e}");
    // 8 kt is over the warning and under the limit: allowed, and said.
    w.runway_direction(Flow::West).unwrap();
    assert!(w.runway_status().iter().any(|s| s.contains("8 kt of tailwind on 29L 28 29R 27")), "{:?}", w.runway_status());
    // 12 kt is over the limit: the switch is refused, naming the end.
    let mut w = shift(1, Mode::Peak, Wind { dir: 100, kt: 12, gust: None });
    let e = w.runway_direction(Flow::West).unwrap_err();
    assert!(e.contains("tailwind") && e.contains("limit 10"), "{e}");
    assert!(w.traffic.as_ref().unwrap().schedule.next.is_none(), "refused: nothing waits");
    // Calm wind: any direction, any end.
    let mut w = shift(1, Mode::Peak, CALM);
    w.runway_set("29R", true, true).unwrap();
    assert_eq!(w.traffic.as_ref().unwrap().schedule.plan().land, vec!["29L", "28", "29R"]);
    // At least one landing and one departure end.
    w.runway_set("29L", true, false).unwrap();
    w.runway_set("28", true, false).unwrap();
    assert_eq!(w.runway_set("29R", true, false).unwrap_err(), "keep at least one landing runway");
    w.runway_set("29R", false, false).unwrap();
    assert_eq!(w.runway_set("27", false, false).unwrap_err(), "keep at least one departure runway");
    // Practice has no traffic to plan for.
    assert!(World::new(1).runway_direction(Flow::East).is_err());
}

/// A switch waits for the old direction, holds new movements without a
/// runway meanwhile, then draws them from the new plan; whatever was
/// assigned before keeps its runway, and nothing is raised.
#[test]
fn a_switch_waits_for_the_old_runways_then_takes_over() {
    let seed = 3;
    let mut w = shift(seed, Mode::Peak, CALM);
    let mut c = controller(&w, seed);
    run(&mut w, &mut c, 6 * 60 * TICKS_PER_REAL_S, |_| false);
    let before: Vec<(String, &str)> = w.aircraft.iter().filter_map(|a| a.assigned.map(|r| (a.callsign.clone(), r))).collect();
    assert!(before.iter().any(|(_, r)| matches!(*r, "29R" | "27")), "traffic is on west runways: {before:?}");
    w.runway_direction(Flow::East).unwrap();
    let t = w.traffic.as_ref().unwrap();
    assert!(t.schedule.next.is_some() && t.schedule.plan().flow == Flow::West, "waiting, west still in force");
    assert!(w.runway_status().iter().any(|s| s.contains("switching to east")), "{:?}", w.runway_status());
    // While it waits, nothing is spawned onto an east runway.
    let mut held_seen = false;
    let switched = run(&mut w, &mut c, 60 * 60 * TICKS_PER_REAL_S, |w| {
        let t = w.traffic.as_ref().unwrap();
        held_seen |= t.final_queue.iter().chain(&t.stand_queue).any(|d| d.rwy.is_empty());
        if t.schedule.next.is_some() {
            assert!(w.aircraft.iter().all(|a| !matches!(&a.phase, Phase::Final(d) | Phase::Landing(d) if d.starts_with("11") || d.as_str() == "10" || d.as_str() == "09")), "an east landing while waiting");
        }
        t.schedule.next.is_none()
    });
    assert!(switched, "the switch never took effect");
    assert!(held_seen, "no movement was held while it waited (the test needs one)");
    assert_eq!(w.traffic.as_ref().unwrap().schedule.plan(), Plan::default_for(Flow::East));
    // New movements are east; everything assigned before is still west.
    let t = w.traffic.as_ref().unwrap();
    let mark = t.log.len();
    run(&mut w, &mut c, 10 * 60 * TICKS_PER_REAL_S, |_| false);
    let t = w.traffic.as_ref().unwrap();
    let east = t.log[mark..].iter().filter(|d| matches!(d.rwy, "11R" | "10" | "11L" | "09")).count();
    println!("after the switch: {} new movements, {east} east; held seen {held_seen}", t.log.len() - mark);
    assert!(east > 0 && east == t.log.len() - mark, "{east} of {}", t.log.len() - mark);
    for (cs, r) in &before {
        if let Some(a) = w.aircraft.iter().find(|a| &a.callsign == cs) {
            assert_eq!(a.assigned, Some(*r), "{cs} kept its runway");
        }
    }
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents.iter().map(|i| &i.text).collect::<Vec<_>>());
    // Choosing the current direction again cancels a wait.
    w.runway_direction(Flow::West).unwrap();
    w.runway_direction(Flow::East).unwrap();
    assert!(w.traffic.as_ref().unwrap().schedule.next.is_none(), "east is already in force: no wait");
}

/// Only 27 departs: a heavy has no runway of 3,100 m. It waits, the panel
/// says so, and 29R lets it go.
#[test]
fn a_heavy_waits_when_no_picked_runway_is_long_enough() {
    let seed = 5;
    let mut w = shift(seed, Mode::Peak, CALM);
    w.runway_set("29R", false, false).unwrap();
    assert_eq!(w.traffic.as_ref().unwrap().schedule.plan().depart, vec!["27"]);
    let mut c = controller(&w, seed);
    let waited = run(&mut w, &mut c, 30 * 60 * TICKS_PER_REAL_S, |w| {
        let t = w.traffic.as_ref().unwrap();
        t.stand_queue.iter().any(|d| d.rwy.is_empty())
    });
    assert!(waited, "no heavy departure drew in 30 min on this seed");
    let st = w.runway_status();
    assert!(st.iter().any(|s| s.contains("heavy departure") && s.contains("3100")), "{st:?}");
    let t = w.traffic.as_ref().unwrap();
    assert!(t.stand_queue.iter().filter(|d| d.rwy.is_empty()).all(|d| matches!(d.wake, twr_sim::Wake::H | twr_sim::Wake::J)), "only heavies wait");
    assert!(t.stand_queue.iter().filter(|d| d.rwy == "27").all(|d| !matches!(d.wake, twr_sim::Wake::H | twr_sim::Wake::J)), "no heavy was given 27");
    w.runway_set("29R", false, true).unwrap();
    w.step_ticks(1);
    let t = w.traffic.as_ref().unwrap();
    assert!(t.stand_queue.iter().chain(&t.final_queue).all(|d| !d.rwy.is_empty()), "29R back: every heavy has a runway");
    assert!(w.runway_status().iter().all(|s| !s.contains("heavy")), "{:?}", w.runway_status());
}

/// Peak, two real hours, a non-default pick: 29R lands and departs (mixed),
/// 28 departs too, 29L alone lands otherwise. Zero incidents, every movement done.
#[test]
fn two_real_hours_at_peak_on_a_mixed_plan_raise_nothing() {
    for seed in 1..=4 {
        let mut w = shift(seed, Mode::Peak, CALM);
        w.runway_set("29R", true, true).unwrap();
        w.runway_set("28", true, false).unwrap();
        w.runway_set("28", false, true).unwrap();
        let plan = w.traffic.as_ref().unwrap().schedule.plan();
        assert_eq!((plan.land.clone(), plan.depart.clone()), (vec!["29L", "29R"], vec!["28", "29R", "27"]));
        let mut c = controller(&w, seed);
        let mut most = 0;
        run(&mut w, &mut c, 2 * TICKS_PER_REAL_H, |w| {
            most = most.max(w.monitor.conflicts.len());
            false
        });
        let t = w.traffic.as_ref().unwrap();
        let (arr, dep) = (t.log.iter().filter(|d| d.arrival).count(), t.log.iter().filter(|d| !d.arrival).count());
        let mixed = t.log.iter().filter(|d| d.rwy == "29R").count();
        println!("seed {seed}: {arr} arrivals {dep} departures, {mixed} on 29R; {} departed {} landed; incidents {}", c.stats.departed, c.stats.landed, w.monitor.incidents.len());
        for i in w.monitor.incidents.iter().take(5) {
            println!("  tick {} {:?}: {}", i.tick, i.rule, i.text);
        }
        assert!(w.monitor.incidents.is_empty(), "seed {seed}: {} incidents", w.monitor.incidents.len());
        assert_eq!(c.stats.departed as usize, dep, "seed {seed}: every departure got out");
        assert_eq!(c.stats.on_stand as usize, arr, "seed {seed}: every arrival parked");
        assert!(mixed > 0, "seed {seed}: 29R carried no movement");
    }
}
