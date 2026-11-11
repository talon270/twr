//! RADIO LINE · the typed command grammar, v1 (PLAN-ui-console A2)
//!
//! · PHRASES      the instruction words for each sim verb key
//! · next_tokens  the words that may legally come next on a partial line
//! · parse        a complete line to (aircraft index, verb key)
//!
//! Rust is the only source of legal actions. The page's composer asks
//! `next_tokens` what to offer and sends a finished line through
//! `parse_command`; it never keeps its own list of verbs. Completions use
//! `World::verbs`, `ground_verbs` and argument validation. Movement after
//! completion can make a command stale; dispatch then reports the refusal.
//!
//! Limits of v1, stated where the claim is made: the words follow the root
//! forms in `research/out/phraseology/grammar.md`. C3 adds point and callsign
//! arguments; HOLD AT, CONTINUE, TAXI FASTER and CROSS IF CLEAR are game
//! shorthand, not a claim of complete ICAO phraseology.
//! `LINE UP`, `TAKEOFF` and `LAND` name no runway, because the sim verbs
//! take none: each aircraft already has one assigned. `TAXI` and `EXIT` open
//! the same map pickers the T and E keys open. Typed routes (2026-11-11,
//! ground-commands REPORT rows 1, 6 and 7): `TAXI TO STAND <name>` or
//! `TAXI TO RUNWAY <designator>`, then optionally `VIA <taxiway> ...`, and
//! `CROSS RUNWAY <designator>` while holding short of it. A typed VIA name
//! picks, for each taxiway, the edge that makes the shortest route; it is
//! the map builder's click, chosen by the sim. The grammar's minimum
//! prefixes are still v2.

use crate::World;

/// Verb key -> instruction words, in strip-hint order. Root forms from
/// grammar.md; `HOLD POSITION` because bare `HOLD` there also starts
/// `HOLD SHORT` and `HOLD AT`.
pub const PHRASES: &[(char, &str)] = &[
    ('P', "PUSHBACK"),
    ('T', "TAXI"),
    ('H', "HOLD POSITION"),
    ('\r', "CONTINUE"),
    ('B', "BACKTRACK"),
    ('-', "TAXI SLOWER"),
    ('=', "TAXI FASTER"),
    ('X', "CROSS IF CLEAR"),
    ('X', "CROSS"),
    ('L', "LINE UP"),
    ('K', "TAKEOFF"),
    ('C', "LAND"),
    ('G', "GO AROUND"),
    ('E', "EXIT"),
];

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_ascii_uppercase).collect()
}

/// The aircraft a callsign names, among those that accept any verb now.
/// An aircraft that accepts nothing (departed, shut down, rolling) is not
/// on the frequency as far as the composer is concerned.
fn find(w: &World, cs: &str) -> Option<usize> {
    (0..w.aircraft.len()).find(|&i| w.aircraft[i].callsign.eq_ignore_ascii_case(cs) && !legal_keys(w, i).is_empty())
}

/// Words that may come next. A line ending in a space (or empty) asks for
/// the next word; otherwise the last word is a partial and only words it
/// begins are offered. Callsigns come first, then the instruction words of
/// the verbs that aircraft accepts now.
pub fn next_tokens(w: &World, line: &str) -> Vec<String> {
    let mut ws = words(line);
    let partial = if line.is_empty() || line.ends_with(char::is_whitespace) { String::new() } else { ws.pop().unwrap_or_default() };
    let mut out: Vec<String> = Vec::new();
    let mut push = |t: &str| {
        if t.starts_with(partial.as_str()) && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    };
    if ws.is_empty() {
        for i in 0..w.aircraft.len() {
            if !legal_keys(w, i).is_empty() {
                push(&w.aircraft[i].callsign.to_ascii_uppercase());
            }
        }
        return out;
    }
    let Some(i) = find(w, &ws[0]) else { return out };
    let rest = &ws[1..];
    let ok = legal_keys(w, i);
    for &(k, phrase) in PHRASES {
        if !ok.contains(&k) {
            continue;
        }
        let pw = words(phrase);
        if rest.len() < pw.len() && pw[..rest.len()] == *rest {
            push(&pw[rest.len()]);
        }
    }
    for phrase in argument_phrases(w, i) {
        let pw = words(&phrase);
        if rest.len() < pw.len() && pw[..rest.len()] == *rest {
            push(&pw[rest.len()]);
        }
    }
    if ok.contains(&'T') {
        for t in taxi_words(w, i, rest, &partial) {
            push(&t);
        }
    }
    out
}

/// How many stand or taxiway names to offer before the user has typed a
/// letter: 270 stands as buttons would bury the radio line.
const OFFER_CAP: usize = 24;

