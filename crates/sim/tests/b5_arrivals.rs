//! PLAN-twr.md § B5, proof: the arrival flow and its rules, each broken on
//! purpose and checked at its boundary.

use twr_sim::aircraft::{LAND_DECEL_MDM, LAND_MDM_PER_TICK, MDM_PER_M, TAXI_MDM_PER_TICK};
use twr_sim::arrival::{brake_mdm, candidates, exits, threshold, DECISION_M, EXIT_MAX_TURN_DEG, GA_CIRCUIT_S};
use twr_sim::conflict::on_runway;
use twr_sim::scenario::{crossing, entry};
use twr_sim::{Aircraft, GoAround, Phase, Rule, Wake, World, PAGE_ARRIVAL_RWY, TICK_HZ};

fn empty() -> World {
    let mut w = World::new(1);
    w.aircraft.clear();
    w
}

fn rules(w: &World) -> Vec<Rule> {
    w.monitor.incidents.iter().map(|i| i.rule).collect()
}

fn until(w: &mut World, max: u32, done: impl Fn(&World) -> bool) -> u32 {
    for t in 0..max {
        if done(w) {
            return t;
        }
        w.step_ticks(1);
    }
    panic!("not done after {max} ticks; incidents {:?}", w.monitor.incidents);
}

/// An arrival on final for `desig`, cleared, no pilot go-around, `out_m`
/// before the threshold (0: it crosses on the next tick).
fn arrival(w: &mut World, cs: &str, desig: &str, out_m: i64) -> usize {
    let i = w.spawn_arrival(cs, Wake::M, desig).unwrap();
    let a = &mut w.aircraft[i];
    a.pilot_ga = None;
    a.cleared_to_land = true;
    a.air_mdm = -(out_m * MDM_PER_M as i64).max(LAND_MDM_PER_TICK as i64);
    i
}

fn on_rwy(w: &World, i: usize) -> bool {
    on_runway(&w.aircraft[i], &w.airport, &w.graph).is_some()
}

// ---- R5: the threshold -------------------------------------------------------

/// A departure still rolling or over the runway: crossing the threshold is
/// an incident. On the tick after it is Departed: nothing.
#[test]
fn r5_departure_ahead() {
    for wait_for_departed in [false, true] {
        let mut w = empty();
        let d = w.add_lined_up("DEP", Wake::M, entry(&w, "29L", true).unwrap()).unwrap();
        w.takeoff(d).unwrap();
        until(&mut w, 10_000, |w| {
            if wait_for_departed { matches!(w.aircraft[d].phase, Phase::Departed(_)) } else { matches!(w.aircraft[d].phase, Phase::Airborne(_)) }
        });
        arrival(&mut w, "ARR", "29L", 0);
        w.step_ticks(1);
        assert!(matches!(w.aircraft[1].phase, Phase::Landing(_)));
        if wait_for_departed {
            assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
        } else {
            assert_eq!(rules(&w), [Rule::Occupied]);
            assert!(w.monitor.incidents[0].text.contains("ARR crossed the 29L threshold with DEP"), "{}", w.monitor.incidents[0].text);
            // The roll that follows does not count the same pair again.
            w.step_ticks(400);
            assert_eq!(w.monitor.incidents.len(), 1, "{:?}", w.monitor.incidents);
        }
    }
}

/// Lined up, crossing, or an earlier arrival not yet vacated: each one on
/// the runway at the threshold tick is an incident.
#[test]
fn r5_lined_up_crosser_and_earlier_arrival() {
    // Lined up at an intermediate entry.
    let mut w = empty();
    w.add_lined_up("LUP", Wake::M, entry(&w, "29L", false).unwrap()).unwrap();
    arrival(&mut w, "ARR", "29L", 0);
    w.step_ticks(1);
    assert_eq!(rules(&w), [Rule::Occupied], "{:?}", w.monitor.incidents);

    // A crosser on it, then the same crosser one tick after it is off. On
    // 29R: no stand-to-runway route at IGI crosses 29L/11R first.
    let c = crossing(&empty(), "29R", 600.0).expect("a route crossing 29R");
    for off in [false, true] {
        let mut w = empty();
        let x = w.add_taxied("CROSS", c.clone()).unwrap();
        w.cross(x).unwrap();
        until(&mut w, 2_000, |w| on_rwy(w, x));
        if off {
            until(&mut w, 2_000, |w| !on_rwy(w, x));
        }
        arrival(&mut w, "ARR", "29R", 0);
        w.step_ticks(1);
        if off {
            assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
        } else {
            assert_eq!(rules(&w), [Rule::Occupied], "{:?}", w.monitor.incidents);
        }
    }

    // An earlier arrival still on its exit, then one tick after it vacated.
    for vacated in [false, true] {
        let mut w = empty();
        let a = arrival(&mut w, "ARR1", "29L", 0);
        until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Taxi));
        if vacated {
            until(&mut w, 10_000, |w| !on_rwy(w, a));
        }
        arrival(&mut w, "ARR2", "29L", 0);
        w.step_ticks(1);
        if vacated {
            assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
        } else {
            assert_eq!(rules(&w), [Rule::Occupied], "{:?}", w.monitor.incidents);
            assert!(w.monitor.incidents[0].text.contains("with ARR1"));
        }
    }
}

