//! PLAN-twr.md § B7: each delay rule broken on purpose and checked at its
//! boundary, and the two ways a shift ends.
//!
//! Most tests give the world a Practice traffic stream: it spawns nothing
//! and never ends, but it switches scoring on, so every aircraft in play is
//! one the test placed.

mod common;

use twr_sim::aircraft::{Aircraft, Phase, Wake};
use twr_sim::score::{STAND_GRACE_S, SURGE_INCIDENTS};
use twr_sim::traffic::{Due, Mode, Traffic, SHIFT_REAL_S, TICKS_PER_REAL_S};
use twr_sim::{World, TICK_HZ};

fn scored(mut w: World) -> World {
    w.traffic = Some(Traffic::new(w.seed, Mode::Practice));
    w
}

/// Delay added by one tick.
fn tick(w: &mut World) -> u64 {
    let d = w.score.delay_ticks;
    w.step_ticks(1);
    w.score.delay_ticks - d
}

const GRACE: u64 = STAND_GRACE_S * TICK_HZ as u64;

#[test]
fn on_stand_counts_after_the_grace() {
    let mut w = scored(World::empty(1));
    let stand = w.airport.stands[10].node;
    w.add(Aircraft { ready: Some(0), assigned: Some("29R"), ..Aircraft::parked("IGO1", stand) });
    w.step_ticks(70 * TICK_HZ);
    assert_eq!(w.score.delay_ticks, 200, "70 game s on stand is 10 s past the grace");

    // Pushed inside the grace: nothing, all the way to the end of the push.
    let mut w = scored(World::empty(1));
    let i = w.add(Aircraft { ready: Some(0), assigned: Some("29R"), ..Aircraft::parked("IGO2", stand) });
    w.step_ticks((GRACE - 1) as u32);
    w.push(i).unwrap();
    // The score counts the state a tick ends in.
    loop {
        let d = tick(&mut w);
        if w.aircraft[i].phase == Phase::Pushback {
            assert_eq!(d, 0, "pushing back");
        } else {
            assert_eq!(d, 1, "the push finished: Holding waits on T from its first tick");
            break;
        }
    }
    assert_eq!(w.score.delay_ticks, 1);
}

#[test]
fn hold_short_waits_for_traffic_free() {
    // IGO512 lined up on 29R; IGO701 held short of a crossing of 29R.
    let mut w = scored(World::scenario(1, "runway").unwrap());
    let (dep, crosser) = (w.aircraft.iter().position(|a| a.callsign == "IGO512").unwrap(), w.aircraft.iter().position(|a| a.callsign == "IGO701").unwrap());
    assert!(matches!(w.aircraft[crosser].phase, Phase::HoldShort(_)));
    assert_eq!(tick(&mut w), 1, "only the lined-up departure waits; the runway it sits on holds the crosser");
    w.takeoff(dep).unwrap();
    loop {
        let d = tick(&mut w);
        if !matches!(w.aircraft[dep].phase, Phase::Departed(_)) {
            assert_eq!(d, 0, "rolling and airborne over 29R: the crosser's wait is the rule");
        } else {
            assert_eq!(d, 1, "runway clear: the crosser waits on X");
            break;
        }
    }
    assert_eq!(w.score.waits.len(), 2, "both waited");
    assert_eq!(w.score.delayed(), 0, "neither waited past a minute");
}

#[test]
fn lined_up_waits_out_the_wake_free() {
    // AIC101 (H) has just left 29R; IGO512 (M) lined up behind.
    let mut w = scored(World::scenario(1, "wake").unwrap());
    let i = w.aircraft.iter().position(|a| a.callsign == "IGO512").unwrap();
    let mut free = 0;
    loop {
        let d = tick(&mut w);
        if w.wake_wait(i).is_some() {
            assert_eq!(d, 0);
            free += 1;
        } else {
            assert_eq!(d, 1, "the first tick the wake allows K");
            break;
        }
    }
    println!("waited {free} ticks of wake without delay");
    assert!(free > 0);
}

