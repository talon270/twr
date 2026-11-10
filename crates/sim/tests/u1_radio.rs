//! PLAN-ui-console A2: the radio line grammar, v1. The composer may offer
//! only what the sim would accept, and every line it builds must parse to
//! the verb key the strip hint would press.

use twr_sim::radio::{next_tokens, parse, PHRASES};
use twr_sim::{Aircraft, Phase, World};

/// A world with one departure on its stand and one shut-down aircraft.
fn world() -> (World, usize, usize) {
    let mut w = World::empty(1);
    let stand = w.airport.stands[0].node;
    let other = w.airport.stands[1].node;
    let on_stand = w.add(Aircraft::parked("IGO512", stand));
    let gone = w.add(Aircraft { phase: Phase::ShutDown, ..Aircraft::parked("AIC304", other) });
    (w, on_stand, gone)
}

#[test]
fn empty_line_offers_only_aircraft_that_accept_a_verb() {
    let (w, i, gone) = world();
    assert!(!w.verbs(i).is_empty(), "the parked departure accepts something");
    assert!(w.verbs(gone).is_empty(), "a shut-down aircraft accepts nothing");
    assert_eq!(next_tokens(&w, ""), vec!["IGO512".to_string()]);
    assert_eq!(next_tokens(&w, "ig"), vec!["IGO512".to_string()]);
    assert!(next_tokens(&w, "AIC").is_empty(), "the shut-down callsign is never offered");
}

#[test]
fn next_words_are_exactly_the_verbs_the_sim_accepts() {
    let (w, i, _) = world();
    let ok = w.verbs(i);
    let want: Vec<String> = PHRASES
        .iter()
        .filter(|(k, _)| ok.contains(k))
        .map(|(_, p)| p.split(' ').next().unwrap().to_string())
        .collect();
    assert_eq!(next_tokens(&w, "IGO512 "), want, "verbs {ok:?}");
    // A parked aircraft can push, and cannot take off.
    assert!(want.contains(&"PUSHBACK".to_string()));
    assert!(!want.contains(&"TAKEOFF".to_string()));
}

#[test]
fn a_partial_word_completes_and_a_finished_phrase_offers_nothing() {
    let (w, _, _) = world();
    assert_eq!(next_tokens(&w, "igo512 pu"), vec!["PUSHBACK".to_string()]);
    assert!(next_tokens(&w, "IGO512 PUSHBACK ").is_empty());
    assert!(next_tokens(&w, "NOBODY ").is_empty());
}

#[test]
fn every_offered_line_parses_to_its_verb() {
    let (w, i, _) = world();
    for &(k, phrase) in PHRASES {
        assert_eq!(parse(&w, &format!("igo512  {}", phrase.to_lowercase())), Ok((i, k)), "{phrase}");
    }
}

#[test]
fn bad_lines_say_why() {
    let (w, _, _) = world();
    assert_eq!(parse(&w, "  "), Err("type a callsign".into()));
    assert_eq!(parse(&w, "XYZ123 TAXI"), Err("no aircraft XYZ123".into()));
    assert_eq!(parse(&w, "IGO512"), Err("IGO512: say the instruction".into()));
    assert_eq!(parse(&w, "IGO512 FLY"), Err("IGO512: \"FLY\" is not an instruction".into()));
}