// ---- A10, A11 ----------------------------------------------------------------

/// A10: the runway is free on the tick the arrival passes its vacate point,
/// not before. K two ticks early is an incident (one tick early, the arrival
/// is off before the next check); K on that tick is not.
#[test]
fn a10_departure_on_the_tick_the_arrival_vacates() {
    for early in [true, false] {
        let mut w = empty();
        let a = arrival(&mut w, "ARR", "29L", 0);
        w.step_ticks(1);
        // Line up behind the roll once it is well down the runway.
        until(&mut w, 10_000, |w| w.aircraft[a].pos_mdm > 500 * MDM_PER_M);
        let d = w.add_lined_up("DEP", Wake::M, entry(&w, "29L", true).unwrap()).unwrap();
        let vac = w.aircraft[a].exit.as_ref().unwrap().vacate_mdm;
        until(&mut w, 10_000, |w| w.aircraft[a].pos_mdm + if early { 2 * TAXI_MDM_PER_TICK } else { 0 } >= vac);
        assert_eq!(on_rwy(&w, a), early, "on the runway until the vacate point");
        assert_eq!(w.runway_blocker(d).is_some(), early);
        w.takeoff(d).unwrap();
        until(&mut w, 10_000, |w| matches!(w.aircraft[d].phase, Phase::Departed(_)));
        if early {
            assert_eq!(rules(&w), [Rule::Occupied], "{:?}", w.monitor.incidents);
        } else {
            assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
        }
        assert_eq!(w.aircraft[a].phase, Phase::Holding, "vacated and waiting for T");
    }
}

/// A11: after the exit node the arrival is a ground mover, so the ground
/// check catches it rolling into an aircraft stopped on its exit.
#[test]
fn a11_exit_into_a_stopped_aircraft() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", "29L", 0);
    let end = w.aircraft[a].route.end();
    w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked("STOP", end) });
    until(&mut w, 10_000, |w| !w.monitor.conflicts.is_empty());
    assert_eq!(w.aircraft[a].phase, Phase::Taxi, "conflict raised once off the runway");
    until(&mut w, 10_000, |w| !w.monitor.incidents.is_empty());
    assert_eq!(rules(&w), [Rule::Ground]);
}

/// A9: late braking. 29L to its default exit takes the time the profile
/// says: at 70 m/s to the braking point, then 1.6 m/s² to 10 m/s.
#[test]
fn a9_runway_occupancy_29l() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", "29L", 0);
    w.step_ticks(1);
    let x = w.aircraft[a].exit.clone().unwrap();
    let to_exit = until(&mut w, 20_000, |w| matches!(w.aircraft[a].phase, Phase::Taxi));
    let to_vacate = to_exit + until(&mut w, 20_000, |w| !on_rwy(w, a));
    let brake = brake_mdm(LAND_MDM_PER_TICK);
    let expect = (x.at_mdm - brake) as f64 / LAND_MDM_PER_TICK as f64 + (LAND_MDM_PER_TICK - TAXI_MDM_PER_TICK) as f64 / LAND_DECEL_MDM as f64;
    let hz = f64::from(TICK_HZ);
    println!(
        "29L exit {} at {} m: threshold to exit {:.2} s (profile {:.2} s), to vacate {:.2} s",
        x.label(),
        x.at_mdm / MDM_PER_M,
        f64::from(to_exit) / hz,
        expect / hz,
        f64::from(to_vacate) / hz
    );
    assert_eq!(x.label(), "Z2");
    assert!((f64::from(to_exit) - expect).abs() <= 2.0, "{to_exit} ticks vs {expect:.1}");
    assert!(f64::from(to_vacate) / hz < 80.0);
}

// ---- Verbs -------------------------------------------------------------------