#[test]
fn go_arounds_instructed_count_pilot_ones_do_not() {
    let mut w = scored(World::empty(1));
    let i = w.spawn_arrival("IGO1", Wake::M, "29L").unwrap();
    w.aircraft[i].pilot_ga = None;
    w.go_around(i).unwrap();
    let mut n = 0;
    loop {
        let d = tick(&mut w);
        if matches!(w.aircraft[i].phase, Phase::Final(_)) {
            assert_eq!(d, 0, "back on final");
            break;
        }
        assert_eq!(d, 1);
        n += 1;
    }
    println!("G cost {n} ticks, {:.1} game min", n as f64 / 20.0 / 60.0);
    assert_eq!(w.score.delay_ticks, n);

    let mut w = scored(World::empty(1));
    let i = w.spawn_arrival("IGO2", Wake::M, "29L").unwrap();
    w.aircraft[i].pilot_ga = Some((2_000 * 10_000, "unstable approach"));
    w.clear_to_land(i).unwrap();
    while !matches!(w.aircraft[i].phase, Phase::Final(_)) || w.go_arounds.is_empty() {
        w.step_ticks(1);
    }
    assert_eq!(w.score.delay_ticks, 0, "a pilot go-around is a challenge");
    assert_eq!(w.challenges(), 1);
}

#[test]
fn stand_queue_counts_after_the_grace() {
    let mut w = scored(World::empty(1));
    let nodes: Vec<u32> = w.airport.stands.iter().map(|s| s.node).collect();
    for (k, n) in nodes.into_iter().enumerate() {
        w.add(Aircraft::parked(&format!("P{k}"), n));
    }
    let due = |k: u32| Due { tick: 0, arrival: false, callsign: format!("Q{k}"), type_code: "A20N", wake: Wake::M, rwy: "29R" };
    w.traffic.as_mut().unwrap().stand_queue.extend((0..3).map(due));
    w.step_ticks(GRACE as u32);
    assert_eq!(w.score.delay_ticks, 0, "inside the grace");
    w.step_ticks(100);
    assert_eq!(w.score.delay_ticks, 300, "3 queued departures for 100 ticks");
    assert_eq!(w.traffic.as_ref().unwrap().stand_queue.len(), 3, "still no free stand");
}

#[test]
fn surge_ends_on_the_third_incident() {
    let mut w = World::scenario(1, "wake").unwrap();
    w.traffic = Some(Traffic::new(1, Mode::Surge));
    let e = w.aircraft.iter().find(|a| a.callsign == "IGO512").unwrap().route.nodes[0];
    let mut k = 0;
    while w.score.over.is_none() {
        // K at once, inside the wake and onto a busy runway: incidents.
        if let Some(i) = w.aircraft.iter().position(|a| matches!(a.phase, Phase::LinedUp(_))) {
            w.takeoff(i).unwrap();
        } else if w.aircraft.iter().all(|a| !matches!(a.phase, Phase::LineUp(_))) {
            k += 1;
            w.add_lined_up(&format!("X{k}"), Wake::L, e).unwrap();
        }
        w.step_ticks(1);
        assert!(w.tick < 100_000, "no third incident");
    }
    let third = w.monitor.incidents[SURGE_INCIDENTS - 1].tick;
    assert_eq!(w.score.over, Some(third), "over on the third incident's tick");
    let before = (w.tick, format!("{:?}", w.aircraft), w.score.clone());
    w.step_ticks(1000);
    assert_eq!(before, (w.tick, format!("{:?}", w.aircraft), w.score.clone()), "nothing moves after the end");
}

#[test]
fn shift_ends_at_thirty_real_minutes() {
    let mut w = World::shift(3, Mode::Normal);
    let mut c = common::Ctl::new(&w, 3);
    c.spawn = false;
    while w.score.over.is_none() {
        w.step_ticks(1);
        c.act(&mut w);
    }
    assert_eq!(w.score.over, Some(SHIFT_REAL_S * TICKS_PER_REAL_S));
    let (t, m) = (w.tick, w.score.movements);
    for _ in 0..1000 {
        w.step_ticks(1);
        c.act(&mut w);
    }
    assert_eq!((w.tick, w.score.movements), (t, m), "no movement after 30:00 counts");
    println!("Normal seed 3 under the controller: {m} movements of {} offered, delay {:.1} game min over {} aircraft", w.offered(), w.score.delay_min(), w.score.delayed());
}

#[test]
fn same_seed_same_commands_same_score() {
    let run = || {
        let mut w = World::shift(5, Mode::Peak);
        let mut c = common::Ctl::new(&w, 5);
        c.spawn = false;
        while w.score.over.is_none() {
            w.step_ticks(1);
            c.act(&mut w);
        }
        w.score
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    assert!(a.movements > 0);
}
