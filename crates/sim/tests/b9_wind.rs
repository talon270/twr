//! PLAN-twr.md § B9: the wind, the flow it picks, east flow end to end
//! under the controller, and windshear go-arounds.

mod common;

use common::*;
use twr_sim::aircraft::Wake;
use twr_sim::traffic::TICKS_PER_REAL_H;
use twr_sim::weather::{self, Flow, Wind};
use twr_sim::{GoAround, Mode, World};

#[test]
fn the_draw_follows_its_table() {
    let n = 10_000u64;
    let winds: Vec<Wind> = (0..n).map(weather::draw).collect();
    let east = winds.iter().filter(|w| w.flow() == Flow::East).count() as f64 / n as f64 * 100.0;
    let gusty = winds.iter().filter(|w| w.gust.is_some()).count() as f64 / n as f64 * 100.0;
    println!("{n} seeds: east flow {east:.1}%, gusty {gusty:.1}%");
    // 25% easterly x the 11 in 14 speeds at 5 kt or more = 19.6%.
    assert!((18.0..=21.0).contains(&east), "east {east:.1}%");
    assert!((18.0..=22.0).contains(&gusty), "gusty {gusty:.1}%");
    for w in &winds {
        assert!((2..=16).contains(&w.kt), "{w:?}");
        assert!((270..=340).contains(&w.dir) || (60..=130).contains(&w.dir), "{w:?}");
        if let Some(g) = w.gust {
            assert!((w.kt + 8..=w.kt + 15).contains(&g), "{w:?}");
        }
    }
}

#[test]
fn same_seed_same_wind() {
    for seed in 1..=50 {
        assert_eq!(World::shift(seed, Mode::Normal).wind(), weather::draw(seed));
    }
    assert_eq!(World::new(7).wind(), Wind::CALM, "Practice is calm, west flow");
}

#[test]
fn east_flow_runs_every_movement_with_no_incident() {
    let east = Wind { dir: 100, kt: 12, gust: None };
    for seed in 1..=10 {
        let (w, s, conflicts) = run_shift(seed, Mode::Peak, Some(east), 2 * TICKS_PER_REAL_H, |_| {});
        let t = w.traffic.as_ref().unwrap();
        let (arr, dep) = (t.log.iter().filter(|d| d.arrival).count(), t.log.iter().filter(|d| !d.arrival).count());
        assert!(t.log.iter().all(|d| ["11R", "10", "11L", "09"].contains(&d.rwy)), "seed {seed}: a west runway in east flow");
        println!(
            "seed {seed}, east: {arr} arrivals, {dep} departures; {} departed, {} landed, {} shut down; incidents {}; score {} movements, delay {:.1} game min over {} aircraft",
            s.departed, s.landed, s.on_stand, w.monitor.incidents.len(), w.score.movements, w.score.delay_min(), w.score.delayed()
        );
        assert_eq!(conflicts, 0, "seed {seed}: a conflict was live");
        assert!(w.monitor.incidents.is_empty(), "seed {seed}: {:?}", w.monitor.incidents.first());
        assert_eq!(s.departed as usize, dep, "seed {seed}: every departure got out");
        assert_eq!(s.on_stand as usize, arr, "seed {seed}: every arrival reached a stand and shut down");
        assert_eq!(w.score.movements, s.departed + s.landed, "seed {seed}: movements");
    }
}

#[test]
fn heavies_depart_the_long_runway() {
    for seed in 1..=20 {
        let mut s = twr_sim::traffic::Schedule::new(seed, Mode::Peak);
        let long = s.wind.flow().dep_rwys()[0];
        let dues: Vec<_> = (0..TICKS_PER_REAL_H / 2).flat_map(|t| s.tick(t)).collect();
        for d in dues.iter().filter(|d| !d.arrival && matches!(d.wake, Wake::H | Wake::J)) {
            assert_eq!(d.rwy, long, "seed {seed}: {} ({}) from {}", d.callsign, d.type_code, d.rwy);
        }
    }
}

/// Spawn `n` arrivals in a world with `wind` and count the windshear draws.
fn windshear_draws(wind: Wind, n: usize) -> usize {
    let mut w = World::shift(1, Mode::Light);
    w.traffic.as_mut().unwrap().schedule.wind = wind;
    (0..n).filter(|k| {
        let i = w.spawn_arrival(&format!("T{k}"), Wake::M, "29L").unwrap();
        w.aircraft[i].weather_ga.is_some()
    }).count()
}

#[test]
fn windshear_on_gusty_shifts_only_and_it_is_a_challenge() {
    let n = 10_000;
    let gusty = windshear_draws(Wind { dir: 290, kt: 14, gust: Some(26) }, n);
    let pct = gusty as f64 / n as f64 * 100.0;
    println!("{n} gusty arrivals: {gusty} windshear ({pct:.2}%; 3% of the 95% without a pilot go-around is 2.85%)");
    assert!((2.35..=3.35).contains(&pct), "{pct:.2}%");
    assert_eq!(windshear_draws(Wind { dir: 290, kt: 14, gust: None }, n), 0, "steady wind: none");

    // One, end to end: it goes around for weather, and it costs no delay.
    let mut w = World::shift(1, Mode::Light);
    w.traffic.as_mut().unwrap().schedule.wind = Wind { dir: 290, kt: 14, gust: Some(26) };
    let i = w.spawn_arrival("WS1", Wake::M, "29L").unwrap();
    w.aircraft[i].pilot_ga = None;
    w.aircraft[i].weather_ga = Some(2_000 * twr_sim::aircraft::MDM_PER_M);
    w.aircraft[i].cleared_to_land = true;
    let mut seen = None;
    for _ in 0..20_000 {
        w.step_ticks(1);
        if let Some(g) = w.aircraft[i].went_around {
            seen = Some(g);
            break;
        }
    }
    assert_eq!(seen, Some(GoAround::Weather("windshear")));
    assert!(seen.unwrap().challenge());
    assert_eq!(w.challenges(), 1);
    assert_eq!(w.score.delay_ticks, 0, "a weather go-around is not delay");
}
