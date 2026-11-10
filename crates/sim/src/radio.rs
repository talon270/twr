//! RADIO LINE · the typed command grammar, v1 (PLAN-ui-console A2)
//!
//! · PHRASES      the instruction words for each sim verb key
//! · next_tokens  the words that may legally come next on a partial line
//! · parse        a complete line to (aircraft index, verb key)
//!
//! Rust is the only source of legal actions. The page's composer asks
//! `next_tokens` what to offer and sends a finished line through `parse`;
//! it never keeps its own list of verbs. A verb is offered only when
//! `World::verbs` says it would succeed now, the same test the strip hints
//! use, so the composer can never offer a refusal.
//!
//! Limits of v1, stated where the claim is made: the words follow the root
//! forms in `research/out/phraseology/grammar.md` but carry no arguments.
//! `CROSS`, `LINE UP`, `TAKEOFF` and `LAND` name no runway, because the sim
//! verbs take none: each aircraft already has one assigned. `TAXI` and
//! `EXIT` open the same map pickers the T and E keys open; a typed route
//! (`TAXI 29L VIA A D`) and the grammar's minimum prefixes are v2.

use crate::World;

/// Verb key -> instruction words, in strip-hint order. Root forms from
/// grammar.md; `HOLD POSITION` because bare `HOLD` there also starts
/// `HOLD SHORT` and `HOLD AT`.
pub const PHRASES: &[(char, &str)] = &[
    ('P', "PUSHBACK"),
    ('T', "TAXI"),
    ('H', "HOLD POSITION"),
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
    (0..w.aircraft.len()).find(|&i| w.aircraft[i].callsign.eq_ignore_ascii_case(cs) && !w.verbs(i).is_empty())
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
            if !w.verbs(i).is_empty() {
                push(&w.aircraft[i].callsign.to_ascii_uppercase());
            }
        }
        return out;
    }
    let Some(i) = find(w, &ws[0]) else { return out };
    let rest = &ws[1..];
    let ok = w.verbs(i);
    for &(k, phrase) in PHRASES {
        if !ok.contains(&k) {
            continue;
        }
        let pw = words(phrase);
        if rest.len() < pw.len() && pw[..rest.len()] == *rest {
            push(&pw[rest.len()]);
        }
    }
    out
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