/// The next words of `TAXI TO STAND|RUNWAY <x> [VIA <taxiway> ...]`.
fn taxi_words(w: &World, i: usize, rest: &[String], partial: &str) -> Vec<String> {
    let cap = |mut v: Vec<String>| {
        v.sort();
        v.dedup();
        v.retain(|t| t.starts_with(partial));
        if partial.is_empty() {
            v.truncate(OFFER_CAP);
        }
        v
    };
    let r: Vec<&str> = rest.iter().map(String::as_str).collect();
    match r.as_slice() {
        ["TAXI"] => vec!["TO".into()],
        ["TAXI", "TO"] => vec!["STAND".into(), "RUNWAY".into()],
        ["TAXI", "TO", "STAND"] => cap(w.airport.stands.iter().filter(|s| crate::traffic::stand_fits(s.code, &w.aircraft[i].type_code)).map(|s| s.name.to_ascii_uppercase()).collect()),
        ["TAXI", "TO", "RUNWAY"] => cap(w.airport.runways.iter().flat_map(|r| r.name.split('/')).map(str::to_ascii_uppercase).collect()),
        ["TAXI", "TO", "STAND" | "RUNWAY", _] => vec!["VIA".into()],
        ["TAXI", "TO", "STAND" | "RUNWAY", _, "VIA", used @ ..] => {
            cap(taxiway_names(w).into_iter().filter(|n| !used.contains(&n.as_str())).collect())
        }
        _ => Vec::new(),
    }
}

fn taxiway_names(w: &World) -> Vec<String> {
    w.airport.edges.iter().filter(|e| e.kind == crate::airport::EdgeKind::Taxiway && !e.name.is_empty()).map(|e| e.name.to_ascii_uppercase()).collect()
}

/// Resolve the words after `TAXI TO` into a destination node and via edges.
/// Every name is checked against the graph; a route that does not exist
/// says so by name.
fn resolve_taxi(w: &World, i: usize, ws: &[String]) -> Result<Command, String> {
    let (kind, name) = (ws.first().map(String::as_str).unwrap_or(""), ws.get(1).ok_or("say where: TAXI TO STAND <name> or RUNWAY <designator>")?);
    let via_names: &[String] = match ws.get(2).map(String::as_str) {
        None => &[],
        Some("VIA") if ws.len() > 3 => &ws[3..],
        _ => return Err("after the destination, only VIA <taxiway> ... may follow".into()),
    };
    let dests: Vec<u32> = match kind {
        "STAND" => vec![w.airport.stands.iter().find(|s| s.name.eq_ignore_ascii_case(name)).map(|s| s.node).ok_or_else(|| format!("no stand {name}"))?],
        "RUNWAY" => {
            let need = crate::aircraft::min_takeoff_run_m(w.aircraft[i].wake);
            let at: Vec<u32> = w
                .airport
                .runway_exits()
                .into_iter()
                .filter(|&n| w.graph.runway_run(&w.airport, n).is_some_and(|(run, d)| d.eq_ignore_ascii_case(name) && w.graph.run_len_dm(&run) / 10 >= need))
                .collect();
            if at.is_empty() {
                return Err(format!("no entry onto {name} with {need} m of runway"));
            }
            at
        }
        other => return Err(format!("TAXI TO {other}: say STAND or RUNWAY")),
    };
    // Shortest total route over every destination and, for each VIA name,
    // every edge of that taxiway: the click the map builder would have taken.
    let len = |r: &crate::route::Route| w.graph.run_len_dm(r);
    let mut best: Option<(u64, u32, Vec<u32>)> = None;
    for &to in &dests {
        let mut via: Vec<u32> = Vec::new();
        for n in via_names {
            let edges: Vec<u32> = (0..w.airport.edges.len() as u32).filter(|&e| w.airport.edges[e as usize].kind == crate::airport::EdgeKind::Taxiway && w.airport.edges[e as usize].name.eq_ignore_ascii_case(n)).collect();
            if edges.is_empty() {
                return Err(format!("no taxiway {n}"));
            }
            let pick = edges
                .into_iter()
                .filter_map(|e| {
                    let mut v = via.clone();
                    v.push(e);
                    w.plan(i, to, &v, &[]).map(|r| (len(&r), e))
                })
                .min();
            match pick {
                Some((_, e)) => via.push(e),
                None => {
                    via.clear();
                    via.push(u32::MAX);
                    break;
                }
            }
        }
        if via.contains(&u32::MAX) {
            continue;
        }
        if let Some(r) = w.plan(i, to, &via, &[]) {
            if best.as_ref().is_none_or(|b| len(&r) < b.0) {
                best = Some((len(&r), to, via));
            }
        }
    }
    best.map(|(_, to, via)| Command::Taxi { to, via }).ok_or_else(|| format!("no route to {kind} {name}{}", if via_names.is_empty() { String::new() } else { format!(" via {}", via_names.join(" ")) }))
}

/// A complete line to (aircraft index, verb key). The verb is not checked
/// against `World::verbs` here: the caller dispatches it through the same
/// path the key takes, so a refusal carries the sim's own reason.
pub fn parse(w: &World, line: &str) -> Result<(usize, char), String> {
    let ws = words(line);
    let Some(cs) = ws.first() else { return Err("type a callsign".into()) };
    let i = (0..w.aircraft.len())
        .find(|&i| w.aircraft[i].callsign.eq_ignore_ascii_case(cs))
        .ok_or_else(|| format!("no aircraft {cs}"))?;
    let said = ws[1..].join(" ");
    if said.is_empty() {
        return Err(format!("{cs}: say the instruction"));
    }
    PHRASES
        .iter()
        .find(|(_, p)| *p == said)
        .map(|&(k, _)| (i, k))
        .ok_or_else(|| format!("{cs}: \"{said}\" is not an instruction"))
}


