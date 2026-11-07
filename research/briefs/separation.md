# Brief: separation and runway rules — what counts as a conflict

Output folder: `research/out/separation/`. Topic name for `research/sources/` and
`research/work/`: `separation`.

## Why this exists

`PLAN-twr.md` names the one thing the game must never get wrong: **a conflict is
never missed, and an aircraft you did not endanger is never flagged.** Step B4
builds the conflict model: ground spacing, runway occupancy, departure-after-
departure, departure-after-arrival and wake categories. It is "done when" tests
build every conflict type on purpose and assert each is caught, and a 10-hour
headless run raises zero false alarms. Neither is possible until the rules are
written down with their numbers. Today none are; the sim has no conflict model.

This run produces those rules, each one precise enough to code and test, and the
test cases that prove the code follows them.

## Read first

- `PLAN-twr.md` § A2 (the compressed clock: rules run on game time, not real
  time), § Phase 0, and B4.
- `crates/sim/src/aircraft.rs` — the `Phase` enum is every state an aircraft can
  be in; each rule should say which phases it applies between.
- If `research/out/vidp/` or `research/out/aircraft/` already hold files, use
  them: VIDP's runway spacing and holding points, and per-type wake categories.
  Those runs may still be in progress; read what exists, do not wait.

## Sources

| Source | What to take from it |
|---|---|
| ICAO Doc 4444 (PANS-ATM), ch. 5 (wake turbulence separation), ch. 7 (aerodrome control: runway use, departing and arriving aircraft, reduced runway separation, conditional clearances), ch. 8 (radar minima on final, for arrival spacing) | The rules themselves. Cite paragraph numbers |
| ICAO Annex 11 | What aerodrome control is responsible for, and what it is not (ground spacing between taxiing aircraft is mainly the pilot's) |
| ICAO Doc 9643 (simultaneous operations on parallel or near-parallel runways) | Which combinations of VIDP's runways can run independently, by their spacing |
| ICAO Doc 9870 (Manual on the Prevention of Runway Incursions) | The definition of a runway incursion and its severity classes A–E |
| AIP India ENR 1 and GEN 1.7, AAI's Manual of Air Traffic Services (MATS) where public, DGCA Civil Aviation Requirements | India's own minima and its filed differences from ICAO. **India-specific values win** over ICAO defaults; say where they differ |
| VIDP AD 2.20 and AD 2.22 | Delhi's local runway procedures and low-visibility rules |
| EUROCONTROL RECAT-EU and ICAO's wake categories (L/M/H/J) | Wake matrices. Find whether India uses RECAT or the four-category scheme |
| EUROCONTROL A-SMGCS specification | What surface conflict alerting actually alerts on, with its thresholds. This is the closest real-world source for **ground** spacing, which ICAO does not set as a minimum |

## What to produce

### 1. `rules.csv` — every rule, one row each

`id, applies_between (e.g. DEP-DEP same runway, ARR-DEP, DEP-crossing, TAXI-TAXI),
phases (from the Phase enum, or the arrival phases the sim will need),
condition, minimum, unit (s, m, NM, state), measured_from, measured_to,
exceptions, source_paragraph, grade, india_difference, game_note`.

A row must be codeable. "Adequate separation" is not a minimum; the row says what
the controller checks, against what number, from which event to which event (e.g.
"from the first aircraft's lift-off to the second's start of take-off roll").

Cover at least:

- **One aircraft on the runway:** when a departure may start its roll after a
  preceding departure (crossed the end, or airborne and a distance ahead) and
  after a preceding arrival (vacated); when an arrival may cross the threshold.
- **Reduced runway separation minima** (Doc 4444 § 7.11), with every condition
  attached, and whether India permits them.
- **Wake turbulence:** time-based departure minima (same runway, parallel runways
  under the spacing threshold, crossing runways, intersection departures,
  opposite direction), and distance minima for arrivals on final. Both ICAO and
  RECAT if India uses it.
- **Runway crossings:** when an aircraft may be cleared to cross an active runway,
  relative to an arrival on final (time or distance) and to a departure.
- **Line up and wait:** how long an aircraft may hold on the runway, and the rules
  for conditional line-up ("behind the landing ...").
- **Landing clearance:** when it may be issued, and the conditions that require a
  go-around instruction.
- **Parallel runways at VIDP:** which pairs are independent, dependent or
  segregated for which operation, from their centreline spacing.
- **Low visibility:** what changes — ILS critical and sensitive areas, which
  holding points are used, increased spacing.
- **Ground (taxi-taxi):** what A-SMGCS alerts on, and what pilots are expected to
  keep. There is likely no ICAO number; say so, and propose an `authored` value
  with its reasoning.

### 2. `wake.csv`

The departure and arrival wake matrices: leader category × follower category →
minimum, unit, condition, source. One matrix per scheme India uses.

### 3. `incidents.md`

What the game should count as an incident and how severe it is. Map each rule
violation to Doc 9870's severity classes (A serious incident narrowly avoided …
E insufficient information) or to a lesser "loss of separation" class, and say
which the game should end a Surge shift on. `PLAN-twr.md` asks whether Surge
should end on the first runway incursion; give the evidence either way.

### 4. `test-cases.md`

For every row in `rules.csv`, at least two scenarios: one just inside the
minimum (must not flag), one just outside (must flag), plus each exception.
Columns: `rule_id, setup (aircraft, phases, positions, times on the game clock),
action, expected verdict`. Write them so a Rust test can be built from each row
without reading the source. This file is the core of B4's "done when".

### 5. `REPORT.md` and `sources.csv`, per the shared rules

In `REPORT.md`, a section **What a game simplification loses**: where the real
rule depends on something the game will not model (pilot visual responsibility,
controller judgement, surface radar), and the simplest stand-in that never
flags a safe situation as a conflict.

## Out of scope for this run

En-route and approach radar separation beyond the final approach segment.
Vehicles on the manoeuvring area (tugs, follow-me, fire service). Helicopter and
military-specific minima, except where Palam traffic shares VIDP's runways.
