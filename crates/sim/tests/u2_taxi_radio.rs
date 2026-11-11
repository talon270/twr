//! Typed taxi routes and `CROSS RUNWAY` on the radio line (ground-commands
//! REPORT rows 1, 6, 7): the words come from the live graph, the route the
//! sim plans is the one a click on the same taxiway would plan.
use twr_sim::radio::{next_tokens, parse_command, Command};
use twr_sim::{Phase, World};

/// Practice's IGO512, pushed back and waiting for taxi.
fn holding() -> (World, usize) {
    let mut w = World::new(1);
    let i = w.aircraft.iter().position(|a| a.callsign == "IGO512").unwrap();
    w.push(i).unwrap();
    for _ in 0..20_000 {
        if w.aircraft[i].phase == Phase::Holding {
            break;
        }
        w.step_ticks(1);
    }
    assert_eq!(w.aircraft[i].phase, Phase::Holding, "pushback ends Holding");
    (w, i)
}

#[test]
fn taxi_to_a_stand_or_runway_completes_parses_and_sends() {
    let (mut w, i) = holding();
    let cs = w.aircraft[i].callsign.clone();
    assert!(next_tokens(&w, &format!("{cs} TAXI ")).contains(&"TO".to_string()));
    assert_eq!(next_tokens(&w, &format!("{cs} TAXI TO ")), vec!["RUNWAY", "STAND"].iter().rev().map(|s| s.to_string()).collect::<Vec<_>>());
    let stands = next_tokens(&w, &format!("{cs} TAXI TO STAND "));
    assert!(!stands.is_empty() && stands.len() <= 24, "capped without a typed letter: {}", stands.len());
    let name = stands[0].clone();
    assert!(next_tokens(&w, &format!("{cs} TAXI TO STAND {}", &name[..1])).len() > 1, "a typed letter lifts the cap");
    assert_eq!(next_tokens(&w, &format!("{cs} TAXI TO STAND {name} ")), vec!["VIA"]);

    let Ok((_, Command::Taxi { to, via })) = parse_command(&w, &format!("{cs} taxi to stand {name}")) else { panic!("stand did not parse") };
    assert!(via.is_empty());
    assert_eq!(w.airport.stands.iter().find(|s| s.name.eq_ignore_ascii_case(&name)).unwrap().node, to);

    let Ok((_, Command::Taxi { to, via })) = parse_command(&w, &format!("{cs} TAXI TO RUNWAY 29R")) else { panic!("runway did not parse") };
    let route = w.plan(i, to, &via, &[]).expect("a route to 29R");
    assert!(w.graph.runway_run(&w.airport, to).is_some_and(|(_, d)| d == "29R"), "the entry leads onto 29R");
    w.taxi(i, route).unwrap();
    assert_eq!(w.aircraft[i].phase, Phase::Taxi);
    println!("taxi radio: stand {name} and runway 29R parse, plan and send; {} stand names offered unfiltered", stands.len());
}

#[test]
fn via_picks_the_taxiway_and_bad_names_say_why() {
    let (w, i) = holding();
    let cs = w.aircraft[i].callsign.clone();
    let Ok((_, Command::Taxi { to, via: none })) = parse_command(&w, &format!("{cs} TAXI TO RUNWAY 29R")) else { panic!() };
    let plain = w.plan(i, to, &none, &[]).unwrap();
    // A named taxiway the plain route does not use, so VIA changes the route.
    let on_plain: Vec<&str> = plain.edges.iter().map(|&e| w.airport.edges[e as usize].name.as_str()).collect();
    let other = w
        .airport
        .edges
        .iter()
        .filter(|e| e.kind == twr_sim::airport::EdgeKind::Taxiway && !e.name.is_empty() && !on_plain.contains(&e.name.as_str()))
        .map(|e| e.name.clone())
        .find(|n| matches!(parse_command(&w, &format!("{cs} TAXI TO RUNWAY 29R VIA {n}")), Ok((_, Command::Taxi { .. }))))
        .expect("some taxiway can be forced into the route");
    let Ok((_, Command::Taxi { to: t2, via })) = parse_command(&w, &format!("{cs} TAXI TO RUNWAY 29R VIA {other}")) else { panic!() };
    let routed = w.plan(i, t2, &via, &[]).unwrap();
    assert!(routed.edges.iter().any(|&e| w.airport.edges[e as usize].name == other), "VIA {other} puts {other} in the route");
    assert!(w.graph.run_len_dm(&routed) >= w.graph.run_len_dm(&plain), "a forced taxiway never makes the route shorter");
    assert!(next_tokens(&w, &format!("{cs} TAXI TO RUNWAY 29R VIA ")).iter().all(|t| t != "VIA"));
    for (line, why) in [
        ("TAXI TO STAND NOPE9", "no stand"),
        ("TAXI TO RUNWAY 99", "no entry"),
        ("TAXI TO RUNWAY 29R VIA ZZZ9", "no taxiway"),
        ("TAXI TO", "say where"),
        ("TAXI TO BAY 3", "STAND or RUNWAY"),
        ("TAXI TO STAND A06 FAST", "only VIA"),
    ] {
        let e = parse_command(&w, &format!("{cs} {line}")).unwrap_err();
        assert!(e.contains(why), "{line:?} -> {e:?} should mention {why:?}");
    }
    println!("via radio: VIA {other} forces {other} into the 29R route ({} -> {} dm); six bad lines refused with a reason", w.graph.run_len_dm(&plain), w.graph.run_len_dm(&routed));
}

#[test]
fn cross_runway_needs_the_runway_the_aircraft_holds_short_of() {
    let mut w = World::scenario(1, "c3-cross").unwrap();
    let i = (0..w.aircraft.len()).find(|&k| w.aircraft[k].callsign == "IGO520").unwrap();
    for _ in 0..40_000 {
        if matches!(w.aircraft[i].phase, Phase::HoldShort(_)) {
            break;
        }
        w.step_ticks(1);
    }
    let Phase::HoldShort(r) = w.aircraft[i].phase.clone() else { panic!("IGO520 never held short: {:?}", w.aircraft[i].phase) };
    let d = r.split('/').next().unwrap().to_string();
    assert!(next_tokens(&w, &format!("IGO520 CROSS RUNWAY ")).contains(&d.to_ascii_uppercase()));
    assert_eq!(parse_command(&w, &format!("IGO520 cross runway {d}")), Ok((i, Command::Key('X'))));
    let e = parse_command(&w, "IGO520 CROSS RUNWAY 99").unwrap_err();
    assert!(e.contains("holding short of"), "{e}");
    println!("cross runway: holding short of {r}; CROSS RUNWAY {d} parses to X, runway 99 refused");
}