/// C3 arguments stay in Rust; the page sends these through the sim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Key(char),
    HoldAt(u32),
    GiveWay(usize),
    Expedite,
    /// `TAXI TO ...`: the destination node and the via edges, resolved by
    /// the sim from names; the page plans and sends it.
    Taxi { to: u32, via: Vec<u32> },
}

fn legal_keys(w: &World, i: usize) -> Vec<char> {
    let mut keys = w.verbs(i);
    keys.extend(w.ground_verbs(i).iter().filter_map(|(k, _)| {
        if *k == "Enter" { Some('\r') } else { k.chars().next() }
    }));
    keys
}

/// Graph names are descriptive; the node suffix makes a point unambiguous.
pub fn hold_point_name(w: &World, node: u32) -> String {
    let names = point_names(w, node);
    if names.is_empty() { node.to_string() } else { format!("{} {node}", names.join("/")) }
}

fn point_names(w: &World, node: u32) -> Vec<String> {
    let mut names: Vec<String> = w.airport.holds.iter().filter(|h| h.node == node && !h.name.is_empty()).map(|h| h.name.to_ascii_uppercase()).collect();
    names.extend(w.airport.edges.iter().filter(|e| (e.a == node || e.b == node) && e.kind == crate::airport::EdgeKind::Taxiway && !e.name.is_empty()).map(|e| e.name.to_ascii_uppercase()));
    names.sort();
    names.dedup();
    names
}

fn argument_phrases(w: &World, i: usize) -> Vec<String> {
    let mut phrases = Vec::new();
    for n in w.aircraft[i].hold_points(&w.graph) {
        phrases.push(format!("HOLD AT {}", hold_point_name(w, n)));
        phrases.push(format!("HOLD AT {n}"));
    }
    for j in w.give_way_targets(i) {
        phrases.push(format!("GIVE WAY TO {}", w.aircraft[j].callsign.to_ascii_uppercase()));
    }
    if let crate::Phase::HoldShort(r) = &w.aircraft[i].phase {
        if w.verbs(i).contains(&'X') {
            phrases.extend(r.split('/').map(|d| format!("CROSS RUNWAY {}", d.to_ascii_uppercase())));
        }
    }
    let mut a = w.aircraft[i].scratch();
    if a.set_taxi_speed(crate::TaxiSpeed::Expedite).is_ok() {
        phrases.push("EXPEDITE TAXI".into());
    }
    phrases
}

pub fn parse_command(w: &World, line: &str) -> Result<(usize, Command), String> {
    let ws = words(line);
    let Some(cs) = ws.first() else { return Err("type a callsign".into()) };
    let i = w.aircraft.iter().position(|a| a.callsign.eq_ignore_ascii_case(cs)).ok_or_else(|| format!("no aircraft {cs}"))?;
    let said = ws[1..].join(" ");
    if let Some(point) = said.strip_prefix("HOLD AT ") {
        let candidates = w.aircraft[i].hold_points(&w.graph);
        if let Ok(n) = point.parse::<u32>() {
            return if candidates.contains(&n) { Ok((i, Command::HoldAt(n))) } else { Err(format!("{cs}: point {n} is not a hold point ahead on the route")) };
        }
        let matches: Vec<u32> = candidates.into_iter().filter(|&n| hold_point_name(w, n) == point || point_names(w, n).iter().any(|name| name == point)).collect();
        return match matches.as_slice() {
            [n] => Ok((i, Command::HoldAt(*n))),
            [] => Err(format!("{cs}: no hold point {point} ahead on the route")),
            _ => Err(format!("{cs}: {point} names several points; use the point number")),
        };
    }
    if let Some(other) = said.strip_prefix("GIVE WAY TO ") {
        let j = w.aircraft.iter().position(|a| a.callsign.eq_ignore_ascii_case(other)).ok_or_else(|| format!("no aircraft {other}"))?;
        return Ok((i, Command::GiveWay(j)));
    }
    if said == "TAXI TO" || said.starts_with("TAXI TO ") {
        let ws: Vec<String> = said.split_whitespace().skip(2).map(String::from).collect();
        return resolve_taxi(w, i, &ws).map(|c| (i, c));
    }
    if let Some(d) = said.strip_prefix("CROSS RUNWAY ") {
        return match &w.aircraft[i].phase {
            crate::Phase::HoldShort(r) if r.split('/').any(|x| x.eq_ignore_ascii_case(d)) => Ok((i, Command::Key('X'))),
            crate::Phase::HoldShort(r) => Err(format!("{cs}: holding short of {r}, not {d}")),
            _ => Err(format!("{cs}: not holding short of a runway; CROSS IF CLEAR crosses the next one")),
        };
    }
    if said == "EXPEDITE TAXI" {
        return Ok((i, Command::Expedite));
    }
    parse(w, line).map(|(i, k)| (i, Command::Key(k)))
}