#[test]
fn verbs_refuse_with_a_reason() {
    let mut w = empty();
    let dep = w.add_lined_up("DEP", Wake::M, entry(&w, "29L", true).unwrap()).unwrap();
    assert!(w.clear_to_land(dep).unwrap_err().contains("only an aircraft on final"));
    assert!(w.go_around(dep).unwrap_err().contains("only an aircraft on final"));

    let a = arrival(&mut w, "ARR", "29R", 6_000);
    w.aircraft[a].cleared_to_land = false;
    assert_eq!(w.verbs(a), ['C', 'G', 'E']);
    w.clear_to_land(a).unwrap();
    assert!(w.clear_to_land(a).unwrap_err().contains("already"));

    // E: too close to slow, from final.
    let (ap, g) = (&w.airport, &w.graph);
    let near = exits(ap, g, "29R").into_iter().find(|(x, _)| x.at_mdm < 1_500 * MDM_PER_M).expect("an exit under 1,500 m on 29R").0;
    let e = w.set_exit(a, near.node).unwrap_err();
    assert!(e.contains("needs 1500 m to slow"), "{e}");

    // E: one that turns back up the runway.
    let (ap, g) = (&w.airport, &w.graph);
    let back = candidates(ap, g, "29R").into_iter().find(|c| c.turn_deg > EXIT_MAX_TURN_DEG && !exits(ap, g, "29R").iter().any(|(x, _)| x.node == c.node)).unwrap();
    assert!(w.set_exit(a, back.node).unwrap_err().contains("turns back against 29R"));
    let not = threshold(&w.airport, &w.graph, "29L").unwrap().0;
    assert!(w.set_exit(a, not).unwrap_err().contains("not an exit off 29R"));

    // G on the runway is too late; E to an exit already passed is refused.
    until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Landing(_)));
    assert!(w.go_around(a).unwrap_err().contains("too late"));
    until(&mut w, 10_000, |w| w.aircraft[a].pos_mdm > near.at_mdm);
    assert!(w.set_exit(a, near.node).unwrap_err().contains("is behind"));
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// E to a later exit: the aircraft leaves there, through the route it was
/// given, and holds at that exit's vacate point.
#[test]
fn e_picks_a_later_exit() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", "29R", 3_000);
    let first = w.aircraft[a].exit.clone().unwrap();
    let later = exits(&w.airport, &w.graph, "29R").into_iter().find(|(x, _)| x.at_mdm > first.at_mdm + 500 * MDM_PER_M).unwrap();
    w.set_exit(a, later.0.node).unwrap();
    until(&mut w, 20_000, |w| w.aircraft[a].phase == Phase::Holding);
    assert_eq!(w.aircraft[a].route, later.1);
    assert_eq!(w.aircraft[a].trail, later.1.edges, "rolled the route it was given, edge for edge");
    println!("29R: default {} at {} m, chose {} at {} m", first.label(), first.at_mdm / MDM_PER_M, later.0.label(), later.0.at_mdm / MDM_PER_M);
}

// ---- Go-arounds ----------------------------------------------------------------

/// No C: goes around at the decision point, out of sight on the circuit,
/// back on final after GA_CIRCUIT_S. With C: lands. Neither is an incident.
#[test]
fn go_around_without_clearance_and_back_on_final() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", "29L", 3_000);
    w.aircraft[a].cleared_to_land = false;
    until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::GoAround(_)));
    let out = w.aircraft[a].final_m();
    assert!(out <= DECISION_M as f64 && out > DECISION_M as f64 - 3.6, "went around {out} m out");
    assert_eq!(w.go_arounds.len(), 1);
    assert_eq!(w.go_arounds[0].why, GoAround::NoClearance);
    assert!(!w.go_arounds[0].why.challenge());
    until(&mut w, 10_000, |w| !w.aircraft[a].visible());
    let hidden = until(&mut w, 100_000, |w| matches!(w.aircraft[a].phase, Phase::Final(_)));
    assert_eq!(u64::from(hidden), GA_CIRCUIT_S * u64::from(TICK_HZ) + 1);
    assert!(!w.aircraft[a].cleared_to_land);
    w.clear_to_land(a).unwrap();
    w.aircraft[a].pilot_ga = None;
    until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Landing(_)));
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// A pilot go-around comes even after C, is a challenge, and counts as no
/// incident. G is logged as instructed.
#[test]
fn pilot_and_instructed_go_arounds() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", "29L", 5_000);
    w.aircraft[a].pilot_ga = Some((2_000 * MDM_PER_M, "unstable approach"));
    until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::GoAround(_)));
    assert!((w.aircraft[a].final_m() - 2_000.0).abs() < 3.6);
    assert_eq!(w.go_arounds[0].why, GoAround::Pilot("unstable approach"));
    assert!(w.go_arounds[0].why.challenge());

    let b = arrival(&mut w, "ARR2", "29R", 5_000);
    w.go_around(b).unwrap();
    assert_eq!(w.go_arounds[1].why, GoAround::Instructed);
    assert!(w.monitor.incidents.is_empty());
}

