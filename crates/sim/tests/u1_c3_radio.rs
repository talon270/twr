//! C3 radio arguments come from the live graph and aircraft, not the page.
use twr_sim::radio::{hold_point_name, next_tokens, parse_command, Command};
use twr_sim::{Phase, World};

#[test]
fn hold_at_completes_graph_names_and_numbers_and_rejects_stale_points() {
    let mut w = World::scenario(1, "ground").unwrap();
    let i = 1;
    let n = w.aircraft[i].hold_points(&w.graph)[2];
    let label = hold_point_name(&w, n);
    let cs = w.aircraft[i].callsign.clone();
    assert_eq!(parse_command(&w, &format!("{cs} HOLD AT {n}")), Ok((i, Command::HoldAt(n))));
    let line = format!("{cs} HOLD AT {label}");
    assert_eq!(parse_command(&w, &line.to_lowercase()), Ok((i, Command::HoldAt(n))));
    let mut prefix = String::new();
    for token in line.split_whitespace() {
        assert!(next_tokens(&w, &prefix).contains(&token.to_string()), "{prefix:?} must offer {token}");
        prefix.push_str(token);
        prefix.push(' ');
    }
    assert!(next_tokens(&w, &prefix).is_empty());
    w.hold_at(i, n).unwrap();
    assert_eq!(parse_command(&w, &format!("{cs} CONTINUE")), Ok((i, Command::Key('\r'))));
    assert!(next_tokens(&w, &format!("{cs} C")).contains(&"CONTINUE".into()));
    while w.aircraft[i].phase == Phase::Taxi { w.step_ticks(1); }
    assert!(parse_command(&w, &format!("{cs} HOLD AT {n}")).is_err());
    w.resume(i).unwrap();
    assert_eq!(w.aircraft[i].phase, Phase::Taxi);
    assert!(parse_command(&w, &format!("{cs} HOLD AT 4294967295")).is_err());
    println!("hold radio: {label}; names and numbers complete, reached and invalid points refused; CONTINUE resumes");
}

#[test]
fn give_way_parses_callsigns_and_excludes_invalid_targets_from_completion() {
    let w = World::scenario(1, "ground").unwrap();
    assert_eq!(parse_command(&w, "igo602 give way to igo601"), Ok((1, Command::GiveWay(0))));
    assert!(next_tokens(&w, "IGO602 GIVE WAY TO ").is_empty(), "the other aircraft is parked; it cannot be offered");
    assert_eq!(parse_command(&w, "IGO602 GIVE WAY TO MISSING"), Err("no aircraft MISSING".into()));
    println!("give-way radio: callsign parsed; stationary and unknown targets not offered");
}

#[test]
fn c3_speed_backtrack_cross_and_continue_words_parse() {
    let w = World::scenario(1, "ground").unwrap();
    for (words, key) in [("CONTINUE", '\r'), ("BACKTRACK", 'B'), ("TAXI SLOWER", '-'), ("TAXI FASTER", '='), ("CROSS IF CLEAR", 'X')] {
        assert_eq!(parse_command(&w, &format!("IGO602 {words}")), Ok((1, Command::Key(key))));
    }
    assert_eq!(parse_command(&w, "IGO602 EXPEDITE TAXI"), Ok((1, Command::Expedite)));
    assert!(next_tokens(&w, "IGO602 TAXI ").contains(&"SLOWER".into()));
    assert_eq!(next_tokens(&w, "IGO602 EXPEDITE "), vec!["TAXI"]);
    println!("C3 radio: CONTINUE, BACKTRACK, TAXI SLOWER/FASTER, CROSS IF CLEAR, EXPEDITE TAXI parsed");
}

#[test]
fn give_way_completion_dispatches_and_releases_at_the_junction() {
    let mut w = World::scenario(1, "c3-give-way").unwrap();
    let before = format!("{:?}", w.aircraft);
    assert_eq!(w.give_way_targets(0), vec![1]);
    assert_eq!(next_tokens(&w, "IGO601 GIVE WAY TO "), vec!["IGO602"]);
    assert_eq!(next_tokens(&w, "IGO601 GIVE WAY TO igo6"), vec!["IGO602"]);
    assert_eq!(format!("{:?}", w.aircraft), before, "completion must not change the world");
    let (i, Command::GiveWay(j)) = parse_command(&w, "IGO601 GIVE WAY TO IGO602").unwrap() else { panic!("give-way command") };
    w.give_way(i, j).unwrap();
    let mut held = false;
    let mut released = false;
    for _ in 0..10_000 {
        w.step_ticks(1);
        held |= w.aircraft[i].phase == Phase::HoldPosition;
        if held && w.aircraft[i].stop.is_none() {
            released = true;
            break;
        }
    }
    assert!(held && released);
    assert_eq!(w.aircraft[i].phase, Phase::Taxi);
    assert!(w.monitor.incidents.is_empty());
    println!("give-way completion: IGO602 offered; no mutation; held, auto-released, 0 incidents");
}

#[test]
fn repeated_taxiway_names_need_a_point_number() {
    let mut w = World::scenario(1, "ground").unwrap();
    let points = w.aircraft[1].hold_points(&w.graph);
    // Name two legal points identically to exercise ambiguity independently
    // of the airport's current naming coverage.
    for &node in &points[..2] {
        w.airport.holds.push(twr_sim::airport::Hold { node, name: "TEST POINT".into() });
    }
    let error = parse_command(&w, "IGO602 HOLD AT TEST POINT").unwrap_err();
    assert!(error.contains("names several points"));
    let label = hold_point_name(&w, points[0]);
    assert_eq!(parse_command(&w, &format!("IGO602 HOLD AT {label}")), Ok((1, Command::HoldAt(points[0]))));
    w.airport.holds.pop();
    assert_eq!(parse_command(&w, "IGO602 HOLD AT TEST POINT"), Ok((1, Command::HoldAt(points[0]))));
    println!("hold names: unique multiword name accepted; duplicate refused; node-qualified label accepted");
}

#[test]
fn browser_fixtures_offer_cross_ahead_and_backtrack() {
    let mut w = World::scenario(1, "c3-cross").unwrap();
    assert!(next_tokens(&w, "IGO520 CROSS ").contains(&"IF".into()));
    w.cross(0).unwrap();
    assert!(w.aircraft[0].cross_ahead.is_some());
    let mut w = World::scenario(1, "c3-backtrack").unwrap();
    assert_eq!(next_tokens(&w, "IGO530 B"), vec!["BACKTRACK"]);
    w.backtrack(0).unwrap();
    assert_eq!(w.aircraft[0].backtrack.as_deref(), Some("27"));
    println!("browser fixtures: early X accepted; B accepted at short entry 5 for 27");
}
