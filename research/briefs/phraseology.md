# Brief: radiotelephony — the words a tower controller actually says

Output folder: `research/out/phraseology/`. Topic name for `research/sources/` and
`research/work/`: `phraseology`.

## Why this exists

The game's third input path is a typed command line, and the plan promises it
uses "plain phraseology words" (`PLAN-twr.md` § A1, § Input):
`IGO512 TAXI 29L VIA A D HOLD SHORT 28`. That example was written from memory.
A real Delhi Ground controller would say something different, and a player who
knows real RT will notice. The game also needs a radio log: the controller's
instruction and the pilot's readback, as text, for every action.

This run finds the real phrases for every action the game has, and turns them
into a grammar the typed line can parse. **Real wording first, then the
shortest typable form of it**, never a made-up keyword.

## Read first

- `PLAN-twr.md` § Input — the verbs: `P` push, `T` taxi, `H` hold, `X` cross,
  `L` line up, `K` takeoff, `C` cleared to land, `G` go around, `E` exit.
- `crates/sim/src/lib.rs` — `push`, `taxi`, `hold`, `cross`, `line_up`,
  `takeoff` are the commands that exist today.
- If `research/out/aircraft/operators.csv` exists, use its telephony
  designators for callsigns. That run may still be in progress.

## Sources

| Source | What to take from it |
|---|---|
| ICAO Doc 4444, ch. 12 (phraseologies) | The standard phrases for aerodrome and ground control. Cite paragraph numbers |
| ICAO Doc 9432 (Manual of Radiotelephony) | Worked examples, spelling and number pronunciation, callsign forms |
| ICAO Annex 10 Vol II, ch. 5 | Message structure, readback, callsign abbreviation rules |
| Doc 4444 § 4.5.7.5 | **Which instructions must be read back.** Runway entry, hold short, cross, take-off and landing clearances are among them; quote the list |
| AIP India GEN 3.4 / ENR, AAI MATS and DGCA's radiotelephony (FRTOL) exam material where public | Indian practice and any differences from ICAO |
| VIDP AD 2.18 | Delhi's ATC unit callsigns ("Delhi Ground", "Delhi Tower", "Delhi Delivery") and how many of each |
| UK CAA CAP 413 | `secondary` only: a very complete public phraseology manual. Use it to fill examples, and flag every place it differs from ICAO, because India follows ICAO |
| AAIB India accident and incident reports involving VIDP | Real Delhi transcripts, showing how phrases are actually used there. Quote short excerpts only |

## What to produce

### 1. `phrases.csv` — one row per instruction

`action (game verb or a new one), controller_phrase (template, with slots in
braces: {callsign}, {runway}, {taxiways}, {holding_point}, {stand},
{wind}, ...), pilot_readback (template), readback_required (Y/N + source),
when_used, variants, india_note, source_paragraph, grade`.

Cover every verb above, plus what the game will need soon: start-up approval,
pushback with facing direction, taxi with and without "hold short", give way,
follow, continue taxi, conditional line-up ("behind the landing ..., line up
and wait, behind"), take off with wind, cancel take-off clearance, stop
immediately, landing clearance with wind, go around, vacate via a named exit,
contact Ground on a frequency, and the pilot reports the game could simulate
("ready for departure", "runway vacated").

### 2. `numbers-and-names.md`

How the game should write the words it says, so the radio log reads right:
digits and their spoken forms (`tree`, `fife`, `niner`), runway designators
("two niner left"), taxiway letters in the phonetic alphabet, callsigns
(telephony + flight number, e.g. `IFLY 512`, and the abbreviated form once
contact is established), frequencies, wind and QNH.

### 3. `grammar.md` — the typed line

A token grammar (EBNF or similar) for the game's typed command line, derived
from `phrases.csv`:

- each keyword traced to the real word it abbreviates, with the real phrase
  beside it;
- the shortest unambiguous abbreviation for each keyword;
- what the parser should accept as optional (a player typing the full phrase
  must work too);
- the legal-next-word set after each token, which the autocomplete uses.

Rewrite the plan's example `IGO512 TAXI 29L VIA A D HOLD SHORT 28` in the real
form and in your grammar's typed form, and list every way the plan's version
differs from real usage.

### 4. `readback.md`

What a correct readback contains for each instruction that requires one, and
the common readback errors the literature records (hold-short omitted, wrong
runway, callsign confusion between similar flight numbers). The game may later
simulate a wrong readback as a hazard the player must catch; say which errors
are documented as frequent, with sources.

### 5. `REPORT.md` and `sources.csv`, per the shared rules

## Out of scope for this run

Voice synthesis and audio. Approach, departure and en-route phraseology beyond
the hand-off from Tower. Emergencies, except "stop immediately" and "go around",
which the game uses.