/// The pilot draw hits about PILOT_GA_PCT of arrivals, from the seed alone.
#[test]
fn pilot_go_around_rate_is_seeded() {
    let count = |seed: u64| {
        let mut w = World::new(seed);
        (0..1000).filter(|k| {
            let i = w.spawn_arrival(&format!("A{k}"), Wake::M, "29L").unwrap();
            w.aircraft[i].pilot_ga.is_some()
        }).count()
    };
    let n = count(3);
    println!("pilot go-arounds drawn: {n} of 1000");
    assert!((30..=70).contains(&n), "{n}");
    assert_eq!(n, count(3));
}

// ---- The whole flow --------------------------------------------------------------

/// Final, C, land, default exit, T to a free stand, X at any crossing:
/// on stand, having rolled the landing route and then the drawn taxi route.
#[test]
fn final_to_on_stand() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", PAGE_ARRIVAL_RWY, 8_000);
    let landing = w.aircraft[a].route.clone();
    until(&mut w, 20_000, |w| w.aircraft[a].phase == Phase::Holding);
    assert_eq!(w.aircraft[a].trail, landing.edges);
    // Vacated, within reach of the runway: T only, never L.
    assert_eq!(w.verbs(a), ['T']);
    assert!(w.line_up(a).unwrap_err().contains("taxis to a stand"));
    let stands: Vec<u32> = w.airport.stands.iter().map(|s| s.node).collect();
    let taxi = stands.iter().rev().find_map(|&s| w.plan(a, s, &[], &[])).unwrap();
    w.taxi(a, taxi.clone()).unwrap();
    let mut crossings = 0;
    while w.aircraft[a].phase != Phase::OnStand {
        until(&mut w, 200_000, |w| matches!(w.aircraft[a].phase, Phase::HoldShort(_) | Phase::OnStand));
        if matches!(w.aircraft[a].phase, Phase::HoldShort(_)) {
            w.cross(a).unwrap();
            crossings += 1;
        }
    }
    assert_eq!(w.aircraft[a].trail, [landing.edges, taxi.edges.clone()].concat());
    assert_eq!(w.verbs(a), ['P'], "on stand: ready to push back later");
    println!("final to on-stand: {} taxi edges, {crossings} crossings, {} s game", taxi.edges.len(), w.tick / u64::from(TICK_HZ));
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// T to a stand someone is on is refused, with who is on it.
#[test]
fn occupied_stand_is_refused() {
    let mut w = empty();
    let a = arrival(&mut w, "ARR", "29L", 0);
    until(&mut w, 20_000, |w| w.aircraft[a].phase == Phase::Holding);
    let s = w.airport.stands.iter().rev().find(|s| w.plan(a, s.node, &[], &[]).is_some()).unwrap().clone();
    w.add(Aircraft::parked("PARK", s.node));
    let r = w.plan(a, s.node, &[], &[]).unwrap();
    let e = w.taxi(a, r).unwrap_err();
    assert_eq!(e, format!("stand {} occupied by PARK", s.name));
}

/// The page's world: one departure on a stand and IGO713 on final for 29L.
/// An hour of it with nobody at the controls replays exactly.
#[test]
fn page_world_has_an_arrival_and_replays() {
    let run = |seed| {
        let mut w = World::new(seed);
        assert_eq!(w.aircraft[1].callsign, "IGO713");
        assert_eq!(w.aircraft[1].phase, Phase::Final("29L".into()));
        w.step_ticks(72_000);
        (format!("{:?}", w.aircraft.iter().map(|a| (&a.phase, a.air_mdm, a.pos_mdm)).collect::<Vec<_>>()), w.go_arounds.clone(), w.monitor.incidents.clone())
    };
    let (a, b) = (run(5), run(5));
    assert_eq!(a, b);
    println!("one hour, nobody at the controls: {} go-arounds", a.1.len());
    assert!(a.1.len() >= 5, "goes around every circuit");
}
