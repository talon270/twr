# twr — plan

Written 2026-11-05, against an empty `twr/` folder.
Method: read the public page, `llms.txt` and `robots.txt` of twrctrl.com (v1.4.2,
the reference game), grepped its inlined game script for its scheduling constants,
and queried OpenStreetMap (Overpass API) for every runway, taxiway, stand and
holding point inside VIDP, VABB and VOBL. Its `/api/` was not touched; `robots.txt`
disallows it. The reference game was not played, so nothing here claims how it feels.
**Nothing below is implemented — this is the plan.**

---

## Phase 0 — shape

| Question | Answer |
|---|---|
| Who opens this and when? | You, on a laptop, for a 10–30 minute session when you want to be busy. Keyboard and mouse. Onboarding matters less than speed of input |
| The one thing it must never get wrong | **An aircraft does exactly what the drawn route showed, and a conflict is never missed.** A collision you did not cause, or a route that differs from its preview, makes every loss feel unfair. Both get guards and tests first (B2, B4) |
| Honest confidence of its central number | The score is never one blended number. Movements handled, delay you caused (minutes) and incidents are printed side by side, always |
| What happens when its data source dies | Nothing. Airport data is baked into the build; the game never fetches. A stale layout is fixed by re-running the extractor, and the title screen says which OSM date each airport was built from |

---

## Part A — findings

### A1 · Reusable code from twrctrl.com: none

The page footer reads "© Copyright of TERRN AB 2026" and there is no licence
anywhere in the HTML, `engine-fx.js` or `radio-fx.js`. No licence means all rights
reserved. It is also minified JavaScript and this build is Rust, so even a
permitted copy would be a rewrite.

What carries over is ideas, which are not anyone's property, and this game reworks
each one rather than copying it:

| Their idea | What twr does instead |
|---|---|
| Typed `CALLSIGN TARGET PATH` clearances with abbreviations (`PSHBK`, `BKTRK`, `-Q`, `.Q`) | Click-built routes first, single-key verbs second, a typed line third using plain phraseology words (`TAXI 29L VIA A D`, `HOLD SHORT D`) |
| One score from 80, ~100 = by the book | Three raw numbers, no blend (Phase 0, Q3) |
| Real-time pace with 1x/4x/10x | Compressed pace and a Surge mode built for overload (A2) |
| Keflavík, Arlanda, Haneda | Delhi, Mumbai, Bengaluru, from OSM (A3) |

The name, look, sounds and text are ours. Working title is `twr` until you pick one.

### A2 · DESIGN RISK (high): their pace is real time, which is why it drags

Their scheduler, from the shipped script:

```
SCHED_WINDOW_S=3600, SCHED_N_DEP=14, SCHED_N_ARR=13.5, SCHED_MIN_GAP_S=105
traffic multipliers: 0.5, 1, 1.5, 2, 3
```

That is 27.5 movements per real hour on Normal, never two inside 105 s, and 82 per
hour on Extreme. Their rate buttons speed up the whole sim, so 4x shortens the busy
moments by the same factor as the empty ones.

**Fix in twr:** separate the two.
- **Movement is compressed, decisions are not.** Taxi, pushback and runway roll run
  at `TIME_COMPRESSION = 3.0` by default, so a 4-minute real taxi takes 80 s. The
  rule checks (separation, runway occupancy) use the compressed clock, so the
  physics stays consistent.
- **Surge mode is the panic mode.** Traffic starts at a real peak-hour rate and
  climbs 10% every 2 minutes. The shift ends at the third incident. The result is
  how long you held, and how many movements you handled.
- **Shift mode** is a fixed 30-minute bank at a set traffic level, for practice.

**Traffic rates count per real hour; only movement is compressed.** If the schedule
ran on the 3× clock too, an IGI peak hour would arrive as 252 movements per real
hour. That is beyond anything playable, and the number would mean nothing.

IGI's real pace, sourced 2026-11-05:

| Figure | Value | Source |
|---|---|---|
| Daily average | ~1,275 movements (233,331 in H1 FY27 ÷ 183 days; the division is ours) | DIAL figures via Swarajya |
| 24-hour average | ~53 per hour (1,275 ÷ 24) | derived |
| Peak hour | 84 per hour, all runways combined | DIAL via Construction World, June 2024 |
| Target | 110 per hour, a plan, not reached | same |

**Surge starts at 53 per real hour and climbs from there.** It passes IGI's real peak
of 84 around the 10-minute mark. Past that point you're handling alone what the
real tower handles at its busiest. The real tower splits that work across several
tower and ground positions; that staffing claim is assumed, not sourced. Shift mode
levels: Light 27, Normal 53, Peak 84. BLR and BOM rates get sourced the same way in
B8.

### A3 · DESIGN RISK (high): OSM layout quality differs a lot between the three airports

Overpass counts from 2026-11-05. "Named" means the element carries a `ref` tag (the
taxiway letter or stand number):

| | IGI (VIDP) | BLR (VOBL) | BOM (VABB) |
|---|---|---|---|
| Runways | 4 — 09/27, 10/28, 11L/29R, 11R/29L | 2 — 09L/27R, 09R/27L | 2 — 09/27, 14/32 |
| Taxiway segments named | 245 of 591 (41%) | 56 of 162 (35%) | **0 of 316** |
| Distinct taxiway names | 110 | 39 | 0 |
| Stands named | 106 of 115 | 128 of 169 | 86 of 100 |
| Holding points named | 0 of 46 | 0 of 19 | 0 of 29 |

The geometry exists at all three. The names don't, and the whole clearance system
runs on taxiway names. Mumbai's lines are drawn but nothing is labelled.

**Fix:** an override file per airport (`data/<icao>.overrides.toml`) that names
segments and holding points by OSM id, filled by hand from the aerodrome charts in
India's eAIP (published by AAI). Build in order of data quality: **IGI, then BLR,
then BOM.** I have not opened the eAIP yet, so whether its charts are legible enough
to label from is unconfirmed. If they aren't, Mumbai gets letters we invent
ourselves, and the airport-select screen says so.

Not yet checked: whether OSM taxiway ways share nodes where they cross, which the
route graph needs. B1 measures connected components per airport before anything is
built on top of it.

### A4 · DESIGN RISK (medium): wasm doesn't load from `file://` by default

`wasm-bindgen`'s generated loader `fetch()`es the `.wasm` file, and Chrome blocks
that from `file://`, which breaks the house "open it and it runs" rule.

**Fix:** the build inlines the wasm as base64 into one `index.html` and starts it
with `initSync(bytes)`. No server, no fetch. B0 proves this before anything else
depends on it.

### A5 · Toolchain is missing

This machine has no `rustup`, `cargo`, `wasm-bindgen` or `wasm-opt`. Node is
present (used for Playwright only). B0 lists the install. The `pacman` lines need
`sudo`, so you run those.

---

## Part B — the build

### Stack

| Layer | Choice | Why |
|---|---|---|
| Sim core | `twr-sim` crate, pure Rust, **no web dependencies** | `cargo test` runs a whole shift headless, so routing, conflicts and pace are verified by running them |
| Browser shell | `twr-web` crate, `wasm-bindgen` + `web-sys` | Thin: input in, draw calls out |
| Map | Canvas 2D | A few hundred taxiway polylines and about 80 aircraft is well within Canvas 2D. WebGL would add a lot of code for nothing |
| Strips, radio line, HUD | DOM | Dense text, autocomplete and tabular numbers are what the DOM is good at. Immediate-mode game UI text is where the "janky" feel comes from |
| Airport extractor | `twr-osm` binary in the same Cargo workspace | Same language, runs once per airport, output baked in via `include_bytes!` |
| Build | `build.sh`: `cargo build --release`, `wasm-bindgen --target no-modules`, `wasm-opt -O3`, inline into `dist/index.html` | No bundler, no npm |
| Storage | `localStorage` key `twr.v1`, `SCHEMA_VERSION = 1` | Settings and personal bests only; a session is not saved mid-shift |

**The sim is deterministic.** Fixed 20 Hz tick on the compressed clock, seeded RNG,
seed shown on screen. Any shift replays exactly from its seed and its command log,
so a bug report is a seed and a list of commands, not "it did something weird".

### Look and interaction (design handbook)

- **Look: Nothing OS Classic, dark.** Black, white, one red. The red is reserved for
  conflicts and incidents, which suits the one-red-per-screen rule. Its sourcing is
  tagged `blog` (weak) in `designs.md`. Its listed weakness is dense content, so the
  strips get a 13px mono body instead of its 11px labels.
- **Motion: Precise product, overridden by the look's "click, not swoosh" (100 ms
  linear).** Nothing on screen animates for decoration. Aircraft move because the
  sim moves them.
- **UI tip cards used:** `command-menu` (the typed radio line), `hit-slop` (aircraft
  are a few pixels wide at full zoom-out), `tabular-numbers` (strip timers),
  `interruptible-animations` (pan and zoom retarget mid-flight).
- `~/.claude/CLAUDE.md § Design handbook`, which `memory.md` cites, does not exist on
  this machine. These picks come from `md/designs.md` and the handbook manifest.

### Input — the part that has to beat theirs

1. **Click the aircraft** or its strip. Its legal next actions light up as key hints
   on the strip.
2. **Single keys for verbs:** `P` push, `T` taxi, `H` hold, `X` cross, `L` line up,
   `K` takeoff, `C` cleared to land, `G` go around, `E` exit.
3. **Taxi by clicking:** after `T`, click the destination (holding point or stand).
   A shortest route is drawn at once. Click a taxiway to route through it, or
   right-click a taxiway to avoid it. `Enter` sends. **The preview is the route.**
   The sim receives the same edge list the screen drew, and nothing re-plans it.
4. **Typed line for keyboard players:** `IGO512 TAXI 29L VIA A D HOLD SHORT 28`.
   Autocomplete only offers words that are legal next. Every command it builds is
   the same as one the clicks would build.
5. Space pauses. `Tab` cycles to the next aircraft that is waiting on you, oldest
   first.

### Steps — each one runs and ships on its own

| # | Slice | Done when |
|---|---|---|
| **B0** | Toolchain and an empty shell: workspace, `build.sh`, wasm inlined, canvas draws a dot, DOM strip panel present, both `localStorage` read and write | `dist/index.html` opened from `file://` in Playwright: zero console errors, dot visible |
| **B1** | IGI extractor: OSM to route graph (nodes, edges, taxiway names, runways, stands, holding points), with an overrides file | `cargo test`: one connected component (or a list of the disconnected ones), every stand reachable from every runway exit, every named taxiway present |
| **B2** | Map and one aircraft: pan, zoom, click-to-select, taxi by clicking, preview equals route | Test: for 1,000 random stand-to-runway requests, the path the sim follows equals the drawn path, edge for edge |
| **B3** | Departure flow: push, taxi, hold short, cross, line up, takeoff | Playwright drives one departure from stand to airborne with real clicks |
| **B4** | Conflict model: ground spacing, runway occupancy, departure-after-departure and departure-after-arrival spacing, wake categories | Tests that build every conflict type on purpose and assert each one is caught, plus a 10-hour headless run with an auto-controller and zero false alarms on spaced traffic |
| **B5** | Arrival flow: aircraft on final, cleared to land, exit, taxi to stand, go-around | Playwright: one arrival from final to on-stand |
| **B6** | Traffic: per-airport operator and fleet table (IndiGo, Air India, Air India Express, Akasa, SpiceJet, plus foreign carriers), schedule generator, Shift and Surge modes, `TIME_COMPRESSION` | Headless run prints movements per real hour at each level. Starting rates trace to sourced AAI numbers or are labelled `authored` |
| **B7** | Score panel: movements, delay you caused, incidents. Surge ends at 3 incidents. Personal bests in `localStorage` | Seed old-shape storage and reload; bests survive |
| **B8** | **Moved to 1.1 (your call, 2026-11-07).** BLR, then BOM, using the B1 extractor and overrides | Same B1 tests pass per airport. BOM ships only once its taxiway names are filled, or ships labelled as invented |
| **B9** | Harden and README: wind picks the runway direction, title-screen OSM attribution and data dates, performance at 80 aircraft | Frame time measured in Playwright at 80 aircraft and quoted. README section 4 written |

Gate 2 applies after every slice: run it, quote the output.

---

## Out of scope

- **Radar vectoring, approach and en-route.** Arrivals appear on final. This is a
  tower game; adding approach doubles the sim for a different game.
- **Touch and phone layout.** Keyboard and mouse only for now. The click-route input
  would port to touch later; the single-key verbs would not.
- **Real METAR climatology.** Wind and visibility are random within authored ranges
  per airport. Sampling real METAR history is a data project of its own.
- **Sound.** Possibly later. Not in v1.
- **Service worker / PWA install.** A service worker cannot register from `file://`.
  The single HTML file is already offline.
- **Voice input**, multiplayer, accounts, any network call at runtime.
- **Saving a shift mid-session.** Shifts are short; only settings and bests persist.

## Licence notes

OSM data is ODbL. The title screen carries "© OpenStreetMap contributors". If this
folder is ever published, the derived airport files carry the same licence. eAIP
charts are used as a reference to type names into override files; no chart image
ships. **Not for real-world navigation** goes on the title screen.

## Open questions for you

- Game title — anything but a TWRCTRL lookalike.
- Surge's end condition: 3 incidents is a guess. Would you rather it end on the
  first runway incursion?

---

## B4 — conflict model

Written 2026-11-05, against B3 as built (twr-sim 20/20, twr-osm 6/6, `verify_b2.py`
29/29, `verify_b3.py` 22/22).
Method: re-read `aircraft.rs`, `lib.rs` and `route.rs`; measured IGI's geometry
from `data/vidp.twr` with a scratch script (numbers below); sourced the rules
(source strength stated per rule). **Doc 4444 itself was not read.** ICAO's site
sits behind a bot check that refused both a plain and a stealth fetch. The one
full copy found online is an unlicensed mirror, so it was left alone. The wake
table comes from a national AIS document that publishes Doc 4444 Amendment 9.
The rest comes from search snippets, labelled `snippet`.
**Built 2026-11-05 after approval; see "Built" at the end of this section.**

### Measured IGI geometry that shapes the rules

| Measure | Value | Consequence |
|---|---|---|
| Runways that intersect | none | No crossing-runway occupancy rule needed at IGI |
| 11L/29R to 11R/29L centrelines | 381 m, both on 103.7° | Under 760 m: wake treats them as one runway |
| 09/27 to 10/28 | 812 m at mid-length, 13.2° apart | Projected west-flow paths (27, 28) cross ~5 km out; east-flow paths (09, 10) diverge |
| Taxiway segment pairs closer than 44 m, not joined within 6 hops | 126, closest 6.1 m | Straight-line ground spacing would false-alarm (A6) |
| Closest two stands | 26.8 m | Parked aircraft can't be in the ground check |

### Findings

#### A6 · DESIGN RISK (high): straight-line ground spacing raises false alarms at IGI

OSM draws parallel taxilanes and dual lines 6–44 m apart in 126 places. Two
aircraft passing on parallel lines would read as a 20 m loss of spacing that
never happened.
**Fix:** measure ground spacing along the taxi network, only along the
mover's cleared route ahead (G1). This is the smallest fix that sees the
real cases (nose-to-tail, head-on, converging at a junction, pushing back
into a passer) and is blind to parallel lines by construction.

#### A7 · INCONSISTENCY (medium): `--red` text fails contrast in the dark theme

`#d71921` measures 4.05:1 on `#000` and 3.79:1 on the dark panel `#0b0b0c`,
under the 4.5:1 that text needs. It passes in light mode (5.18 and 4.72).
**Fix:** a separate dark text red, `#ff4d55` (6.44:1 on black, 6.04:1 on the
panel). Shapes on the canvas keep `#d71921`; 3:1 is enough for graphics.

#### A8 · MODEL GAP (medium): the world has no tick count of its own

`World::step_ticks` moves aircraft but counts nothing; only `Clock::advance`
counts, and tests bypass the clock. Wake timers need a time that tests and
the frame loop share.
**Fix:** `World::tick: u64`, bumped in `step_ticks`. `Clock` stays a frame-to-tick
converter.

### The rules

All times are **game time** on the 3× clock (§ A2): a 2:00 wake gap is 40 s on
your watch. Source strength: `read` means the page was read in full; `snippet`
means only a search-result excerpt was seen; `authored` means the number is mine.

| # | Rule | Number | Source |
|---|---|---|---|
| W0 | Wake category, from MTOW | L ≤ 7,000 kg; M over 7,000 and under 136,000; H 136,000 or more; J (SUPER) = A380-800 | Categories and J: GACA AIC 07/2020 (`read`). Mass limits: Wikipedia, citing Doc 4444 4.9.1 (`snippet`) |
| W1 | Departure wake, same runway, timed leader airborne to follower airborne | J→H 2:00 · J→M 3:00 · J→L 3:00 · H→M 2:00 · H→L 2:00 · M→L 2:00 · all others none | Table: Doc 4444 Amendment 9, as published in GACA AIC 07/2020 (`read`). The same values appear on an ICAO APAC slide (`snippet`). Timing from airborne: UK RA3277 (`snippet`); whether Doc 4444 names a reference point: not found |
| W2 | Intermediate departure: follower starts more than 150 m beyond the leader's start point | W1 + 1:00 | Doc 4444 5.8.3.4 via the Airservices AIP and a SESAR 2025 paper (`snippet`) |
| W3 | W1/W2 also apply across parallel runways under 760 m apart, and where the follower's projected path crosses the leader's | IGI pairs: 11L↔11R, 29L↔29R, 27↔28. Projection is a straight line, 10 km past the runway end | Conditions: SKYbrary and UK SERA.8012 (`snippet`). Projection and 10 km cap: `authored`, because the game has no SIDs. The altitude test ("same level or 300 m below") is ignored and always assumed to apply, the cautious reading |
| R1 | A takeoff roll may not start while another aircraft is on the runway ahead of it, or while any landing aircraft is on that runway | — | Doc 4444 7.9.2 per an ICAO APAC working paper (`snippet`); paragraph number varies by edition |
| R2 | A takeoff roll may not start until the preceding departure on that runway has passed the runway end | `Departed` = past the end | Same paragraph as R1 (`snippet`). "Or has started a turn" is not modelled: nothing turns before the end |
| R3 | Runway incursion: entering a runway (X or L) ahead of an aircraft on takeoff or landing roll on it | — | ICAO definition via SKYbrary (`snippet`): "the incorrect presence of an aircraft… on the protected area of a surface designated for the landing and take off of aircraft" |
| R4 | "On the runway" means on a runway edge, or within `HOLD_SHORT_M` = 90 m of a runway node along the aircraft's own route | 90 m | Annex 14 Table 3-2, code 4 precision runway (`snippet`, now matching B3's recollection). Using the hold-short distance is `authored`, so occupancy starts exactly where B3 stops aircraft |
| G1 | Ground spacing: centre-to-centre distance along the network, counted along the mover's cleared route ahead. A stopped aircraft counts when it is up to 20 m off that route. Converging routes use the sum of both distances to the node where they meet | warning under 300 m and closing; incident under 45 m | `authored`. Doc 4444 sets no taxi spacing; this is unconfirmed (the lookup did not reach it). 45 m is an A320's length (37.6 m, recalled, not checked) plus a margin. 20 m is half that length. 300 m gives 4.3 s of real time head-on at 2 × 10 m/s, or 8.5 s behind a stopped aircraft, before 45 m |
| G2 | Ground check skips: two stopped aircraft; any aircraft on stand; any aircraft on runway phases (lined up, roll, airborne, landing) | — | `authored`. Stands are 26.8 m apart and runways have R1–R3 |

**Not in B4, on purpose:** reduced runway separation minima (they need local
approval), the 1-minute and 2-minute departure track rules (departures leave
the game at the runway end, and radar spacing belongs to approach), and ICAO's
seven wake groups. AIC 07/2020 says groups are an optional alternative that
needs a local safety case. Whether AAI uses them at VIDP: not found, so
B4 uses the four categories. Displaced-threshold and opposite-direction wake
(Doc 4444 5.8.4, 5.8.5): not sourced, and B4 runs one flow at a time.

**No verb refuses on separation grounds.** The game lets you issue a wrong
clearance, as the real frequency does. B3's refusals (wrong phase, too little
runway) stay. The model flags the mistake; it does not prevent it.

### Conflict, incident, and what shows on screen

Red is used for these two states only. Nothing pulses or animates: a red state
appears on the tick it starts and goes on the tick it ends. Reduced motion
changes nothing, because there is no motion.

| State | Meaning | Map | Strip |
|---|---|---|---|
| **Conflict** (live, not counted) | G1 warning: under 300 m and closing | Red ring on both aircraft, red line joining them | 3 px red left bar, `CONFLICT IGO513 · 180 m ahead` (tabular numbers) |
| **Incident** (counted) | G1 under 45 m, or any of W1–W3, R1–R3 broken | Red filled square where it happened, left for 60 s game | Both strips tagged `INCIDENT`; one row in the incident log: game time, rule, callsigns, measured value against the minimum, e.g. `00:41:12 WAKE · IGO514 (M) 1:31 behind AIC101 (H), min 2:00` |
| Wake timer (neutral) | A follower lined up behind a W1–W3 leader | — | `wake 1:12` in the normal ink, and the K hint shows it. Neutral, because waiting is correct |
| Runway busy (neutral) | R1/R2 would break if K were pressed now | — | `rwy busy · IGO513` beside the K hint |

An incident counts **once per episode**: a ground pair must open past 300 m
before it can count again. Runway and wake incidents are tied to one takeoff or
one runway entry, so they count once naturally. The B7 score reads the incident
log; B4 shows its count and rows under the strips. Text red is `#ff4d55` in dark
and `#d71921` in light (A7).

**What counts as an incident**, in one line each: a takeoff that broke wake
spacing (W1–W3); a takeoff with the runway occupied (R1, R2); a runway
incursion (R3); a ground loss under 45 m (G1).

### Departure-after-arrival before B5 exists

B4 adds **`Phase::Landing(String)`**, the landing roll on a runway, and nothing
before or after it. A test places a **synthetic arrival**: an `Aircraft` on the
runway's run at the threshold, in `Landing`, at 70 m/s. It slows at
1.6 m/s² (`authored`) until it reaches taxi speed, then stops on the runway,
still occupying it, until the test removes it from `World::aircraft`. Removal
stands in for vacating. B5 adds the approach before `Landing` and a real exit
after it. B5 must not rename the variant: the scripts read Phase Debug text.

This tests R1's landing half (takeoff with an arrival still on the runway) and
R3 (crossing ahead of a landing roll) without the B5 flow.

### Proof (the two "done when" items)

**1 · One test per conflict type, each built on purpose.** Each rule gets a
breaking case and a case that only just passes. A model that flags everything
fails the second.

| Test | Breaks | Must not flag |
|---|---|---|
| ground nose-to-tail | taxi into a stopped aircraft: conflict at < 300 m, incident at < 45 m | stopped 46 m behind it |
| ground head-on | two routes facing on one taxiway | — |
| ground converging | two routes into one junction | one aircraft already 30 m past the junction on another taxiway |
| ground pushback | push into a passing taxi | — |
| ground parallel lines | — | two aircraft passing on the 6.1 m and 15.2 m parallel pairs |
| R1 occupied | K with a crosser on the runway ahead | crosser behind the departure's start |
| R1 arrival | K with a synthetic arrival on the runway | K one tick after it is removed |
| R2 | K while the leader is still `Airborne` on the runway | K on the tick after the leader is `Departed` |
| R3 | X ahead of a takeoff roll; X ahead of a landing roll | X behind a takeoff roll |
| W1 | M behind H, airborne 1 tick inside 2:00; each J pair at its value | the same pairs at exactly 2:00 / 3:00; M behind M at once |
| W2 | intermediate M behind H at 2:59.95 | full-length M behind H at 2:00 |
| W3 | 11R after 11L, H then M, inside 2:00; 28 after 27 | 10 after 09 (paths diverge); 28 after 29R (2.2 km apart, paths never cross) |
| episode | an incident counts once while the pair stays close | — |
| determinism | one game hour run twice from one seed gives the same incident log | — |

**2 · 10-hour headless run, zero false alarms.** 10 game hours = 720,000 ticks,
release build, one seed per run, 3 seeds. A test-only **auto-controller**
spaces traffic by its own method, so the model is not checking itself:

- **Ground:** it clears a taxi route only if no other non-parked aircraft is
  within 350 m in a straight line of any node on that route. This is cruder
  than G1 and stricter, so if G1 still flags something, G1 is wrong.
- **Runways:** one flow per game hour, alternating west and east, for every
  runway. It presses K **on the exact tick** each rule allows: after the wake
  minimum, after the leader is `Departed`, after the arrival is removed. Runway
  rules are tested at their boundary, where an off-by-one would show.
- **Crossings:** X only when no roll is under way on that runway.
- **Traffic:** a departure spawns on a free stand every 90 s game, if the
  stand has no aircraft within 350 m. Wake mix 70% M, 20% H, 5% J, 5% L
  (`authored`, chosen to hit every W1 pair). One synthetic arrival every 10
  minutes, placed only on an empty runway.

It asserts zero conflicts and zero incidents. It also asserts these floors
over the 10 hours, so a run that dodges the rules can't pass: ≥ 300
departures, ≥ 50 takeoffs on a wake boundary tick, ≥ 20 across the 11L/11R
pair, ≥ 30 arrivals with a K on the tick after removal, ≥ 50 crossings. The
floors are set now, before any run. If the first run misses one, the miss
gets reported, and the floor does not move to match the run.

### Code

| File | Change |
|---|---|
| `crates/sim/src/aircraft.rs` | `Wake { L, M, H, J }` field (default M; IGO512 is an A320neo); `Phase::Landing`; landing deceleration; constants at the top |
| `crates/sim/src/conflict.rs` (new) | W/R/G checks over a world snapshot; wake runway groups computed once from geometry; constants at the top with their sources |
| `crates/sim/src/lib.rs` | `World::tick`, `conflicts` (this tick), `incidents` (log); the check runs after every step; named scenarios, shared by tests and the page |
| `crates/web/src/lib.rs` | draw rings, lines, squares; hooks: read conflicts and incidents; one test-only write hook that loads a named scenario |
| `web/shell.html` | strip red bar and tags, wake timer, incident log, dark text red |
| `verify_b4.py` (scratchpad) | Loads each scenario through the hook and drives the real keys. Checks red appears and clears, counts log rows, checks both themes. `verify_b2.py` and `verify_b3.py` rerun unchanged |

### Out of scope for B4

- Landing clearance, arrival-after-departure, arrival wake, go-around: **B5**.
- Per-type wake and size: **B6** sets them from the fleet table. B4 uses M
  unless a test sets otherwise.
- Predicting conflicts further than "closing now": a lookahead adds new
  ways to raise false alarms. Revisit after playtest.
- ILS critical areas, vehicles, wingtip clearance by real span.
- The runway in use chosen by wind: **B9**. B4's controller picks the flow.

### Open questions for you

- Doc 4444 paragraph numbers and the W2 and W3 wording rest on snippets. If you
  have a licensed copy, paste 4.9.1, 5.8.3 and 7.9.2, and I will mark the rules
  `read`. Otherwise they ship labelled `snippet`.
- G1's 300 m warning and 45 m incident are guesses, to be tuned in playtest.

### Built (2026-11-05)

Approved and built the same day. `research/run.sh` found nothing to fold in:
the `vidp`, `aircraft` and `ui` runs started at 21:22 and stopped with
empty logs and no files, and `separation` was never run. Every number above
still has the source strength shown, and each one is a named constant at the
top of `crates/sim/src/conflict.rs`, so the separation findings can replace
them once they exist.

| Proof | Result |
|---|---|
| `tests/b4_rules.rs`: each rule broken on purpose, and its boundary | 15/15. Nose-to-tail: first conflict at 299.1 m, held at 45.6 m with no incident, then exactly one. 61 parallel taxiway pairs (closest 15.2 m) passed in both directions, none flagged. Every wake pair flagged one tick inside its minimum, clear on it |
| `tests/b4_long_run.rs`: 10 game hours × 3 seeds, own-method controller | 0 conflicts, 0 incidents. Departures 570 / 572 / 560; wake-boundary takeoffs 99 / 95 / 104 (45 / 42 / 43 across the parallels); takeoffs on the tick an arrival vacated 53 / 51 / 51; crossings 172 / 141 / 157. 69 s release |
| One hour replayed twice from one seed | identical aircraft, stats and incident log |
| `lib` (B0–B3 + 2 new) / `twr-osm` | 22/22 / 6/6 |
| Playwright `verify_b4.py` (scenarios through `load_scenario`, real clicks and keys, both themes) | 35/35: conflict ring and strip bar appear and clear on H; one incident per episode; wake timer in plain ink; dark log text rgb(255, 77, 85), light rgb(215, 25, 33); no dialogs, no console errors. `verify_b2.py` 29/29; `verify_b3.py` 19/19 and 22/22 on two seeds (16 checks + 3 per crossing) |

**BUG (medium), found by the long run, fixed: a crossing clearance carried
over to a later meeting with the same runway.** `cleared` held runway names,
so a route that crossed 11L/29R and met it again rolled through the second
meeting without stopping. Before the fix, 176 of the 1,000 routes in
`followed_path_equals_drawn_path_1000_times` ended with the aircraft parked
on a runway in `Holding` (811 lined up, 13 short of runway). After it, 0 do
(987 lined up, 13 short). `cleared` now holds the route index of each
crossing. Two meetings whose stops overlap (under 90 m apart) form one
crossing, since there is nowhere to wait between them. Regression test:
`crossing_clearance_does_not_carry_to_a_later_meeting_of_the_same_runway`.
It failed before the fix and passes after.

**What the first long runs got wrong, all in the test controller, none in
the monitor:**
- a 350 m reservation buffer gridlocked the apron (3 departures in 10 h);
  it is now 60 m, and each taxi route is reserved with its push;
- two K's issued in one pass on linked runways lifted off on the same tick.
  The monitor logged all 4 as wake incidents, correctly;
- the traffic was changed to reach the floors; the floors were not changed.
  Spawns are every 60 s, not 90. Arrivals touch down at the entry of a
  departure that is holding short. First-run boundary counts were 45 wake,
  17 parallel and 4 after-arrival.

**Deviation from the plan:** the incident square for a runway rule marks the
aircraft that was in the way, not the midpoint. The midpoint of a takeoff
roll and a crosser 1 km away is empty grass.


---

## B5 — arrival flow

Written 2026-11-06, against B4 as built (sim lib 22/22, `b4_rules` 15/15,
`b4_long_run` 2/2, `verify_b4.py` 35/35).
Method: re-read `aircraft.rs` (phases, `step`, `crossings`), `conflict.rs`
(`on_runway`, `rolling`, the ground net) and `lib.rs`; measured every runway
exit at IGI from `data/vidp.twr` with a scratch script (`exits.py`, numbers
below); one web search for the threshold rule. **The research runs still have
nothing to fold in.** All five logs (`vidp`, `aircraft`, `ui` at 21:22,
`separation` and `phraseology` at 21:44) hold one line: "You've hit your
5-hour limit". The runs died on the plan's usage cap, not on a script fault.
Re-run them after the reset; the arrival rules below are swappable constants.
**Approved 2026-11-06 and built the same day; see "Built" at the end of this section.**

### Measured IGI exits that shape the rules

"Usable" means at least 1,500 m from the threshold (70 to 10 m/s at
1.6 m/s², the B4 landing constants) and a first exit segment turning 95° or
less off the landing direction. The cut drops the reverse-angled exits
(155–177°), which belong to the opposite direction.

| Landing | Length | Exits | Usable | First usable |
|---|---|---|---|---|
| 09 | 2,811 m | 19 | 9 | D3 at 1,965 m |
| 27 | 2,811 m | 19 | 10 | D4 at 1,794 m |
| 10 | 3,808 m | 26 | 14 | unnamed at 1,766 m |
| 28 | 3,808 m | 26 | 14 | unnamed at 2,112 m |
| 11L | 4,404 m | 40 | 29 | unnamed at 1,842 m |
| 29R | 4,404 m | 40 | 28 | unnamed at 2,434 m |
| 11R | 4,421 m | 13 | 7 | Y1 at 2,292 m |
| 29L | 4,421 m | 13 | 6 | Z2 at 3,259 m |

"Usable" counts every forward exit edge with a vacate route, any distance
along, as `arrival::exits` finds them in the built sim; "first usable" is
the default, the first one at 1,500 m or more. Corrected 2026-11-06 from
the scratch script's first table, which swapped 10 and 28 and counted only
exits past 1,500 m.

Many exit edges have no OSM name. The strip then names the exit by distance
(`exit at 2,113 m`). The overrides file can fill the names in B8.

### Findings

#### A9 · DESIGN RISK (high): braking at the threshold makes 29L hold the runway for 3.5 minutes

B4's landing roll brakes from the threshold until it reaches taxi speed. On 29L
the first usable exit is Z2 at 3,259 m. Braking takes 37.5 s and 1,500 m.
Rolling the other 1,759 m at 10 m/s takes 176 s, so the runway is occupied for
**213 s**. Each departure behind an arrival would wait that long.
**Fix:** hold 70 m/s, then brake late so the aircraft reaches 10 m/s at its exit
node. The speed each tick is `min(70 m/s, √(v_exit² + 2·a·d_left))`, computed
with integer `u64::isqrt` so it stays deterministic. 29L to Z2 then takes 25 s +
37.5 s = **62.6 s**. The synthetic arrival, which has no exit, keeps B4's
behaviour, so no B4 test changes. This is the smallest fix: one speed formula
in one match arm.

#### A10 · MODEL GAP (medium): a landing aircraft counts as on the runway for its whole route

`on_runway` returns the runway for a `Landing` aircraft from `route.nodes[0]`,
wherever it is. B4 routes ended on the runway, so that was true. A B5 route
carries on down an exit taxiway, so the arrival would block K long after it had
left. **Fix:** for `Landing`, use the same span as a crossing. The runway is
occupied until the aircraft passes the last runway node plus the hold distance
(`Crossing::exit`). Beyond that the aircraft has vacated.

#### A11 · MODEL GAP (medium): the ground check never sees an arrival

`on_ground_net` leaves out `Landing`, so an arrival rolling off the exit at
10 m/s could hit a taxiing aircraft and nothing would flag it.
**Fix:** when the landing aircraft enters its first non-runway edge, its phase
changes to `Taxi` on the same route. The route and trail are untouched, and the
existing G1 code covers it. At the route's end it is `Holding`, waiting for T.

### Lifecycle and verbs

| Phase (Debug text) | Where | Strip state | Keys offered |
|---|---|---|---|
| `Final("29L")` (new) | Off the graph, on the extended centreline, 70 m/s | `Final 29L · 6.2 km · not cleared` / `cleared to land` | C, G, E |
| `Landing("29L")` (B4 name, kept) | Threshold to exit node; brakes late (A9) | `Landing 29L, 112 kt · exit Z2` | E (later exit only) |
| `Taxi` | Exit edges to the vacate point (A11) | `Taxi, 140 m to go` | H |
| `Holding` | Vacated, past the exit's hold distance | `Holding` | T |
| `Taxi` → `OnStand` | T + click a stand; crossings use X as in B3 | `On stand 41` | (P, for B6 turnarounds) |
| `GoAround("29L")` (new) | Climbs out along the runway like `Airborne`, then hidden | `Going around 29L · final again in 5:40` | — |

- **C, cleared to land:** only on `Final`. It is never refused for separation.
  If the runway is busy, `rwy busy · IGO512` shows beside the hint, as it does
  for K.
- **G, go around:** only on `Final`, cleared or not. A `Landing` aircraft is on
  the ground, so G is refused with "on the runway: too late to go around".
- **E then click an exit:** sets the exit. It is refused, with the reason on the
  strip, if the exit is behind the aircraft, if it turns more than 95°, or if
  there is too little room to slow down: "Y1 is 1,120 m ahead; needs 1,500 m to
  slow". **The default needs no key.** Each arrival gets the first usable exit,
  and the strip shows it.
- **T then click a stand:** as in B3. A route ending at a stand node ends
  `OnStand`, not `Holding`. A stand with an aircraft on it is refused:
  "stand 41 occupied by IGO601".
- **No landing clearance by the decision point → automatic go-around.** The
  decision point is 1,160 m before the threshold. That is the 200 ft CAT I
  decision height on a 3° path (200 ft ÷ tan 3°), labelled `authored` from
  those two nominal values. This is not an incident. The strip shows the
  notice "went around: no landing clearance" in plain ink. B7 counts it as
  delay you caused.
- **After a go-around, the aircraft comes back on final.** It returns at the
  spawn distance after `GA_CIRCUIT_S` = 360 game s, which is 2 real minutes
  (`authored`).

### Go-arounds, by cause

| Cause | Trigger | Counts against you? | Strip / log |
|---|---|---|---|
| **Pilot** (unstable approach, bird strike, …) | Seeded draw per arrival, `PILOT_GA_PCT` = 5% (`authored`; a real rate is a few per 1,000 approaches, which is too rare to be a game event). The pilot decides at a seeded point between 3 km and the threshold, even after C | **No.** It is a challenge: the aircraft returns in 360 s and must be fitted back into the flow | `GO AROUND · IGO713 (pilot: unstable approach)` in plain ink; the log row has the tag `challenge` |
| **Weather** | Not before B9; B9 adds wind and visibility. The same challenge path, with cause `weather` | **No** | as above, `weather: windshear` |
| **You, by G** | You press G | **No incident.** B7 counts the delay | `went around: instructed` |
| **You, no clearance** | No C by the 1,160 m decision point | **No incident.** B7 counts the delay, as caused by you | `went around: no landing clearance` |

Every go-around is drawn from the one seeded `Rng`, so a shift replays
exactly. The B7 score shows challenges handled beside movements, delay and
incidents, never blended into them.

### Rules added

| # | Rule | Value | Source |
|---|---|---|---|
| R5 | A landing aircraft must not cross the threshold until the runway is clear. That means a preceding departure has passed the runway end, and every preceding arrival and crosser has vacated. Lined-up aircraft count as on the runway | Checked on the tick of the `Final` → `Landing` change. An incident is logged as `Occupied`: `RUNWAY OCCUPIED · IGO713 crossed the 29L threshold with IGO512 on it`. The square marks the aircraft in the way | `snippet`: Doc 4444, reproduced in a Colombian AIS circular as 7.8.2. An unofficial 16th-edition copy puts the matching departure rule at 7.9.1. **Paragraph number unconfirmed**. The "started a turn" escape is not modelled; departures fly straight out |
| R2+ | K while a go-around is still over the runway | The existing `Departure` rule, with `GoAround` treated as `Airborne` | as R2 |
| — | Approach speed 70 m/s, spawn 8 km out, decision point 1,160 m, exit speed 10 m/s for every exit, circuit 360 s | constants at the top of `aircraft.rs` | `authored`. Every exit uses one speed because OSM draws rapid exits starting almost tangent to the runway (3–16°), so the first segment cannot tell a rapid exit from a right-angle one |

**Not modelled, on purpose:** spacing between arrivals on final is approach's
job, so the spawner sets it (B6). The plan also leaves out wake behind an
arrival, a go-around crossing a departure's climb-out, conditional clearances
("behind the landing A320, line up") and land-after clearances.

### What the page shows

- At shift start the page adds one arrival, **IGO713 (M), 8 km out on final for
  29L**. The runway is `authored`: IGI's real arrival runway per flow is
  unsourced until the `vidp` research run completes.
- The map draws a dim line from each aircraft on final to its threshold, so
  the screen shows where it will land. An aircraft beyond the view gets no
  edge marker in B5; its strip shows the distance. Playtest decides whether a
  marker is needed.
- Red is still used only for conflicts and incidents. Go-arounds, the busy
  note and the default exit are all in plain ink.

### Proof

**`tests/b5_arrivals.rs`**, each rule broken on purpose and checked at its boundary:

| Test | Breaks | Must not flag |
|---|---|---|
| R5 departure | Threshold crossed while the departure is still rolling or `Airborne` over the runway | Threshold crossed on the tick after it is `Departed` |
| R5 lined up / crosser / arrival | Threshold crossed with each one on the runway | Each one vacated a tick earlier |
| A10 | — | K on the tick the arrival passes its vacate point; the K is fine |
| A11 | Arrival exits into a stopped taxiing aircraft: G1 conflict, then incident | — |
| exits | Every usable exit at IGI has a vacate route that meets no other runway (count quoted) | — |
| A9 | 29L to Z2 runway occupancy is 62.6 s ± 1 tick | — |
| verbs | C, G and E are each refused with a reason in the wrong phase; E is refused on a too-close exit and on a reverse exit | — |
| auto go-around | No C: go-around at 1,160 m, back on final after 360 s | With C: lands |
| full flow | Final, C, land, default exit, T to a stand: `OnStand`, trail equals the drawn route after the exit | — |
| occupied stand | T to a stand with an aircraft on it is refused | — |

**`b4_long_run` with real arrivals:** the controller replaces the synthetic
arrivals. It spawns an arrival on final, gives C, takes the default exit, and
taxis the arrival to a free stand with its own reservation method. It withholds
C from 1 in 10 arrivals and presses G on another 1 in 10.

The floors are set now, before the run:
- the five B4 floors, unchanged;
- ≥ 30 arrivals from final to on-stand;
- ≥ 30 K's on the tick an arrival vacated;
- ≥ 10 go-arounds of each kind (pilot, G, no clearance), and none of them
  logged as an incident;
- 0 conflicts and 0 incidents, 3 seeds.

**`verify_b5.py` (Playwright, real keys and clicks, both themes):**
- IGO713 runs from final to on-stand: C, land on the default exit, T, click a stand, X at any crossing, `OnStand`.
- One scenario where the threshold is crossed with a departure lined up shows the red square and one log row.
- One run with no C goes around automatically and shows the plain notice.
- `verify_b2.py`, `verify_b3.py` and `verify_b4.py` rerun unchanged.

### Code

| File | Change |
|---|---|
| `crates/sim/src/aircraft.rs` | `Final`, `GoAround`; final distance field; late braking (A9); landing → taxi off the runway (A11); `clear_to_land`, `go_around`, `set_exit`; end on a stand → `OnStand`; constants at the top |
| `crates/sim/src/conflict.rs` | `on_runway` span for `Landing` (A10); R5 on the threshold tick; `GoAround` counted as `Airborne` for R2 |
| `crates/sim/src/lib.rs` | spawn on final; occupied-stand refusal in `taxi`; go-around respawn |
| `crates/sim/src/scenario.rs` | `usable_exits(desig)`, the vacate route; scenarios `arrival`, `threshold` |
| `crates/web/src/lib.rs`, `web/shell.html` | C/G/E keys, exit click mode, final line, strip states and notices |

### Out of scope for B5

- Traffic: arrival rates, arrival spacing on final, fleet and wake mix (**B6**).
- Runway choice by wind (**B9**); the page uses 29L for arrivals.
- Rapid exits at higher speed, a separate exit speed by angle: revisit with
  the `aircraft` research.
- Approach vectoring, holding stacks, missed-approach procedures beyond
  "comes back on final".

### Open questions for you

- ~~Should a go-around count against you?~~ **Answered 2026-11-06:** a go-around
  the pilot or the weather causes does not count against you; it counts as a
  challenge. See "Go-arounds, by cause" above.
- The page's arrival runway (29L) and the 8 km spawn are guesses until the
  `vidp` run completes.

### Built (2026-11-06)

| Proof | Result |
|---|---|
| `tests/b5_arrivals.rs` | 13/13. R5 flagged for a departure still over the runway, a lined-up aircraft, a crosser and an arrival still on its exit; clear one tick after each one is off. 29L to Z2: threshold to exit 62.60 s against the profile's 62.63 s, to vacate 75.45 s (A9; 213 s with B4's braking). Final to on-stand: 128 taxi edges, 1 crossing, trail equal to the landing route plus the drawn route. Pilot draw: 54 of 1,000. One hour with nobody at the controls: 7 go-arounds, replays identically |
| `arrival` unit test | Every landing direction has a default exit, and every usable exit's route leaves the runway and meets no other. The exit table above holds the built numbers |
| `b4_long_run` with real arrivals, 10 h × 3 seeds | **0 conflicts, 0 incidents.** Every floor met. Departures 542 / 530 / 543; wake boundary 79 / 68 / 59; parallel 39 / 29 / 21; crossings 143 / 159 / 169; arrivals final to on-stand 66 / 89 / 78; K on the vacate tick 34 / 46 / 42; go-arounds pilot 21 / 19 / 19, G 66 / 53 / 59, no clearance 11 / 19 / 15. 105 s release |
| `lib` / `b4_rules` / `twr-osm` | 24/24 / 15/15 / 6/6 |
| Playwright `verify_b5.py` (strip click, C, E, map clicks, T, X, both themes) | 40/40. Strip `Landing roll 29L, 136 kt · exit Z3` after E moved it from Z2; on stand at `Bay 86` after 1 crossing, taxi edge for edge (83 edges); no-clearance and pilot go-arounds in plain ink, the pilot one counted as a challenge; R5 row `RUNWAY OCCUPIED · IGO713 crossed the 29L threshold with IGO512 on 11R/29L`, red square at IGO512 in both themes. `verify_b4.py` 35/35, `verify_b3.py` 22/22, `verify_b2.py` 29/29 (see the flake note) |

**BUG (medium), found by `verify_b5.py`, fixed: a vacated arrival was offered L.**
B3's line-up from a holding point accepts any runway within 250 m, and an
arrival stops about 90 m past the runway it left. Before the fix the strip
offered `TL`; after it, `T` only, and L is refused with "an arrival taxis to
a stand (T)". `final_to_on_stand` now asserts it.

**What the first long runs got wrong, all in the test controller, none in
the monitor:**
- it protected a runway from arrivals on final but not from go-arounds. A K
  went out with a go-around 1.5 km short of the threshold, which then
  overflew the roll. The monitor logged 74 R2 incidents over 3 seeds,
  correctly. Go-arounds now protect the runway until they pass its far end,
  and a line-up waits for any arrival on final for that runway;
- arrivals on all four runways cut seed 3's parallel wake-boundary count to
  18, under the B4 floor of 20. Arrivals not aimed at a waiting departure now
  land on 27/28 or 09/10;
- that cut arrivals to about 90 per 10 h, and seed 3 then had 9
  no-clearance go-arounds against the floor of 10. The test now withholds C
  from 2 arrivals in 10 and forces a pilot go-around on 2 in 10 (plan: 1 in
  10 each). G stays 1 in 10. No floor moved.

**Deviations from the plan:**
- exit search lives in a new `arrival.rs`, not `scenario.rs`;
- clicking a strip selects its aircraft: at the fitted zoom an arrival 8 km
  out is off the canvas, and the strip is the only way to reach it;
- a go-around log under the incident log, with a challenge count beside it,
  in plain ink;
- incident and go-around lists refresh on their newest row, not their count:
  a loaded scenario could hold as many rows as the last world, all different.

---

## B6 — traffic

Written 2026-11-06, against B5 as built (`b5_arrivals` 13/13, `b4_long_run`
with real arrivals 3 seeds at 0 incidents, `verify_b5.py` 40/40).
Method: re-read `lib.rs` (`World::new`, `spawn_arrival`, `step_ticks`,
`taxi`), the go-around circuit in `aircraft.rs`, and the controller in
`tests/b4_long_run.rs`; counted IGI's stands in `data/vidp.twr` (115, no size
class); read wake category and MTOW per type from the FAA Aircraft
Characteristics Database that the `aircraft` research run downloaded before it
died (`research/work/aircraft/faa_types.json`, public domain); two web searches
for Delhi operator shares and the hourly shape. **The `aircraft` run produced
no `operators.csv` or `movements.md`.** Its log is one line, the usage cap. Every
fleet share below is therefore `authored` or `snippet`, and each one is a
named constant the research can replace.
**Approved 2026-11-06: build on the authored fleet table; arrivals leave their stand after 2 real minutes.**

### What is sourced, and how well

| Figure | Value | Grade | Source |
|---|---|---|---|
| Shift levels | Light 27, Normal 53, Peak 84 movements per real hour | as § A2 | DIAL via Swarajya and Construction World |
| Arrival : departure split | 50 : 50 | `snippet` | the record hour, 82 movements = 40 landings + 42 takeoffs (PSU Connect, undated) |
| Domestic operator shares | IndiGo 66.3%, Air India group 23.9% | `snippet`, **national**, not Delhi | DGCA June 2026 via aviationa2z, 2026-07-22 |
| Delhi seat shares | IndiGo 36.8%, Air India 17.95% | `snippet`, undated | OAG blog. Too old to use; it conflicts with the national share, so it is not used |
| Domestic : international at DEL | 80 : 20 | `authored` | none found. The 27% transfer share (DIAL, May 2026) is passengers, not movements |
| Wake per type | table below | **solid** | FAA ACD `icao_wtc` and MTOW |
| Which types each operator flies at DEL | table below | `authored` | general knowledge of the fleets; not checked per route |
| Hourly shape of the day | not used | — | none public. Shift and Surge use flat rates, so B6 does not need it |

### Findings

#### A12 · BUG (high, once traffic exists): a go-around rejoins final on top of the next arrival

After `GA_CIRCUIT_S` (360 s) the go-around comes back at `FINAL_SPAWN_M`,
whatever is there. With one arrival per page that never mattered. With a
stream of arrivals, a rejoin can land 1 km behind a scheduled arrival on
the same final. No airborne check exists, so the first anyone would see is an
R5 incident at the threshold, caused by the spawner, not by you.
**Fix:** the circuit end and the scheduler share one gate, `final_slot_free(desig, wake)`.
A circuit that ends while the slot is taken waits a tick and asks again.
The strip already counts down the circuit; past zero it reads
`Going around 29L · sequencing`. This is the smallest fix: one check on one
path, and the spawner needs the same check anyway.

#### A13 · DESIGN RISK (medium): traffic on the default page breaks four verify scripts

`verify_b2.py` to `verify_b5.py` assume the page opens with IGO512 on a stand
and IGO713 on final, and nothing else. Traffic that starts on load would add
aircraft under their clicks.
**Fix:** traffic is opt-in. `World::new(seed)` stays the B5 world, now
labelled **Practice** on the page. `World::shift(seed, mode)` is new. The page
starts a shift from a mode picker or from `?mode=normal&seed=123`. All four
scripts run unchanged.

#### A14 · DESIGN RISK (medium): stands carry no size

OSM gives IGI's 115 stands a name and a node, nothing more, so a 777 can be
sent to a stand built for an ATR. **Not fixed in B6.** Any aircraft may use any
stand. The fix needs stand sizes from the eAIP parking chart
(`research/sources/vidp/AMDT10-2026_VIDP-ADC.pdf` is on disk), which is B8
overrides work. The strip does not pretend otherwise.

### The schedule

**Rates count per real hour; the scheduler runs in ticks.** One real hour is
`3600 × TIME_COMPRESSION` game seconds, 216,000 ticks at 3×. A level of `r`
movements per real hour is two Poisson streams, arrivals and departures, each
at `r/2`. Gaps are drawn from the world's one `Rng`. **Why Poisson:** real
schedules bunch, and bunching is the game. A flat gap would make every shift
feel the same.

| Mode | Rate (per real hour) | Length | Ends |
|---|---|---|---|
| Practice | the B5 pair, no stream | open | never |
| Shift: Light / Normal / Peak | 27 / 53 / 84 | 30 real min | at 30 min. Arrivals already on final finish; nothing new spawns |
| Surge | 53, ×1.10 every 2 real min (85 at 10 min, 137 at 20 min) | open | **B7** ends it at 3 incidents. B6 only ramps |

**Arrivals spawn on final, spaced by approach.** An arrival that falls due
spawns at 8 km only if `final_slot_free` holds for its runway. If not, it
waits in an off-screen queue in order. Spacing is measured along final
between the new aircraft and the last one on that final:

| Leader → follower | Minimum | Grade |
|---|---|---|
| any → any | 3 NM (5.6 km), the radar minimum | `memory`: Doc 4444 8.7.3.2, not checked against the text |
| H → H / M / L | 4 / 5 / 6 NM | `memory`: Doc 4444 distance-based wake minima |
| J → H / M / L | 6 / 7 / 8 NM | `memory`; J is absent from IGI's mix (below), so this row is unused |

At 8 km out, a 5 NM gap means the leader must be 1.3 km past the
threshold. **So no M ever spawns behind an H until the H has landed.** That
is realistic and costs about one minute per heavy.

**Departures spawn on a free stand, ready to push.** "Free" means nobody is
on it and nobody's route ends there, the rule `taxi` already enforces. With
no free stand, the departure waits and the HUD says `stands full · 2 waiting`.
The strip names the assigned runway (`dep 29R`). You may taxi it anywhere;
nothing scores the runway in B6 or B7.

**Arrivals on stand shut down and leave.** `ON_STAND_S` = 120 real s after
`OnStand`, the aircraft is removed and counted as handled. There is no
turnaround into a departure: IGI's are 35–60 min, longer than a shift.
`authored`.

**Runways: west flow.** Arrivals 29L and 28, split 50/50 by a seeded draw.
Departures 29R and 27, likewise. `authored`. The real split per flow waits
for the `vidp` run, and B9's wind adds east flow.

### Operators, types, wake

Movement shares. The domestic 80% is split by the national DGCA shares,
rounded, with the remainder to SpiceJet and Alliance Air. The Air India
group's 23.9% is split 14 / 10 between mainline and Express (`authored`).

| Operator | ICAO | Share | Types at DEL (`authored`) and share within the operator |
|---|---|---|---|
| IndiGo | IGO | 52% | A20N 50, A21N 40, AT76 10 |
| Air India | AIC | 11% | A20N 45, A21N 25, B788 15, B77W 10, A359 5 |
| Air India Express | AXB | 8% | B38M 70, B738 30 |
| Akasa Air | AKJ | 4% | B38M 100 |
| SpiceJet | SEJ | 3% | B738 70, DH8D 30 |
| Alliance Air | LLR | 2% | AT76 100 |
| Emirates | UAE | 4% | B77W 100 |
| Qatar Airways | QTR | 3% | B788 50, A359 50 |
| flydubai | FDB | 2% | B38M 100 |
| Etihad | ETD | 2% | B789 100 |
| Lufthansa | DLH | 2% | A359 100 |
| British Airways | BAW | 2% | B788 50, B77W 50 |
| Singapore Airlines | SIA | 2% | A359 100 |
| Thai | THA | 2% | A333 100 |
| Blue Dart (cargo) | BDA | 1% | B752 100 |

**Wake, from the FAA ACD:** A20N, A21N, B738, B38M, AT76, DH8D, B752 are M.
B788, B789, B77W, A359 and A333 are H. That gives about **20% heavies**
(Air India's widebodies plus most foreign carriers), and the strip shows
the type. No A380 (J) is in the mix: none is confirmed for DEL. No L: no
light aircraft fly scheduled service at IGI.

**Callsigns:** ICAO prefix plus a seeded 3–4 digit number, unique in a
shift. Telephony (`IFLY`, `AIR INDIA`, `EXPRESS INDIA`) waits for the
`phraseology` run: several are not what you would guess, and a wrong one is
worse than none.

### What the page shows

- A mode picker (native `<select>`: Practice, Light, Normal, Peak, Surge) and
  a seed field beside the existing seed readout. Start restarts the world.
  The URL carries both, so a shift can be replayed from its link.
- HUD: mode, the current rate in movements per real hour, the time left
  (Shift) or elapsed (Surge), and the queues: `final queue 1 · stands full 0`.
- Strips: type and wake beside the callsign (`IGO2417 A21N M`), and the
  assigned runway. Strip order is order of appearance; `Tab` to the next
  aircraft waiting on you is a B7 or playtest question.

### Proof

**`tests/b6_traffic.rs`**, with no controller (the scheduler alone):

| Test | Asserts |
|---|---|
| rates | 10 real hours at each level, 3 seeds: spawned per real hour within ±10% of 27 / 53 / 84 (Poisson σ is about 4% at 530 events), printed |
| split | arrivals between 45% and 55% of spawns |
| mix | each operator's share within ±3 points of the table over 10 h at Peak; heavies 17–24% |
| spacing | at every spawn and every circuit end, the gap to the leader on that final meets the table. Broken on purpose with the gate off: the A12 rejoin is caught |
| stands | no departure ever spawns on a stand that is occupied or bound for |
| unique | no callsign repeats within a shift |
| surge | the rate at each 2-min step equals 53 × 1.10ᵏ, rounded, printed to 30 min |
| replay | same seed, same mode: identical spawn log |

**`b6_long_run.rs`:** the B4 controller, moved to `tests/common/` and split
so it can control traffic it did not spawn. `b4_long_run` keeps its own
spawning and must still pass unchanged. On B6 traffic:

- Peak, 2 real hours (6 game hours) × 3 seeds: **0 conflicts, 0 incidents**,
  and movements handled per real hour printed beside the rate offered.
- Surge × 3 seeds: printed only. The minute the controller's backlog first
  passes 10 aircraft. There is no floor; the controller is a test tool, not a
  claim about what a player can hold.

**`verify_b6.py` (Playwright, real clicks, both themes):** pick Normal and
Start; within 60 real s at least one new strip appears with type and wake;
`?mode=peak&seed=7` loaded twice gives the same first five callsigns;
Practice still shows exactly IGO512 and IGO713. `verify_b2.py` to
`verify_b5.py` rerun unchanged.

### Code

| File | Change |
|---|---|
| `crates/sim/src/traffic.rs` (new) | `Mode`, the Poisson streams, operator and type tables with their grade comments, `final_slot_free`, the final queue, stand pick, callsigns, the Surge ramp |
| `crates/sim/src/lib.rs` | `World::shift(seed, mode)`; the scheduler runs each tick; on-stand removal |
| `crates/sim/src/aircraft.rs` | `Aircraft` carries `type_code`; circuit end asks `final_slot_free` (A12); `parked` takes a wake |
| `crates/sim/tests/common/mod.rs` | the controller from `b4_long_run.rs`, spawning split from control |
| `crates/web/src/lib.rs`, `web/shell.html` | mode picker, seed field, URL params, HUD queues, type on strips |

### Out of scope for B6

- **The score and Surge's end** (B7). B6 counts nothing a player sees.
- **Stand sizes** (A14) and stand allocation by terminal: B8 overrides.
- **A day's shape** (banks, the night international wave). Modes use flat
  rates; a 24-hour mode needs the hourly profile the research has not found.
- **Wind, east flow, runway choice** (B9).
- **Telephony and the typed radio line**: the `phraseology` run.
- Turnarounds that turn an arrival into a departure.

### Open questions for you

- The fleet table is authored. Re-run `research/run.sh aircraft` before B6,
  or build on these numbers and swap them in later? They are constants
  either way; the only cost of waiting is the run itself.
- Arrivals on stand vanish after 2 real minutes. Keep them parked until a
  stand is needed instead, so the apron fills up as a shift goes on?

### Built (2026-11-07)

| Proof | Result |
|---|---|
| `tests/b6_traffic.rs` | 6/6. **Rates**, 40 real hours × 3 seeds: Light 26.7 / 27.4 / 26.8, Normal 52.4 / 53.4 / 54.0, Peak 84.6 / 85.2 / 83.5 per real hour; arrivals 47.6–51.2%. **Mix**, 100 h at Peak (8,286 movements): every operator within 0.6 points of the table (IndiGo 51.4% against 52%), heavies 20.4%. **Surge**: 53, 58.3, 64.1 … 85.4 at 10 min, 137.5 at 20, 221.4 at 30. **Spacing and stands**, Surge for 1 real hour with nobody at the controls, 3 seeds: 194 / 201 / 190 arrivals onto final, 138 / 132 / 139 of them rejoins after a go-around, every one at least its minimum behind its leader; the final queue peaked at 74 / 69 / 69 and the stand queue at 31 / 31 / 3; never two aircraft on one stand. **A12**: a go-around waited past its circuit and rejoined 3 NM (5,556 m, within one tick) behind the arrival that had taken its slot. **Replay**: same seed and mode, identical spawn log and aircraft |
| `tests/b6_long_run.rs`, Peak, 10 seeds (plan: 3) | **0 conflicts and 0 incidents on every seed, and every movement offered was handled**: each departure departed and each arrival landed, reached a stand and shut down. A 30-min shift offered 70–116 movements per real hour (Poisson, about 42 per shift). Surge, printed only: the controller's backlog first passed 10 at 13:49 / 12:36 / 24:22, and was 18 / 23 / 15 at 30 min, with 0 incidents |
| `b4_long_run`, controller moved to `tests/common/` | Identical to B5, number for number: departures 542 / 530 / 543, arrivals on stand 66 / 89 / 78, 0 incidents |
| Whole workspace | 70 passed, 0 failed (`lib` 26, `b4_rules` 15, `b5_arrivals` 13, `b6_traffic` 6, `b6_long_run` 2, `b4_long_run` 2, `twr-osm` 6) |
| Playwright `verify_b6.py` (select, typing, Start, links, strip click, C, T, map clicks, X, both themes, 1500 and 1920 px) | 34/34. Practice opens on IGO512 and IGO713 only; Start gives a Normal shift and `?mode=normal&seed=…` in the URL; `?mode=peak&seed=7`, a reload, and seed field + Start all give the same first five (AXB9606, AKJ8767, THA226, IGO332, IGO6439); IGO8791 landed, parked on Bay 87, shut down 121 real s later, and its strip left the bay; Surge reads 85/h at 10:00; no error banner, no console error, no dialog. `verify_b5.py` 40/40, `verify_b4.py` 35/35, `verify_b3.py` 22/22, `verify_b2.py` 29/29 on 8 of 8 runs after the script fix below |

**BUG (medium, pre-existing since B4 at least), found by `verify_b6.py`, fixed: resizing the window raised a red "Script error. ResizeObserver loop" banner.**
`main`'s grid row was auto-sized, so it grew to the canvas's own pixel height,
which `fit` sets from the row. The saved B4 page shows the banner on a
1500 → 1920 px resize; after `grid-template-rows: minmax(0, 1fr)`, it does not.

**verify_b2's theme check failed on 4 of 8 runs; the script was wrong, not the page.**
Its random taxi destination is sometimes a stand, and since B5 a route that ends
at a stand ends `OnStand`, where T needs a push first. The script now presses P
when that happens: 0 of 8 runs fail. This is the "flake" B5's row mentions.

**Deviations from the plan:**
- the schedule draws from its own `Rng` (seeded from the world's seed), not the
  world's one stream, so what happens in play cannot shift the traffic;
- arrivals in each stream are a Bernoulli trial per tick, which is Poisson to
  within a tick, with integer thresholds so the page and the tests agree;
- rate tests run 40 real hours, not 10: at 10 h, ±10% is only 1.6σ for Light;
- the stand test runs Surge, because a Shift stops spawning at 30 min, before
  Peak can fill 115 stands;
- arrivals that leave are marked `ShutDown` and stay in the world's list (as
  `Departed` does), so no index the page holds moves;
- the status bar lost its `Ticks` readout (a B0 debug value, read by no
  script) and the OSM attribution moved to the map's bottom-left corner: with
  the shift controls added, the bar overflowed 79 px at 1500 px and the
  attribution wrapped to 8 lines;
- in a shift, strips of departed and shut-down aircraft are hidden; Practice
  keeps them, as B2–B5 did.

---

## B9, early measurement: frame time at 80 aircraft (2026-11-07)

Measured ahead of B9 with `tools/perf_80.py`. Surge (seed 1) runs with nobody
at the controls until 81 aircraft are in play. That is 48–49 drawn (departures
waiting on stands, arrivals on final) and 32–33 out on the circuit. Then 600
frames are timed. Headless Chromium runs on this machine's real GPU, an AMD
Radeon Vega 3, an integrated, low-end part.

| Screen | Sim + canvas (`app.frame`) | Whole frame (rAF gap) | Frames over 20 ms |
|---|---|---|---|
| 1500×950, 1x | median 3.0 ms, p95 8.5 | median 16.7, p95 33.3 | 39 of 599 |
| 1920×1080, 1x | median 2.4 ms, p95 5.4 | median 16.7, p95 16.8 | 18 of 599 |
| 1500×950, 2x | median 2.2 ms, p95 4.5 | median 16.7, p95 16.7 | 0 of 599 |

**60 fps holds; the page's own work is 2–3 ms of the 16.7 ms budget.** The
whole JS frame, strips included, has a median of 4.5–6.8 ms. Dropped frames
vary between runs (0 to 39 of 599 at the same setting), and hiding the strip
panel does not remove them, so they are not the strips. Under SwiftShader
(no GPU), 2x density costs 50 ms per frame. That is the test rig, not the page.
**Not measured:** 80 aircraft all taxiing at once. Here most of them sit on
stands, and the ground conflict check is the part that grows with moving
aircraft. B9 measures that with a controller driving them.

---

## B7 — score

Written 2026-11-07, against B6 as built (70 tests, `verify_b6.py` 34/34).
Method: re-read the B6 world (`traffic.rs`, `World::step_ticks`), the
monitor's `wake_wait` and `runway_blocker`, the go-around log, and the page's
storage code (`twr.v1`, `SCHEMA_VERSION` 1, an empty `MIGRATIONS`). Nothing
in this slice waits on the research runs. **You answered Surge's end:
3 incidents, as § A2 planned.**
**Approved 2026-11-07: delay in game minutes; a departure on its stand gets
`STAND_GRACE_S` = 60 game s (20 real s) before its wait counts.**

### The three numbers, and what each one counts

The score is never blended (Phase 0, Q3). Each number is named for what it
counts, and its raw quantity prints beside it.

| Number | Counts | Shown as |
|---|---|---|
| **Movements** | takeoffs (the tick a departure is `Airborne`) plus landings (the tick an arrival crosses its threshold), beside what the shift offered | `Movements 38 · of 42 offered` |
| **Delay you caused** | game minutes that aircraft spent waiting on a clearance you could have given, by the rules below | `Delay 14.5 min · 6 aircraft` |
| **Incidents** | `monitor.incidents`, as B4 logs them | `Incidents 1` |
| Challenges (apart) | pilot and weather go-arounds you fitted back in | `Challenges 2` |

**"Of N offered" is there because seeds differ.** A 30-minute Peak shift
offered between 35 and 58 movements across the 10 seeds B6 ran. A best of
"40 movements" means nothing without the number that was possible.

**Delay counts in game minutes.** That is the time the aircraft and its
passengers experience. At 3× it is three times the real seconds you watched,
and the label says `min` with a `game time` tooltip, not real minutes.

### Delay: what counts against you

One tick of delay, per aircraft, for each tick it is in one of these states:

| State | Counts when | Does not count when |
|---|---|---|
| A departure `OnStand` | 60 game s after it fell due (`STAND_GRACE_S`, `authored`): a real departure has a slot time, it is not ready the instant it appears | inside the grace |
| A departure in the stand queue (B6's `stands full`) | each tick past the same grace. Stands are full because nobody pushed | inside the grace |
| `Holding` with no route | always: it wants T | — |
| `HoldPosition` | always: you stopped it with H | — |
| `HoldShort` | the runway it guards is clear | anyone is on that runway (`on_runway`); waiting for traffic is correct |
| `LinedUp` | nothing binds K | `wake_wait` or `runway_blocker` reports a reason: the wait is the rule |
| `GoAround` after G or no clearance | every tick until back on final | a pilot or weather go-around: a challenge, not delay |

**Not counted, on purpose:**
- taxi time, however long the route you drew;
- an arrival on final not yet cleared, since it is still flying in;
- the final queue, because approach spaces arrivals (B6), not you.

**The go-around count is low.** It stops at the rejoin and does not add the
8 km flown again, about 38 real seconds. State that in the tooltip; do not
inflate it with an estimate.

`Delay · 6 aircraft` counts the distinct aircraft that waited. 14 minutes on
one aircraft and 14 minutes spread across 14 aircraft are different shifts.

### How a shift ends

| Mode | Ends | Then |
|---|---|---|
| Light / Normal / Peak | at 30:00 real | the world stops: `World::over` is set and `step_ticks` does nothing. The result shows |
| Surge | on the tick of the third incident | the world stops; the result shows how long you held (`held 17:42`) |
| Practice | never | no score panel, no bests |

**The world stops instead of running on.** A score that keeps changing after
"Shift over" is a score you can't trust. Stopping inside the sim keeps a
replay of the same seed and commands ending on the same tick.

### What the page shows

- **The score panel** sits at the top of the strip column, above the strips:
  four tabular numbers, plain ink. Incidents switch to red text only when
  above 0, since red means incidents.
- **The result** is a native `<dialog>` with a `::backdrop` scrim (the
  handbook's figure-ground rule; the panel must reach 2.5:1 against the
  dimmed map, checked in both themes). It holds the three numbers and the
  challenges, the personal best for each number in this mode, and two buttons:
  **Again** (same seed) and **New seed**. Esc closes it and leaves the stopped
  map on view (handbook UI tip: back closes the top layer).
- **Look and motion:** Nothing OS Classic, unchanged. No count-up animation
  on the numbers ("click, not swoosh"). UI-tip cards used: `tabular-numbers`
  and the predictive-back card for the dialog.

### Personal bests

**Storage goes to `SCHEMA_VERSION` 2, with the migration in the same change.**
v1 `{ v, settings }` becomes v2 `{ v, settings, bests: {} }`.
`MIGRATIONS[1]` adds an empty `bests` and keeps `settings` untouched.

`bests[mode]` holds three separate records, each with its seed and date, and
none of them blended:

| Record | Best is | Kept with |
|---|---|---|
| `movements` | the most | offered, seed, date |
| `delay_per_movement` | the least, counted only for a shift with 10 or more movements | the raw delay and movements, seed, date |
| `incidents` | the fewest, ties going to more movements | movements, seed, date |
| Surge only: `held` | the longest time before the third incident | movements, seed, date |

A shift that ends with the tab closed or reloaded is not recorded: there is
no result to trust.

### Proof

**`tests/b7_score.rs`**: each delay rule is broken on purpose and checked at
its boundary.

| Test | Asserts |
|---|---|
| on stand | a departure left 70 game s on its stand: exactly 200 ticks of delay (10 s past the grace); one pushed inside the grace: 0 |
| hold short | waiting while a departure rolls on that runway: 0; the tick it is clear: counting starts |
| lined up | waiting out a 2:00 wake: 0; one tick past the wake: 1 |
| go-arounds | a G: delay until back on final; a pilot go-around: 0 delay, 1 challenge |
| stand queue | 3 departures queued for 100 ticks: 300 |
| movements | the long-run controller's departures + landings equal the score's movements, to the unit, on 3 seeds |
| Surge end | three incidents built on purpose: `over` is set on the third one's tick, and 1,000 more ticks change nothing |
| Shift end | at 30:00, `over` is set; no movement after it counts |
| replay | same seed, same command log, same score |

**`b6_long_run` prints the score** per Peak seed: the controller's delay is
a number a player can compare with their own.

**`tools/verify_b7.py` (Playwright, real clicks, both themes):**
- a Normal shift, fast-forwarded to 30:00, shows the dialog with
  the three numbers, and they match the hooks;
- the dialog's scrim measures 2.5:1 or better in both themes;
- Again restarts the same seed, and Esc closes the dialog without restarting;
- bests are saved; a reload keeps them;
- a seeded v1 `twr.v1` (theme light) reloads as v2 with the theme still light
  and `bests` empty;
- a stored v3 is refused with the banner, and the saved copy is left as is;
- Practice shows no score panel;
- `verify_b2.py` to `verify_b6.py` rerun.

### Code

| File | Change |
|---|---|
| `crates/sim/src/score.rs` (new) | `Score { movements, offered, delay_ticks, delayed: BTreeSet<callsign>, challenges }`, the per-tick delay rules, `over` |
| `crates/sim/src/lib.rs` | score tick after the monitor; `over` stops `step_ticks`; Shift and Surge end rules |
| `crates/web/src/lib.rs` | score hooks: movements, offered, delay minutes, delayed count, incidents, challenges, `over`, held |
| `web/shell.html` | score panel, result `<dialog>`, `SCHEMA_VERSION` 2 and `MIGRATIONS[1]`, bests |

### Out of scope for B7

- **A blended score, grades or stars.** Never (Phase 0).
- **Taxi efficiency**, meaning a longer route than needed. Hard to call fair
  when the shortest route crosses a busy runway.
- **Arrival delay on final and in the final queue.** That is approach's
  work, not yours.
- **Leaderboards, sharing, anything online.**
- **Saving a shift mid-way** (Out of scope, top of plan).

### Open questions for you

- Delay in **game** minutes, as planned, or real minutes? Game minutes are
  what aircraft live through; real minutes are what you watched. The number
  differs by 3×.
- Is "a departure on its stand counts from the tick it appears" too harsh?
  A real departure has a slot time, so it is not ready the instant it
  appears. A grace period (for example 60 game s) is a one-line change.

### Built (2026-11-07)

| Proof | Result |
|---|---|
| `tests/b7_score.rs` | 8/8. On stand: 70 game s counts exactly 200 ticks; pushed inside the grace, 0. Hold short behind a roll on 29R: 0. Lined up: 977 ticks of wake waited, 0 delay. A G cost 9,983 ticks (8.3 game min); a pilot go-around cost 0 and counted 1 challenge. Stand queue, Surge end on the third incident, Shift end at 30:00, and replay all hold. Normal seed 3 under the controller: 22 of 22 offered, 16.3 game min over 2 aircraft |
| `b6_long_run`, Peak, 10 seeds | movements equal departed + landed on every seed (the plan's "movements" test lives here, on 10 seeds, not 3). The controller scores 35–58 movements and 51.4–192.5 game min of delay over 9–24 aircraft. Still 0 incidents on every seed |
| Whole workspace | 72 passed, 0 failed (`lib` 26, `b4_rules` 15, `b5_arrivals` 13, `b7_score` 8, `b6_traffic` 6, `b6_long_run` 2, `b4_long_run` 2, `twr-osm` 6). `b4_long_run` is unchanged, number for number |
| Playwright `verify_b7.py` | 22/22 on 3 of 3 runs. Practice has no panel; the panel matches the hooks mid-shift; at 30:00 the dialog opens with four rows that match the sim; scrim 2.92:1 dark, 3.36:1 light (needs 2.5); Esc closes it and the shift stays over; nothing moves after the end; Again restarts seed 3; New seed works; bests survive a reload; a v1 store (light) loads as v2 with the theme kept and `bests` empty; a v3 store is refused with the banner and left untouched; no console error |
| `verify_b2.py` to `verify_b6.py` | 29/29, 22/22, 35/35, 40/40, 34/34, after the `verify_b3` fix below |

**Deviations from the plan:**
- `Delay · N aircraft` counts aircraft whose waits add up past
  `DELAYED_OVER_S` = 60 game s (`authored`), not every aircraft that waited
  one tick. Without it, the controller "delayed" 22 of 22, each for the one
  tick between vacating and T;
- the fewest-incidents best needs 10 movements, like delay per movement: an
  idle shift (0 movements, 0 incidents) otherwise held the record;
- an ended shift stops its clock too (`World::frame` returns 0 steps), or
  the game clock on screen ran on over a frozen map;
- best dates are the local date (`toLocaleDateString("en-CA")`), not UTC:
  a shift before 05:30 IST was saved as yesterday (the run at about 01:00
  on 2026-11-07 saved `2026-11-06`);
- the panel reads `Movements 38 of 42 offered`, without the `·`.

**`verify_b3`'s airborne check failed once (640 ticks against 700–800); the
script was wrong, not the page.** The page's own frames stepped the sim
between fast-forwards, uncounted, and the cargo suite was loading the CPU.
The timed roll now runs paused (Space): 760 ticks on 3 of 3 runs, which is
the expected 750 rounded up to the script's 20-tick steps. `verify_b7` got
the same pause before its mid-shift panel read.

---

## B9 — harden and release 1.0

Written 2026-11-07, against B7 as built (72 tests, `verify_b2`–`verify_b7`
all passing). Method: re-read `traffic.rs` (`ARR_RWYS`, `DEP_RWYS`),
`conflict.rs::wake_links` and its IGI test, `arrival.rs` (go-around causes,
`PILOT_GA_PCT`), the page's attribution line, `tools/perf_80.py`, and every
`research/out/` folder. **B8 (BLR, BOM) is now 1.1, by your call.**
**Approved 2026-11-07: as written; the title stays `twr`; the finished
`aircraft` research goes into 1.0 (§ "Aircraft research, folded in").**

### What research exists, and what B9 does without it

| Run | State on 2026-11-07 | Used in B9 |
|---|---|---|
| `aircraft` | finished: `REPORT.md`, `types.csv`, `operators.csv`, geometry for 3 types | no, see open question 2 |
| `separation` | `rules.csv` (29 rules, LVP included); every log ends at the usage limit, so no report | no |
| `vidp` | 2 runs, both empty logs. **Nothing on how IGI picks its flow** | no: wind is authored |
| `ui`, `phraseology` | empty logs | no |

### Findings

#### F1 · MODEL GAP (high): the game only ever runs west flow

`traffic.rs` fixes `ARR_RWYS = ["29L", "28"]` and `DEP_RWYS = ["29R", "27"]`.
Every shift at IGI uses the same four runway ends, whatever the weather.

**Fix:** east flow is the reciprocal set: land 11R and 10, depart 11L
and 09. The conflict model already covers it, because `wake_links` is
built from geometry, not from names: its IGI test lists `11L/11R Parallel`
and finds no east-flow crossing (09 departures and 10 arrivals do not meet;
only 27/28 do). So east flow needs a second pair of constants and a choice
between them, not new rules. The smallest correct fix: a `Flow` enum with
its two runway sets, picked once per shift.

#### F2 · DESIGN RISK (medium): "title screen" in the plan, but there is no title screen

The Licence notes and Phase 0 Q4 ask for OSM attribution and the data date
"on the title screen". Since B6 the map's bottom-left corner reads
`© OpenStreetMap contributors, data of 2026-11-05 · not for real-world
navigation`, on screen at all times. ODbL needs the credit visible, and
this does that better than a screen you pass once.

**Fix:** none. B9 records the corner as the answer and drops the title
screen. Adding one would be a screen with nothing else on it.

### Wind

**One wind per shift, drawn from the seed, picks the flow.** It draws from
its own `Rng` (seeded from the world seed, as the schedule's is), so the same
seed gives the same wind, and nothing played can change it.

| Item | Rule | Grade |
|---|---|---|
| Direction | 75%: 270°–340°; 25%: 060°–130° | `authored`. From memory, Delhi winds are mostly westerly, and easterly in the monsoon. The `vidp` run would replace this |
| Speed | 2–16 kt, uniform | `authored` |
| Gusts | 20% of shifts gust 8–15 kt above the mean | `authored` |
| Flow | west if the headwind component on 29L is ≥ 0, else east. A wind under 5 kt gives west flow (the preferred direction) | `authored`. Runway selection allows about 5 kt of tailwind (PANS-ATM, from memory, unconfirmed) |
| Crosswind | not checked: IGI's 3 runway pairs lie within about 20° of each other, so no wind favours one pair over another | measured from OSM headings in the build |
| Windshear | gusty shifts only: an arrival inside `PILOT_GA_MAX_M` (3,000 m) gets a 3% go-around, cause `weather: windshear`. It counts as a challenge, not delay (B7) | `authored` |

**Wind does not change any speed.** Ground roll, final approach speed and
taxi stay as B3–B5 set them. A headwind shortening the takeoff roll is
real, but adding it moves every timing the tests pin down, for an effect
you would not see.

**Practice stays west flow and calm.** Its scenarios name 29L and 29R, and
`verify_b2`–`verify_b5` rely on them.

**The runway does not change mid-shift.** A real change of flow means
re-sequencing every aircraft in the system. That belongs with a wind that
changes, which is not in 1.0.

**On the page:** the status bar shows `Wind 290/12` (`G24` when gusting),
then `West flow · land 29L 28 · depart 29R 27`, in tabular numbers. The
result dialog repeats the wind, so a best set in east flow says so.

### Performance: the unmeasured case

The early measurement (§ "B9, early measurement") timed 80 aircraft with
most of them parked. The ground conflict check grows with moving aircraft.
**`tools/perf_80.py` gains a second state:** Surge to 80, then real clicks
push and taxi departures toward the runway until at least 30 are moving
on the ground. Then it times 600 frames at 1500 and 1920 px, as before.
**Done when:** the median `app.frame` time is quoted for both states. If it
passes 8 ms (half the 16.7 ms budget), the conflict check gets a fix
before 1.0.

### Hardening pass

The standing checklist (`skills.md` Phase 4), run as `tools/verify_b9.py`:

| Check | How |
|---|---|
| Zero console errors, fresh profile and seeded v1 and v2 profiles | load each, play 60 real s of Normal with real keys |
| Both themes, at 1500 and 1920 px | screenshot both; measure empty background to the right of the strip column (the Study Tracker bug) |
| `prefers-reduced-motion: reduce` | pan and zoom move with no spring; the dialog has no transition |
| No `alert()` / `confirm()` | grep `web/shell.html` and the built page: 0 calls |
| East flow end to end | a seed that draws east: an arrival lands on 11R or 10 and a departure takes off from 11L or 09, by real clicks |
| Wind on screen | the status bar and the dialog match the hooks |

### README (`twr/README.md`)

In the `skills.md` Phase 6 order:

1. Title and one paragraph: what it is, IGI from OSM, offline, one file.
2. How to run it: open `dist/index.html`; rebuild with `./build.sh`; tests.
3. Keys and clicks, one line each (P, T, H, X, L, K, C, G, Space, Esc).
4. **The things most tower games get wrong**, each a bolded claim with
   numbers: compressed movement but real-hour rates (A2: 252 vs 84 per
   hour); three numbers, never a blended score; delay counts only what you
   could have cleared; a conflict is never missed (10 h × 3 seeds, 0
   incidents); the route you drew is the route flown, edge for edge.
5. **Assumed vs solid**: every `authored` constant (90 m hold, 2 m/s²,
   wind, windshear, grace, fleet table) against what is sourced (wake
   table from GACA AIC 07/2020, IGI rates from DIAL, OSM geometry), with
   the research run that would replace each one.
6. Data and licence: ODbL credit, OSM date, not for navigation.

### Proof

**`tests/b9_wind.rs`:**

| Test | Asserts |
|---|---|
| draw | 10,000 seeds: east flow 18–21% (25% easterly × the 11 in 14 of speeds at 5 kt or more = 19.6%); speed inside 2–16 kt; gusts on 18–22% |
| flow rule | 290/12 is west; 100/12 is east; 100/04 is west (calm); the headwind boundary at exactly 0 is west |
| replay | same seed, same wind, same flow |
| east long run | `b6_long_run` Peak, 10 seeds, forced east flow: every movement handled, 0 incidents |
| windshear | 1,000 gusty arrivals: 2–4% windshear go-arounds, all logged as challenges; 0 on calm shifts |

**`b4_long_run` and `b6_long_run` (west) must print the same numbers as B7.**
They run west flow with no gusts. A changed number there is a bug.

### Code

| File | Change |
|---|---|
| `crates/sim/src/weather.rs` (new) | `Wind { dir, kt, gust }`, its draw, `Flow` and the flow rule, windshear |
| `crates/sim/src/traffic.rs` | `ARR_RWYS` / `DEP_RWYS` become per-`Flow` |
| `crates/sim/src/arrival.rs` | `GoAround::Weather` gets its first cause |
| `crates/web/src/lib.rs` | hooks: wind, flow, runways in use |
| `web/shell.html` | status-bar wind and flow; wind in the result dialog |
| `tools/perf_80.py`, `tools/verify_b9.py` | as above |
| `README.md` (new) | as above |

### Aircraft research, folded in

Source: `research/out/aircraft/` (`REPORT.md`, `movements.md`,
`sim-constants.md`, `types.csv`, `operators.csv`), run 2026-11-07.

| # | Finding | Change in 1.0 |
|---|---|---|
| R1 | Heavies are ~11% of passenger movements, not 20%: GMR's split is 77:23, not 80:20 | `OPERATORS` and `TYPES` rebuilt from `estimate_mix.js`'s output. Rows under 0.5% are dropped and the rest renormalised. The B6 mix test keeps its 0.6-point tolerance against the new table |
| R2 | The A380 (J) flies DEL daily, Emirates 1 of 4 rotations | `A388` (J) joins Emirates' fleet. The J rows of the wake tables come into use |
| R3 | `MIN_TAKEOFF_RUN_M` = 1,800 m is short: an A20N needs ~2,030 m at Delhi's elevation, a 777-300ER ~3,100 m (charts read by eye, ±100–150 m, at MTOW) | a floor per class: narrowbody and turboprop 2,030 m, widebody 3,100 m. A heavy is assigned a departure runway long enough from its entry, measured on the graph. 27 (about 2,810 m in OSM) is then too short for a heavy |
| R4 | Telephony for 89 of 90 operators | the strip tooltip shows it (`IFLY 512`). No radio text changes |

**MTOW is the conservative case.** A heavy at lower weight does use a
shorter runway; a weight model is 1.1. The README's assumed-vs-solid table
says so.

**Not folded in, and why:** per-type rotate and climb speeds (VR is a gap
for every type), per-type Vref (it moves every B5 timing for a 13 kt
spread), wingspans on the map and stand fit (needs the `vidp` run's stand
half), and taxi turn speeds (no primary source). All 1.1.

**Numbers that will move:** the B6 mix figures and both long runs. The bar
stays: 0 incidents and every movement handled.

### Out of scope for B9 (1.0)

- **BLR and BOM** (B8, now 1.1).
- **Visibility and low-visibility procedures.** `rules.csv` holds V1–V4,
  but LVP changes taxi routes and landing clearance, which makes it a slice
  of its own. 1.1.
- **Wind that changes mid-shift, and a runway change.** See Wind.
- **Wind effects on speed.** See Wind.
- **A title screen** (F2).
- **Commits and a release tag.** Only when you ask.

### Open questions for you

1. **Title.** The README needs a name. Keep `twr`, or pick one now?
2. **The finished `aircraft` research** (fleet mix, types, geometry for 3
   types): fold it into 1.0, replacing the authored fleet table, or keep
   all research for 1.1? Folding it in changes the traffic mix, so the B6
   mix test and the long-run numbers move.

### Built (2026-11-07)

| Proof | Result |
|---|---|
| `tests/b9_wind.rs` | 5/5. **Draw**, 10,000 seeds: east flow 19.5% (plan 19.6%), gusty 19.9%, every speed and direction in range. **Replay**: same seed, same wind; Practice calm. **East long run**, Peak, 10 seeds forced east: every movement handled, 0 conflicts, 0 incidents; the controller scores 35–58 movements and 57.8–314.6 game min of delay. **Heavies**: every H and J departure in 20 seeds × 30 min is assigned 29R or 11L. **Windshear**: 283 of 10,000 gusty arrivals (2.83%; plan 2.85%), 0 of 10,000 on a steady wind; one taken end to end goes around as `weather: windshear`, counts 1 challenge and 0 delay |
| `weather.rs` and `traffic.rs` unit tests | flow rule either side of straight across 29L (013° west, 015° east); METAR text; heavies' runway: 29R 4,374 m and 11L 4,378 m, 27 2,753 m and 09 2,726 m on the graph |
| `b6_traffic` mix, 100 h at Peak (8,286 movements) | every operator within 3 points of the new table (IndiGo 55.1% against 54.9%); heavies 11.3% (research 11.1%), 13 A380s |
| `b6_long_run`, Peak, 10 seeds on their own wind (8 west, 2 east) | 0 incidents, every movement handled; delay 79.8–248.1 game min |
| Whole workspace | **86 passed, 0 failed** (`lib` 29, `b4_rules` 15, `b5_arrivals` 13, `b7_score` 8, `b6_traffic` 6, `b9_wind` 5, `b6_long_run` 2, `b4_long_run` 2, `twr-osm` 6) |
| Playwright `verify_b9.py` | 21/21. 0 alert/confirm/prompt calls in the source and the build; fresh, v1 and v2 profiles each play a minute of Normal with no error and keep their saved data; Practice reads calm, west; seed 8 (Light) draws 117/16 east; an arrival landed on 10 and an A20N took off from 11L by real keys and clicks; strip tooltip `EXPRESS INDIA 3937 · B38M`; the result dialog reads `wind 117/16, east flow`; both themes at 1500 and 1920 px: bar fits, 0 px empty at the right; reduced motion: zoom lands in one frame, without it the spring runs |
| `verify_b2.py` to `verify_b7.py` | 29/29, 22/22, 35/35, 40/40, 34/34, 22/22 |

**Performance with 42 departures taxiing at once (`tools/perf_80.py`, Vega 3 GPU, 600 frames each):**

| State | Screen | `app.frame` median | p95 | Frames over 20 ms |
|---|---|---|---|---|
| parked, 81 in play | 1500×950 | 2.7 ms | 10.6 | 86 |
| parked | 1920×1080 | 2.9 ms | 12.0 | 126 |
| parked | 1500×950, 2x | 2.4 ms | 7.4 | 16 |
| **moving**: 42 pushed and taxiing by real clicks | 1500×950 | **3.8 ms** | 13.3 | 36 |
| moving | 1920×1080 | **4.1 ms** | 30.0 | 149 |

**The median stays under half the 16.7 ms budget with 42 moving, so the
done-when holds and the conflict check needs no fix.** The tail does not
hold still: the same parked state gave p95 9.7 ms and 85 slow frames at
1920 px on the previous run, against 12.0 ms and 126 here, and the early
measurement gave 5.4 ms and 18. The cause of the spikes is not found;
unconfirmed. With nobody controlling 42 taxiing aircraft, 11 incidents
happen in 600 frames, so the moving state also times the incident path.

**Deviations from the plan:**
- **Wind sits in the strip column, not the status bar.** In the bar it
  overflowed 107 px at 1500 px. The column shows `117/16 · east flow` and
  `land 11R 10 · depart 11L 09` as text, where the plan had the runways in a
  tooltip.
- **Every operator is kept, weighted in thousandths of a percent.** The
  plan said "rows under 0.5% are dropped". That would have dropped every
  foreign carrier (0.15% each) and most of the heavies. Dropped instead:
  Centrum Air and Aero Nomad (no ICAO code), Alliance Air's Do 228 (no wake
  row). 68 operators, 24 types.
- **The flow rule's boundary is tested at 013° and 015°, not at exactly 0
  headwind.** At 014° the float headwind is about 10⁻¹⁶, either side of 0.
- **Two B4 coverage floors were lowered.** Heavies can no longer use 27 or
  09, so B4's own heavies (25% of its spawns) queue on fewer runways. Wake
  boundary floor 50 → 40 (the seeds read 67 / 48 / 73); parallel boundary
  20 → 15 (31 / 17 / 26). These floors check that the boundary gets
  exercised; incidents stayed at 0 on every seed. `wake_across_linked_runways_only`
  dropped "27 then 28" and "09 then 10": no heavy can lead from those now.
- **`entries_for(desig, min_m)`**, a read-only page hook, so scripts can
  taxi a departure to its assigned runway.
- `verify_b6.py` checked callsigns against the old 15 operators and wake
  `[MH]`; it now takes any 3-letter prefix and `[MHJ]`.
- In B7's controller test, Normal seed 3 now reaches 20 of 22 movements by
  30:00, against 22 of 22 before. The test asserts the shift's end, not
  the count. Not investigated.
- No title screen (F2): the map corner carries the credit, date and the
  navigation warning.
- The strip tooltip is written only when it changes, not every frame.

**`README.md` is written** (sections 1–6, with the assumed-vs-solid table).

---

## B10 — the real airport, and aircraft that look like aircraft (1.0.1)

Written 2026-11-07, against 1.0 as committed (`87f15ba`). Your report: T1
has a new layout, the T2 and T3 stands are not built, and aircraft are
triangles. Method: counted every aeroway element in `data/vidp.osm.json`
by the nearest terminal; read what `crates/osm` keeps (`stand_ids`, the
`.twr` line kinds) and what `crates/web` draws (`draw`, the triangle at
`lib.rs:1019`); read the `aircraft` research's `geometry/` outlines;
searched for the eAIP's VIDP apron charts and probed AAI's site (it
answers). **Nothing below is implemented — this is the plan.**

### Findings

#### M1 · BUG (high): T3 has no stands at all, and T2 has 12

`crates/osm/src/main.rs:281` keeps only `parking_position` **nodes** as
stands. OSM maps T3 differently: 40 `aeroway=gate` nodes (refs like 53–58,
at the building's edge) and 4 stand-centreline ways, with 0
`parking_position` nodes. So T3 gets 0 stands in the game and T2 gets 12.
Of the 115 stands in the game, 91 are at T1 and 12 are at the cargo apron.
Every departure therefore spawns at T1 or cargo, and Air India and every
foreign carrier operate from T1. That is wrong: they fly from T3.

**Fix:** the stands come from the official chart, not from OSM (M2), and
their names stay as the eAIP prints them.

#### M2 · DESIGN RISK (high): OSM cannot tell us whether T1 matches the new layout

OSM's T1 outline is a new way (`1429256812`), so someone has redrawn the
building. Whether its 91 stands and their taxilanes are the new layout or
the old one cannot be checked from OSM alone, and I have no imagery.

**Fix:** use the eAIP. AAI publishes VIDP's aircraft parking and docking
charts (AD 2.24), and they list each stand with a WGS 84 coordinate. A
2023 copy shows the format: `201 28°33'35.05"N 077°05'12.10"E`. A December
2025 NOTAM amends the Apron II chart, so the chart is maintained. Steps:

1. Fetch the current AIRAC's VIDP AD 2.24 charts from aim-india.aai.aero
   and keep the PDFs in `research/sources/vidp/` (git-ignored).
2. Transcribe every stand into `data/vidp.stands.csv` with these columns:
   `stand, lat, lon, apron, terminal, max_code_letter, chart, airac`. A
   script parses the coordinate tables; I check them by hand against the
   chart.
3. The extractor takes stands from that file, not from OSM. It links each
   one to the nearest taxilane, as it does now.
4. **Measure the taxilane fit.** Report every eAIP stand more than 60 m
   from any OSM taxilane. Many misses around T1 would mean OSM's taxilanes
   there are also old. The fix is then new taxilane segments in
   `data/vidp.overrides.toml`, traced from the same chart's lead-in lines.
   This step's size is unknown until the measurement; it is reported
   before any segment is drawn.

**If the AAI site refuses the fetch or the charts carry no coordinate
table:** I stop and say so. I do not fall back to guessing stand positions.

#### M3 · DESIGN RISK (medium): aprons and terminals are in the data but never drawn

The raw extract holds 45 apron polygons and 10 terminal outlines: T1,
three for T3, two unnamed near T2, and four freight. `fetch-osm.sh` keeps
them "so the map can draw them later". The `.twr` format has no line kind
for polygons, so nothing draws them, and the map reads as bare lines.

**Fix:** a `p` line kind in `.twr` (kind, then a ring of points), written by
the extractor and parsed by `airport.rs`. The renderer fills each apron with
a light tone and each terminal with a darker one, under the taxiways, in
both themes. Terminals get their names as labels. The route graph is not
touched.

#### A1 · MODEL GAP (medium): every aircraft is a triangle

`lib.rs:1019` draws a triangle along the direction of travel, the same
size for an ATR and an A380.

**Fix:** a top-down silhouette at true scale, turned to the heading.

| Type group | Shape | Grade |
|---|---|---|
| A20N, A21N, A321 (~63% of movements) | traced outlines from `research/out/aircraft/geometry/` (Airbus sheets, span within 1.1%) | primary |
| A320, A319, B38M, B738, B737, B739, E190 | the A20N outline stretched to each type's length and span (`types.csv`) | derived; a 737's nose and tail differ, so the shape is approximate |
| widebodies (B788, B789, B77W, A359, A333 …) | trace the 777-300ER from Boeing's vector sheet (the research found its dimension lines leak into the fill; hand-set exclusion boxes fix that), then stretch it per type | primary for B77W, derived for the rest |
| AT76, DH8D | an `authored` straight-wing turboprop outline, stretched per type | authored; the README says so |
| A388 | the 777 outline stretched to 79.75 × 72.7 m | derived; it lacks the A380's double deck, which top-down does not show anyway |

**Zoomed out, a 36 m aircraft is 2–3 pixels.** Below a floor of 14 px of
wingspan, the silhouette is drawn at 14 px with a thin outline, so it
reads as a shape at every zoom. Above the floor it is true scale. The
selected aircraft keeps its highlight, and conflicts keep their red.

#### A2 · BUG (medium, made visible by A1): any aircraft can park on any stand

`traffic.rs` picks a random free stand for a departure, whatever its size.
With true-scale shapes, a 777 on an ATR stand would overlap its
neighbours on screen. The eAIP's `max_code_letter` per stand fixes it. A
departure spawns only on a stand that fits its letter (`types.csv`
`icao_code_letter`), and an arrival's taxi to a stand too small is refused
inline: "stand 134L takes code C; B77W is E". This needs M2's file, so it
is built after it.

### Added 2026-11-07, at your request (approval pending)

#### C1 · Strip bays: the list splits by what each aircraft needs from you

Today `#bay` is one list, in order of appearance. It becomes stacked bays,
each with a header and a count. A strip moves bay by itself when its phase
changes; no drag needed.

| Bay | Phases |
|---|---|
| **Pushback** | `OnStand` departures, `Pushback` |
| **Ground** | `Holding`, `Taxi`, `HoldPosition`, `HoldShort`, crossing; arrivals once vacated |
| **Runway** | `LineUp`, `LinedUp`, `TakeoffRoll` (see question 4) |
| **Landings** | `Final`, `Landing`, `GoAround` |

Inside a bay, the strip that needs you soonest goes first: on final, by
distance to the threshold; on the ground, by time waited. Departed and
shut-down strips leave, as in B6. Every bay collapses with a click on its
header; a collapsed bay still shows its count.

#### C2 · Tail direction on pushback

`Aircraft::push` (`aircraft.rs:470`) reverses down the stand link and stops
at the junction, so its heading there is whatever the link gives. Real
push clearances name the tail: "push back, tail west".

**Fix:** P on a stand shows the push's possible ends at its junction: one
arrow per taxilane branch, labelled by compass (`tail W`, `tail E`). Click
one, or press ← / → to cycle and Enter to send. The push runs down the link
and on along the chosen branch for one aircraft length (the type's
`length_m`), and ends facing the other way. A branch with no room (another
aircraft within the length) is refused inline. P, then Enter at once,
takes the first branch, so a fast push stays one key away. The taxi that
follows already refuses a U-turn, so the tail direction decides which way
the aircraft can leave.

#### C3 · More control on the ground

What exists: P, T with a click-built route (vias, avoids), H to hold, X to
cross, L to line up. Which of these to add is open question 5. Each is
small on its own:

| Verb | Does |
|---|---|
| **Hold short of a point** | while taxiing, click a node ahead: the aircraft stops there and waits for T or a continue key |
| **Continue** (`Enter` on a held aircraft) | resume the cleared route after H or a hold-short point |
| **Reroute mid-taxi** | T on a moving aircraft: plan from the next node, the new preview shown before it applies |
| **Give way** | "give way to IGO512": waits until that aircraft has passed the next junction, then continues |
| **Taxi speed** | slow (5 m/s) / normal (10 m/s) / expedite (15 m/s, `authored`) |
| **Cross without stopping** | X given ahead of the hold: no stop at that runway if it is clear on arrival; the R3 check still applies |
| **Backtrack** | taxi along a runway to its end and turn there (Delhi's 09/27 needs it from some entries) |

#### C4 · Choose the runways in use

The wind panel (B9) gets a runway picker: each runway end in the wind's
direction, with **Land** and **Depart** toggles. A runway can be both
(mixed mode). New movements are assigned from what you picked; aircraft
already assigned keep their runway, and you reassign one by its strip.

**Rules:**
- **One direction at a time.** All active ends face west (27/28/29) or all
  face east (09/10/11). The conflict model does not link opposite
  directions (`conflict.rs::wake_links`), so a mix would be unchecked.
  Switching direction is allowed. Everything already assigned keeps its
  runway, and new movements wait until the old direction's runways are
  clear. See open question 6.
- **Tailwind:** an end with more than 10 kt of tailwind is refused inline
  (a typical certified limit, from memory, unconfirmed); 5–10 kt is
  allowed with a warning. The wind's own pick (B9) is the default each
  shift.
- **Heavies still need 3,100 m.** If no departure runway you picked is long
  enough, heavies wait on stand, and the panel says so.
- Delay (B7) does not count waits caused by your runway choice as yours
  unless the aircraft is waiting for a clearance you could give. The rules
  in B7 already say this.

### Proof

| Check | How |
|---|---|
| C1 | Playwright: one aircraft walked from stand to airborne passes through Pushback, Ground and Runway, in order; an arrival through Landings then Ground; counts in each header match the hooks |
| C2 | sim tests: a push with each branch ends one length along it, facing away; a blocked branch is refused; Playwright: ← → Enter pushes tail east, then a taxi west is accepted and a taxi east refused as a U-turn |
| C3 | one sim test per verb chosen, at its boundary; Playwright drives each by keys and clicks |
| C4 | sim tests: east and west ends never active together; a tailwind over 10 kt is refused; a heavy waits when no long runway departs; Peak long run with a non-default pick, 0 incidents |
| M1/M2 | every stand in `vidp.stands.csv` appears in the game with its eAIP name; counts per terminal printed and checked against the charts; T3 and T2 have stands; T1's match the current chart |
| taxilane fit | a test asserts every stand links within 60 m, after any overrides |
| M3 | Playwright screenshots of T1, T2/T3 and cargo in both themes; the apron fill reaches at least 1.3:1 against the background and the taxiway lines keep 3:1 |
| A1 | a sim test: every type in `TYPES` has a shape; drawn span within 2% of `types.csv`; Playwright: a selected A20N on stand, zoomed in, measures 35.8 m ±2% on screen |
| A2 | 10,000 departure spawns: no stand gets a type over its letter; a taxi to a stand that is too small is refused inline; B6–B9 long runs stay at 0 incidents |
| regression | the whole workspace, `verify_b2`–`verify_b9`, `perf_80.py` (silhouettes cost more than triangles: quote the median) |

### Out of scope

- **Satellite or aerial imagery as a backdrop.** That is a licensing
  question, and the game ships offline in one file.
- **3D aircraft or a 3D view.** See open question 1.
- **Changing OSM itself.** If the taxilanes are stale, the fix lives in
  our overrides file. Updating OSM upstream is yours to do if you want to.
- BLR and BOM, LVP, per-type speeds (1.1, unchanged).

### Answered 2026-11-07

1. Models: **2D top-down silhouettes**. 2. Stand source: **fetch the eAIP in
this session**. 3. T2: **follow the eAIP**. M1–A2 **approved**.

### Measured 2026-11-07, after approval: the eAIP charts

Fetched from eAIP India, amendment effective 29-10-2026: the aerodrome chart
and the 15 Parking/Docking charts (`research/sources/vidp/`, git-ignored).
`research/work/vidp/stands_from_charts.py` reads every chart's stand table
(stand, WGS 84 position, code letter) into `data/vidp.stands.csv`: **270
stands, every coordinate row on every chart matched.** Skipped: the three
general aviation charts (they print no coordinates, and GA is not airline
traffic) and the special stands 800–803.

| Apron (chart) | Stands | Chart updated | OSM stand of the same name |
|---|---|---|---|
| Apron I, T1 (101–137, 140–178) | 82 | 13 Feb 2025 | **all 82, median 2–3 m off, max 5 m** |
| Apron II | 45 | 15 Jan 2026 | none |
| Aprons 31–35, T3 | 131 | 13 Feb 2025 | none |
| Cargo | 12 | 13 Feb 2025 | all 12, median 10 m off |

**M2 is answered: the game's T1 stands are the published layout.** The
chart was updated 13 Feb 2025 and is still current. If T1 changed after
that, the official chart does not show it yet either. What looks wrong at
T1 is likely M3: no building and no apron are drawn.

**M1 is bigger than counted.** Apron II and all of T3 (176 stands) are
missing from OSM's stand nodes, not just thinner.

**The plan's 60 m fit test was wrong.** Every chart stand sits 60–134 m
from the nearest OSM taxi line, including T1's, which match exactly. The
chart's point is the stand near the building; the apron taxilane runs one
aircraft length out. The check becomes: every stand links to a taxilane
with a link that crosses no terminal and no other stand's link, and the
longest link is printed per apron.

**C1–C4 approved 2026-11-07:** four bays; every C3 verb (hold short and
continue, reroute mid-taxi, give way and speed, cross early and
backtrack); runways one direction at a time.

**Build order** (riskiest first, each slice run before the next): M2/M1
stands from the eAIP · M3 aprons and terminals · A2 stand fit · A1
silhouettes · C1 bays · C2 tail direction · C4 runway picker · C3 verbs.

### Earlier open questions (answered above)

1. "Airplane models": top-down silhouettes at true scale on the 2D map
   (this plan), or a 3D view? The two are different builds.
2. The stand source is the eAIP, fetched in this session. The `vidp`
   research run was meant to do this, and its logs are empty. Do it here,
   or wait for that run?
3. T2: OSM shows aprons and 12 stands there. Is T2 open in your picture of
   the airport, or closed? The eAIP will say what is published; you may
   know something the chart does not.

### Built 2026-11-07: M3 aprons and terminals

**`p` lines carry 55 outlines: 45 aprons and 10 terminals.** The extractor
writes them after the network, in way-id order. Re-extracting changed
nothing else in `vidp.twr`: the diff against the pre-M3 file is only the
added `p` lines. The area block first landed inside the error branch, after
its `return`, so it never ran. It now runs after the error check.

| Token | Light | Dark | Against |
|---|---|---|---|
| `--apron` | `#dddde0`, 1.36:1 | `#232327`, 1.34:1 | `--bg` |
| `--term` | `#bfc0c5`, 1.34:1 | `#3c3c41`, 1.43:1 | `--apron` |
| `--dim` taxi lines | 5.12:1 / 3.82:1 | 4.55:1 / 3.19:1 | apron / terminal |

**Labels sit on the inside point farthest from the outline**
(`Area::label_point`, a 24 × 24 grid search, cached at load). The vertex
mean put "Terminal 3 (International)" on the apron, beside its V-shaped
pier; `area_label_sits_inside_a_v_shape` holds that case. A name is drawn
only when the building spans 120 px or more. Two unnamed outlines near T2
stay unlabelled: OSM gives them no name, and the game does not invent one.

**COSMETIC (low), left as is:** in light theme the terminal fill and the
runway stroke (`--line` `#c4c6c8`) are nearly the same grey. A darker
terminal would drop `--dim` on it below 3:1. The runway's dashed
centreline tells them apart.

**Proof.** `cargo test --release --workspace`: 87 passed, 0 failed.
`verify_b2`–`verify_b9`: 29, 16, 35, 40, 34, 22, 21 PASS, 0 FAIL.
Screenshots of T1, T3 and Apron II in both themes at 1920 × 1080, 0 page
errors. Perf, an A/B of the same build with the `p` lines stripped, at
1920 × 1080 with 42 moving: medians of 4.75 and 3.40 ms without areas,
7.90 and 4.10 ms with them. The spread between two runs of one build is as
wide as the gap between builds, so the fill cost is not separable from
noise. Every median stays under 8.35 ms, half the frame budget.

### Built 2026-11-07: A2 stand fit

**Every type in `TYPES` carries its code letter** from `types.csv`
(`icao_code_letter`). `stand_fits(stand, type)` passes when the type's letter
is at or below the stand's. A stand with no letter (`-`, OSM's) and a type
not in `TYPES` both pass: there is nothing to check, and refusing would
invent a limit.

**Departures spawn only on a stand that fits.** The queue is walked in
order, but a heavy with no E or F stand free waits without holding up the
A20N behind it. IGI has 47 stands of code E or F and 223 below; a FIFO head
would stall the whole queue behind one A388.

**A taxi to a stand too small is refused inline,** on the same notice path
as "occupied by": `stand D43 takes code C; A333 is E`.

**The long-run autopilot hit that refusal at once.** It sent vacated
arrivals to any free stand. Its stand pick now skips stands that do not fit;
the refusal it hit is the bug A2 exists to stop.

**Proof.** `ten_thousand_spawns_never_overfill_a_stand`: 10,030 placed,
2,303 on E/F stands, none over its letter. With the fit check disabled in
the spawn, the same test fails on `TST6 (B788) on stand 232 code C`.
`a_heavy_with_no_stand_does_not_block_the_queue`,
`letters_order_and_unknowns_pass`, and `stand_too_small_is_refused` (B77W
refused on a C stand with the exact message; A20N accepted on it).
`cargo test --release --workspace --no-fail-fast`: 90 passed, 0 failed, the
B4, B6 and B9 long runs included. `verify_b2`–`verify_b9`: 29, 22, 35, 40,
34, 22, 21 PASS, 0 FAIL.

### Built 2026-11-07: A1 silhouettes

**Every aircraft is a true-scale outline, turned to its heading.** `crates/sim/src/shape.rs` holds five base outlines and stretches one to each of the 24 types in `TYPES` by `types.csv` length and span. The aircraft's position is its nose gear, so a stand's point sits where the nose wheel parks.

| Base | Used for | Grade |
|---|---|---|
| A20N, A21N, A321 | themselves; the A20N also stands in for every other code C jet | traced (Airbus sheets) |
| B77W | itself and every code E and F type | traced, span +1.08% against the sheet |
| turboprop | AT76, AT46, DH8D | `authored`, drawn by hand to ATR 72 proportions; its nose-gear x (3.0 m) is a guess |

**The 777 trace needed one idea, not exclusion boxes.** Boeing's sheet draws the airframe as hairline fills and every grid and dimension line as a stroke, so keeping fills only (`fill_only` in `trace.js`) removes them. The plan's hand-set exclusion boxes were not needed. Result: 47 points, length 73.97 m against 73.86, span 65.50 m against 64.80. The old trace had 3 points.

**Below 14 px of wingspan the outline is drawn at 14 px** (`SILHOUETTE_MIN_PX`), so a 36 m aircraft is a shape, not 2 px, at full zoom-out. The selection ring, conflict rings and label scale with the drawn shape.

**Hit slop is 22 px beyond the drawn shape**, not beyond a point. My first A1 build measured from the shape's middle and missed 40 of 80 clicks 15 px off the position point; the regression run caught it as 52 `verify_b2` failures. Fixed with a distance-to-polygon test; the same 80 clicks now give 0 misses, and `verify_a1.py` keeps the check.

**Proof.**
- `shape` tests: every `TYPES` row has a shape and its span and length are within 2% of `types.csv`. This proves coverage and the stretch, not trace accuracy: stretching sets the span exactly.
- `verify_a1.py`, 8 PASS: an A20N on stand, zoomed in with the wheel, measures 35.80 m by 37.57 m; an A359 64.76 m (published 64.75); the narrowest aircraft at the fitted zoom is 14.00 px; a click on the tail 65 px from the position point selects it.
- Screenshots in both themes: nose-in, body along the stand link.
- **Not checked by eye:** an airborne aircraft (drawn as an outline, not filled). **No before/after run** for the silhouettes: the old build has no `aircraft_outline` hook.

### Built 2026-11-07: UX1, five bugs from a test run

| # | Reported | Root cause | Fix |
|---|---|---|---|
| 1 | a departed aircraft doesn't clear out | `drawStrips` hid finished strips only `if shift`; Practice kept them "as B2-B5 did" | hidden in every mode |
| 4 | an arrival on its stand doesn't clear out | the 120 s shut-down ran inside `traffic_tick`, and Practice has no traffic (`World.traffic` is `None`); its `inbound` map also held only arrivals that traffic spawned | `Aircraft.arrival` and `stand_since`; `World::shut_down_arrivals` runs in every world; the map is gone |
| 2 | no Line up or Takeoff button | keys only | buttons on the selected strip, one per verb it accepts now, pressing the same key; hit 44 px, drawn 28 px |
| 3 | no push-and-taxi key | P then T, with T refused while pushing | **S**: pushes, and the taxi route builder opens when the push ends |
| 5 | requests should flash on the card | the game had no notion of a request | `World::requesting`; the card pulses and says what it asks |

**What counts as a request** (your four): a departure on stand past its 60 s grace ("ready to push"), `Holding` ("requests taxi"), `LinedUp` with no wake wait or blocker ("ready for takeoff"), `Final` and not cleared ("requests landing clearance"). It shares its conditions with the delay score (`stand_ready`, `lined_up_free`) so the two cannot drift. Narrower than the score on purpose: a hold you ordered with H, a hold short of a runway in use, and a go-around are not requests. The pulse is a 12% ink tint at about 0.8 Hz, not red (red is for conflicts); the words carry the meaning. Under reduced motion the tint stays on and nothing moves.

**Limits, stated where the claim is made:**
- **Landing clearance flashes from the moment an arrival spawns**, 7–8 km out, about 86 s. That is what the four-way pick says; a distance gate (say, inside 5 km) is one line if it reads as noise.
- **Practice's IGO512 never flashes.** It has no due time (`ready` is `None`), as the score treats it.
- **A hold-short at a crossing does not flash**, though X is what it waits for. It was not in your list.
- **Verb buttons are `tabindex="-1"`.** Enter is the taxi route's send key, and a focused button would press on it too. Keyboard users have the keys.
- In taxi or exit mode the buttons hide; the hint line says what the clicks mean. There is still no Send or Cancel button.

**Behaviour change that moved a test.** A departed strip no longer updates its text, because it is hidden, so `verify_b3.py` reads `a.strip_state(0)` for "Departed" and asserts the strip is gone. It also presses the new Line up and Takeoff buttons in place of the L and K keys; the L key is still pressed in `verify_b3.py` (the refused case) and `verify_b9.py`, and K in `verify_b4.py` and `verify_b9.py`.

**Proof.**
- Sim: `an_arrival_shuts_down_on_its_stand_without_a_traffic_stream` failed before the fix (`left: OnStand, right: ShutDown`) and passes; a departure parked in the same world is not shut down. `requesting_marks_what_waits_on_you` covers each phase at its boundary.
- `verify_ux1.py`, 28 PASS, every action a real click or key: Practice arrival landed, parked and shut down 120 real s later with its strip gone; departed strip gone; S by key and by button, the builder opening only for the selected aircraft; Takeoff by button; flash text and animation in dark and light; reduced motion gives `animation: none` and a tint. **With the old `shift &&` gate restored in a copy of the page, the two strip checks fail (26 PASS, 2 FAIL).**
- Regression, 2026-11-07: `cargo test --release --workspace --no-fail-fast` **95 passed, 0 failed**. `verify_b3`–`verify_b9`: 17, 35, 40, 34, 22, 21 PASS, 0 FAIL. `verify_b2`: 29, 28, 29 PASS in three runs.
- **The one `verify_b2` failure is pre-existing.** Seed 2742064590 fails "a right-click avoid produced a different route" every time (3 of 3 on this build): the stand's preview route has no edge whose avoid leaves another route (9 of 25 tried). A pre-A1 renderer built in a scratch copy fails the same way (2 of 2). The check depends on the seed; `verify_b2` draws a fresh one per load. Left as is; it is yours to decide whether the check should pick another stand.
- Perf, `perf_80.py`, five states: `app.frame` medians 3.2 to 4.9 ms, rAF-gap median 16.7 ms in all, 7 to 22 frames over 20 ms, 0 page errors. The first run of the day read 9.45 ms in one state while other work ran; the clean rerun did not. Silhouettes cost more than triangles; the spread between runs is wider than the gap.

---

## R1 — code review, 27 items, all to be fixed

Written 2026-11-07, against 87f15ba plus the uncommitted B10, A1, A2 and UX1 changes.
Method: four parallel read-throughs (sim physics, sim world and score, web shell, osm), each CONFIRMED item reproduced in a scratch copy (cargo probes; Playwright on `dist/index.html` with real keys, wheel and clicks). `cargo test --release --workspace --no-fail-fast` 95 passed, 0 failed. `cargo clippy --workspace --all-targets` exit 101. You approved every item for fixing on 2026-11-07.
**Nothing below is implemented — this is the plan.**

### Findings, ranked

#### R1.1 · BUG (high): a new T route mid-crossing cancels the crossing, and the aircraft parks on the runway
`aircraft.rs:514-522`: the mid-edge branch of `taxi()` runs `self.cleared.clear()`; `next_hold()` then sets the limit to `max(lim, pos)`, the current position. Cross 10/28 with X, press T inside the runway span: `HoldShort("10/28") on_runway Some(1) speed 0`, still there after 400 ticks. The strip says "holding short" while the aircraft sits on the runway. CONFIRMED.
**Fix:** after the route is rebuilt, keep as cleared each crossing whose hold line is behind the aircraft (`lim < pos_mdm`). It is the rule `next_hold` already applies, so it adds no new concept.

#### R1.2 · BUG (medium): a stand reads free while its aircraft pushes back
`traffic.rs:399` (`stand_free`) and `lib.rs:334` (`World::taxi`) both test `route.end() == stand`. `push()` (`aircraft.rs:486`) makes the route [stand, link node], so from the first pushback tick the stand counts as free. A new departure spawned on node 4305, 0.1 m from the pushing aircraft; clearing its push gave `GROUND · P0 0.1 m from NEW1, min 45`, 1 of the 3 incidents that end Surge. CONFIRMED for the spawn; the `World::taxi` path is the same test, SUSPECTED.
**Fix:** one helper, `stand_taken(node)`: true if a visible aircraft's route ends there, or starts there while it is in `Pushback` or still on the stand link. Both call sites use it, so the class is fixed once.

#### R1.3 · BUG (medium): queued arrivals always beat a go-around's rejoin
`lib.rs:234-243` runs the rejoin check before movement; `traffic.rs:412-423` runs the queue after it, so the queue sees every slot first. With one 29L arrival always queued, GA1 waited 20,018 ticks (5.6 real min) ready but held, and was charged 30,000 delay ticks. CONFIRMED with a synthetic queue; a real backlog (late Surge, heavy wake gaps) is SUSPECTED.
**Fix:** run the rejoin pass before the queue pass, both after movement. Reordering two existing calls is the smallest change.

#### R1.4 · BUG (medium): two aircraft can share one line-up spot with nothing logged
`conflict.rs:199-201`: `on_ground_net` omits `LinedUp` (and `OnStand`), and `runway()` checks only rolling aircraft. Two aircraft lined up on one 29L entry: 0 incidents, 0 conflicts. CONFIRMED on one node; the real-play path (a second LineUp into the spot) is SUSPECTED.
**Fix:** add `LinedUp` and `OnStand` to `on_ground_net`. Test first that a normal line-up behind a rolling departure and a parked aircraft on its own stand stay clean.

#### R1.5 · BUG (medium): zoom does nothing while paused
`web/shell.html:648`, `crates/web/src/lib.rs:186`, `:900`: pause sends `real_ms = 0`, and the zoom spring advances by it. Scroll while paused: 0.1928 stays 0.1928; unpause jumps to 0.2876. CONFIRMED.
**Fix:** pass the real frame time to the spring and 0 to the sim while paused.

#### R1.6 · BUG (medium): after a seed and Enter, every game key is ignored
`web/shell.html:632` blurs the Start button, but focus stays in `#seed-in`; the key guard (`:385`) drops keys while an input has focus, and strip pointerdown calls `preventDefault` (`:408`). Type 7, Enter, click a strip, press Esc: still selected; P, T, K dead. CONFIRMED.
**Fix:** `document.activeElement.blur()` in place of `el("start").blur()`. One line.

#### R1.7 · DESIGN RISK (medium): line-up ignores the assigned runway direction
`route.rs:230` (`runway_run`), called at `aircraft.rs:399` and `:584`, faces the longer stretch. With 29L active, 6 of 13 entries on 11R/29L face 11R, head-on to arrivals. CONFIRMED by count.
**Fix:** pass the departure's `assigned` runway end and face it; fall back to the longer stretch only when none is assigned.

#### R1.8 · DESIGN RISK: a second finger hijacks the pan
`web/shell.html:349-372` stores `press.id` but never compares it. SUSPECTED.
**Fix:** ignore a second pointerdown; skip moves and releases from any other pointer id.

#### R1.9 · INCONSISTENCY: the `.twr` format changed without a version bump
`airport.rs:28` keeps `twr-airport 1`, though the stand record gained a required code field and the file gained `p` records (uncommitted); the header at `:23` still documents `s <node> [name]`. An old file fails with "stand code must be A-F or -", not the version error. CONFIRMED by reading.
**Fix:** bump to `twr-airport 2`, update the header, re-extract `data/vidp.twr`.

#### R1.10 · INCONSISTENCY (low): play can shift the "independent" traffic stream
`traffic.rs:436`: the schedule's RNG also picks the departure stand, against the claim at `:10-12`. Replay stays deterministic. SUSPECTED.
**Fix:** a second RNG, seeded from the shift seed, for stand picks only.

#### R1.11 · INCONSISTENCY (low): README numbers are stale
`README.md:22` says 86 tests (95 now); `:5` says 115 stands (`vidp.twr` has 270). CONFIRMED by count.
**Fix:** update both, and the 7-minute test time after the final run.

### Low items

| # | Label | Where | Defect | Status | Fix |
|---|---|---|---|---|---|
| R1.12 | COSMETIC | `osm/src/main.rs:482` | `drop_pockets` drops 1 hold without a count (OSM 46, `vidp.twr` 45) | CONFIRMED | count dropped holds in the report line |
| R1.13 | COSMETIC | `shell.html:331-337` | pixel-ratio change at the same size leaves the canvas at half resolution | CONFIRMED | re-run `fit()` on a `(resolution)` media-query change |
| R1.14 | COSMETIC | `traffic.rs:31` | Surge cap "about 44 min" is 52 min | CONFIRMED | fix the comment |
| R1.15 | COSMETIC | `shell.html:14`, `:249` | starts dark, ignores `prefers-color-scheme`; likely dark flash for saved-light users | SUSPECTED | inline `<head>` script: saved theme, else system |
| R1.16 | COSMETIC | `shell.html:337` | resize clears the canvas after the frame is drawn | SUSPECTED | redraw inside `fit()` |
| R1.17 | COSMETIC | `osm/src/main.rs:337` (uncommitted) | "no taxiway to link stands to" names no stand and aborts early | SUSPECTED | name the stand, collect with the other errors |
| R1.18 | NOISE | `S/` | 7 screenshots (1.1 MB), untracked, not ignored | CONFIRMED | move to `_archive/`, add `/S/` to `.gitignore` |

### Improvements

| # | Where | Change |
|---|---|---|
| R1.19 | `traffic.rs:428` | return early when `stand_queue` is empty (today 270 stands × every aircraft, 60 times a second) |
| R1.20 | `aircraft.rs:620` `verbs()` | stop cloning the aircraft 7 times per call (`trail` grows without limit); measure before and after |
| R1.21 | `conflict.rs` `Monitor.departures` | apply the 240 s lookback that `on_airborne` uses; `wake_wait` scans all of it today |
| R1.22 | `shell.html:449-505` | revision counter; skip strip and list rebuilds when nothing changed |
| R1.23 | `airport.rs` `p` parse | check the point count against the fields before `Vec::with_capacity(n)` |
| R1.24 | `aircraft.rs:648` | offer E only when one `set_exit` can succeed |
| R1.25 | `shell.html:57-62`, `:144`, `:590` | handbook: spring `linear()` easing, squircle in `@supports`, 44 px hit area on Start and theme, `min(460px, 100vw - 32px)` dialog, `mmss(app.shift_len_s())` for "30:00" |
| R1.26 | `shape.rs:30` | `#[allow(clippy::approx_constant)]` on the outline table, so clippy lints the whole workspace |
| R1.27 | `crates/sim/tests` | a regression test for each of R1.1 to R1.4, failing before its fix |

### The build

1. **Tests first.** R1.27: write the four sim tests; run them; each must fail on the current code.
2. **Sim bugs.** R1.1, R1.2, R1.3, R1.4; each test goes green. Then R1.7, R1.10.
3. **Data format.** R1.9, R1.23, R1.12, R1.17; re-extract `vidp.twr`; `vidp_twr_is_current` passes.
4. **Performance.** R1.19, R1.20, R1.21, R1.22; `perf_80.py` before and after, medians quoted.
5. **Shell.** R1.5, R1.6, R1.8, R1.13, R1.15, R1.16, R1.25; a Playwright check for each, failing on the old page.
6. **Hygiene.** R1.11, R1.14, R1.18, R1.26; clippy exit 0.
7. **Regression.** `cargo test --release --workspace`, `verify_b2` to `verify_ux1`, `verify_a1`, both themes.

### Out of scope

Nothing from the review. Not in this list: the pre-existing `verify_b2` seed failure (§ UX1), which is still yours to decide.

### Built 2026-11-08

All 27 items are in the code. This session finished R1.11 and R1.18, ran the regression, and fixed four failures the regression found. Three were tests that still assumed pre-R1 behaviour. The fourth was a frame-time regression from R1.25.

| Found by the regression | Cause | Fix |
|---|---|---|
| `go_around_without_clearance_and_back_on_final`: 7200 ticks, expected 7201 | R1.3 moved the rejoin after movement, so it sees the circuit reach 0 on the same tick. The `+ 1` was the old order's one-tick lag | expect exactly `GA_CIRCUIT_S * TICK_HZ` |
| `r1_24_e_only_when_another_exit_can_be_set`: E offered 3538 ticks, withheld 0 | from the default (early) exit, a later exit always had room, so the "withheld" case never arose | give the arrival the farthest exit first. Now 3102 offered, 764 withheld. **With the `exit_room` check removed from `verbs()`, the test fails** (tick 3102, 2856 m: "E offered true, another exit settable false") |
| `verify_b4` 2 FAIL, `verify_b5` 1 FAIL: incident text red swapped between themes | R1.15: first paint follows the system theme, and Playwright's default system is light. The scripts were written for an always-dark start | both open with `color_scheme="dark"`. 35/35 and 40/40 |
| `perf_80` parked: rAF-gap median 33.4 ms, 567 of 599 frames over 20 ms | R1.25 put `corner-shape: squircle` on every strip, and UX1's request pulse animates the card's `background-color`, so each pulse step repaints every squircle card. Either alone: 16.7 ms (125 and 113 frames over 20 ms, scratch copies) | the pulse is now the `opacity` of a `::before` tint layer, which the compositor animates without a repaint. Parked median 16.7 ms. Reduced motion holds the layer at opacity 1 |

**Proof, 2026-11-08.**
- `cargo test --release --workspace --no-fail-fast`: 106 passed, 2 failed (the two above) in 443 s. After the fixes, both pass alone. The fixes touch only test files, so the workspace was not rerun.
- `cargo clippy --workspace --all-targets`: exit 0, 24 warnings.
- Verify scripts: b2 30, 29, 29 PASS in three runs. The seed failure from § UX1 did not appear. b3 23, b4 35, b5 40, b6 34, b7 22, b9 21, a1 8, ux1 28, r1 16 PASS. 0 FAIL and 0 page errors in all.
- `perf_80.py` after the pulse fix: `app.frame` medians 4.9 to 7.55 ms, rAF-gap median 16.7 ms in all five states. Frames over 20 ms: 242, 242, 144 parked and 15, 37 moving.

**Limits.**
- **Frames over 20 ms in the parked states (144 to 242) are higher than UX1's 7 to 22.** The load average was 6.9 on 4 cores during the run, from other sessions. Unconfirmed whether any of the gap is the page.
- **R1.19 to R1.22 have no before/after perf numbers.** The pre-R1 page was not kept, and the uncommitted B10, A1, A2 and UX1 changes mean `87f15ba` is not it. R1.20 is guarded for behaviour (`r1_20_verbs_match_a_fresh_clone_per_verb`), not for speed.
- R1.18: `S/` moved to `../_archive/twr-S-screenshots-20261107/`, and `/S/` added to `.gitignore`.
- R1.11: README now says 270 stands from the eAIP charts (AIRAC 2026-10-29), 591 OSM taxiway ways, and 108 tests in about 7.5 min.

---

### Built 2026-11-08: C1 strip bays

**Four bays, placed by CSS `order`, not by moving strips.** Headers and strips are siblings in `#bay`; a strip's `order` is `bay * 10000 + rank`. A strip that changes phase changes `order` and moves no DOM node, so a click that began on a verb button cannot be swallowed by a re-parent. The wasm gets two read-only hooks, `bay_of(i)` and `bay_key(i)`.

| Bay | Phases | Sort inside |
|---|---|---|
| Pushback | `OnStand` departures, `Pushback` | asking first, then order of appearance |
| Ground | `Holding`, `Taxi`, `HoldPosition`, `HoldShort`, an arrival parked on its stand | same |
| Runway | `LineUp`, `LinedUp`, `TakeoffRoll`, `Airborne` | same |
| Landings | `Final`, `Landing`, `GoAround` | metres to the threshold; a landing roll first, a go-around still circling last |

**Differences from the plan, stated where they matter:**
- **Ground and Pushback do not sort by time waited.** The sim keeps no wait time per aircraft on the ground (only `stand_since` on stands). A strip that is asking (the UX1 request) goes first, then order of appearance.
- **`Airborne` sits in Runway** until it becomes `Departed` and its strip leaves. The plan's table did not place it.
- **Empty bays show no header.** A header with a count of 0 only costs 44 px of a 320 px column. A folded bay keeps its header and count.
- **Folded state is per page load**, not saved. A folded bay hides its strips, but a selected aircraft stays selected and its keys still work (checked with P).
- **The "Strips" label is gone**; the bay headers replace it.
- **Headers are 44 px tall**, not small with extra hit area: a `::after` slop would overlap the strip below.

**Proof, 2026-11-08.** `tools/verify_c1.py`, 21 PASS, six runs after the last edit, 0 FAIL and 0 page errors (one earlier run failed two arrival checks, "landing roll: still Landings" and "vacated, taxiing: Ground"; the cause is unconfirmed and it did not recur in 9 more runs). It checks: counts per header; a departure through Pushback, Ground, Runway by real clicks and keys; an arrival through Landings to Ground; Landings order in a shift (seed 7, four arrivals, DOM order equals `bay_key` order); fold, count kept, unfold; P pressed while its bay is folded; both themes by screenshot. Regression: `verify_b2` 29, `b3` 20, `b4` 35, `b5` 40, `b6` 34, `b7` 22, `b9` 21, `a1` 8, `ux1` 28, `r1` 16, all 0 FAIL. `cargo clippy --workspace --all-targets` exit 0. **Not run:** `cargo test --workspace` (no sim code changed; the web crate has no tests) and `perf_80.py`.

---

### Built 2026-11-09: C2 tail direction on pushback

**P and S now open a picker; Enter takes the first tail.** One arrow per direction on the map (the chosen one solid and heavy, a blocked one dotted), one button per direction on the strip (`1 tail W · N`, pressing its digit), ← → to cycle, Enter or a click on an arrow to send, Esc to cancel. A blocked tail is refused inline (`tail W: IGO713 is 31 m from the tail`) and the picker stays open. The push runs down the stand link, then tail-first along the lane for one aircraft length (`shape::dims`: 37.6 m for an A20N, 73.9 m for a B77W), and the aircraft ends facing back toward the junction.

**The plan's "one aircraft length along the branch" needed more than one edge.** Measured on `vidp.twr`: the lane edges at stand junctions have a median length of 38 m, and 252 of 525 are shorter than an A320. A tail follows the lane through its nodes, taking the straightest continuation at each, until the length is used up or the lane ends (16 of 924 options end early, at a dead end).

| Rule | Why |
|---|---|
| **An option that leaves the nose at a dead end is not offered.** From the node ahead, with the pushed edge off limits, the planner must reach a runway exit. | Without it, 48 of 1,020 options (stands 201–218, A08–A10, D46–D50) left an aircraft whose only route was a U-turn. Found when `verify_b2` showed "No route" on some runs, then pinned by the sim test. |
| **Fewer than two options left means no picker.** `P` pushes at once and stops at the junction, as before. | One option is not a choice. 39 of 270 stands (78 stand/type pairs of 540) fall here: the junction has one lane, or all but one trap. |
| **`Aircraft::taxi` refuses a first edge equal to the edge the aircraft is on**, and `World::plan` avoids it. | The plan said the taxi "already refuses a U-turn". It did not: only a route's own interior was checked. This also covers a mid-edge reroute, which is a behaviour change outside pushes. |

**Limits.**
- **Picking a tail now constrains where the aircraft can go.** A destination behind the tail may have no route, and the preview says "No route". That is the plan's intent ("the tail direction decides which way the aircraft can leave"), but it is new friction. `verify_b2` now tries the twelve farthest points in turn for one that has a route.
- **The default tail (Enter at once) is the first lane the graph lists, not a chosen one.** No preference is modelled.
- **Room is checked at the push's start, along the lane every 5 m and at the end**, against every visible aircraft's body (half of each length, summed). An aircraft that moves in later is the ground check's business.
- **A tail ends mid-lane, so the aircraft is `Holding` on an edge.** Nothing else assumed Holding means "at a node"; the 110-test workspace run and the verify scripts found no case.

**Proof, 2026-11-09.**
- `crates/sim/tests/c2_tail.rs`: for every stand (270) and both an A20N and a B77W, every option: rolled distance (ticks times `PUSH_MDM_PER_TICK`, independent of the option's own figure) equals one length or the lane's length to within 0.05 m; the aircraft stops on an edge facing the junction; a taxi back along the pushed edge is refused with "U-turn"; a planned taxi never reuses it and is accepted. 924 options, 924 U-turns refused, 924 taxied on. A blocked tail is refused and names the aircraft. **With the pushed length cut by 1 m, the length check fails ("rolled 366000 mdm of lane, wanted 375700"); with the U-turn refusal removed, the U-turn check fails.**
- `tools/verify_c2.py`, 17 PASS, 0 FAIL (seed 7: tail W and tail E): P opens the picker without pushing; → and ← move the choice; Esc cancels; Enter pushes tail E; the preview to a runway entry 181 edges long does not use the pushed edge; Enter sends the taxi; digit 2 and button 1 push down different lanes; S opens the same picker and the taxi builder opens when the push ends; light theme screenshot.
- Regression: `cargo test --release --workspace --no-fail-fast` 110 passed, 0 failed, run before the dead-end filter; after it, the quick suites (lib 37, `b4_rules`, `b5_arrivals`, `b7_score`, `r1_regressions`, `c2_tail`) pass again, and `b4_long_run`, `b6_*` and `b9_wind` were not rerun. `verify_b3` 21, `b4` 35, `b5` 40, `b6` 34, `b7` 22, `b9` 21, `a1` 8, `ux1` 28, `c1` 21, `r1` 16: 0 FAIL. `cargo clippy --workspace --all-targets`: 0 errors.
- **`verify_b2` still fails on some runs (2 of the last 8), for two causes, neither the tail logic.** (1) The old seed-dependent "right-click avoid" check (§ UX1), which failed 3 times in about 20 runs today; its rate before C2 is unmeasured. (2) The fallback destination (a stand, when no hold point is on screen) can be one the A20N is too big for: "stand R05 takes code B; A20N is C" (A2), so Enter refuses. That is older than C2, unconfirmed on the pre-C2 build. Both are the test's choice of destination.
- **Not run:** `perf_80.py` (Push mode draws a few arrows per frame only while open).
- **Scripts changed:** `verify_b2`, `b3`, `b9`, `ux1`, `c1` and `perf_80.py` press Enter after P or S; `verify_b2` and `b3` read the trail as the last N edges, since it now starts with the lane edges.

---

### Built 2026-11-09: C4 runway picker

**The wind panel's "Runways" line is now a picker.** A direction pair (West, East) and, for the direction shown, a Land and a Depart toggle per end (`29L 28 29R 27` or `11R 10 11L 09`). Pressed is solid ink. A runway can be on both rows (mixed mode). The wind's own pick is the default each shift, so a shift nobody touches draws exactly what B6 to B9 drew: the draw still takes one `next_u64` per movement, and with the default plan `Plan::pick` returns the same end (`b6_*`, `b9_wind` pass unchanged). Practice has no traffic, so its picker is hidden and the note says "Practice: fixed".

| Rule | What it does |
|---|---|
| **One direction at a time** | Toggling an end of the other direction is refused inline: "29L faces west, the plan is east: switch direction first". |
| **Tailwind** | Warned from 5 kt in the note ("10 kt of tailwind on 11R 10 11L 09 (over 5)"). Refused past 10: "east flow: 11R would have 14 kt of tailwind, limit 10". Both numbers are `authored`, from memory, unconfirmed. |
| **At least one of each** | The last landing end and the last departure end refuse to turn off. |
| **Heavies need 3,100 m** | A heavy departure draws only from picked ends of 3,100 m or more (29L, 28, 29R and their east twins: 4,421, 3,808, 4,404 m). With only 27 or 09 picked it queues without a runway, and the note says "N heavy departures waiting on stand: no departure runway of 3100 m or more". Putting 29R back releases it on the next tick. |
| **A switch waits** | The other direction's default ends wait until nothing is on the old direction: no queued movement with an old runway, nothing on final, landing, going around, lined up, rolling or airborne on one, and no departure assigned to one that has not left. Meanwhile new movements draw no runway and queue; once it takes over they draw from the new plan. Everything assigned before keeps its runway. Pressing the direction in force cancels the wait. |

**Limits, stated where the claim is made:**
- **A switch only ends when the old direction is empty, and the player empties it.** In `verify_c4` (nobody flies the aircraft) it was still waiting 200 game seconds on, with 6 aircraft and movements on the old runways and 1 held. The note keeps saying so and counting them; there is no timeout and no forced switch.
- **Tailwind uses the mean wind, not the gust, and one heading per direction** (284° west, 104° east, as B9 does: the four ends lie within 13°).
- **The plan's "reassign one by its strip" is not built.** Departures' runways are advice (the pilot lines up where you taxi it), and an arrival already on final keeps its runway. Moving an aircraft already on final would need a new route; deferred, nothing depends on it.
- **A heavy waiting for a runway is queued, not drawn on a stand.** It has no strip, so the delay score does not count it. It is counted in "offered", as every queued movement is.
- **Bests are not split by plan.** A shift on a non-default pick writes the same personal bests; the plan is not saved with a result.
- **Mixed mode is checked by the model, not by a mixed-mode rule set of its own:** the Peak runs below (29R lands and departs) raise nothing.

**Proof, 2026-11-09.**
- `crates/sim/tests/c4_runways.rs`, 5 tests: the `ENDS` length table equals the airport's, per end; the rules at their edges (a 100/08 wind warns 8 kt and allows west; 100/12 refuses with "limit 10" and leaves nothing waiting; the other direction's end refused; last-of-kind refusals; Practice refuses); a switch at 6 minutes into a Peak shift (waits, holds movements, `held_seen` true, takes over, 8 of 8 later movements on east runways, every aircraft assigned before keeps its runway, 0 incidents); a heavy waits when only 27 departs and is released by 29R; **Peak, 2 real hours, 4 seeds, 29R landing and departing and 28 departing as well: 0 incidents, every departure out, every arrival parked, 29R carried 13 to 23 movements per seed.** **With `HEAVY_RUN_M` set to 0 the heavy test fails; with the busy check removed from the switch, the switch test fails.**
- `tools/verify_c4.py`, 22 PASS, 0 FAIL, real clicks: Practice hides the picker; the direction in force is pressed and four ends show; 29R toggles on and off and the sim's text agrees; the last departure end refuses inline and the next press clears it; a switch says "East (waiting)" and "switching to east: waiting for west runways to clear, 6 aircraft and movements still on them", west stays in force, pressing west again cancels; on an empty airport the switch takes over at once and the east ends show; seed 2's 14 kt wind refuses the switch in words and nothing waits; dark and light screenshots.
- Regression: `cargo test --release --workspace --no-fail-fast` 115 passed, 0 failed (`b4_long_run` 115 s). `verify_b2` 29, `b3` 24, `b4` 35, `b5` 40, `b6` 34, `b7` 22, `b9` 21, `a1` 8, `ux1` 28, `c1` 21, `c2` 17, `r1` 16: 0 FAIL. `cargo clippy --workspace --all-targets`: 0 errors.
- **Scripts changed:** `verify_b9` reads the picker's pressed toggles in place of the old `#rwys` text.
- **Not run:** `perf_80.py`; a Peak run on the east direction or with a switch inside it for more than 6 minutes; a pick with 5 to 10 kt of tailwind in a long run.

## Built 2026-11-10 — title screen (W2)

**F2 is reversed: there is a title screen.** `web/shell.html` opens a native `<dialog id="title">` on every load: the name `twr`, `© OpenStreetMap contributors`, `eAIP AIRAC 2026-10-29` (constant `AIRAC` in the shell), `OSM extract` read from `app.osm_base()` (2026-11-05, from `data/vidp.twr`), and **Not for real-world navigation**. The map-corner credit stays. Start or Esc closes it; the sim is paused until then.

**Handbook picks.** Look: the result dialog's surface tokens (`--elev`, `--scrim`). Motion: opacity plus a 12 px rise on the press spring (`--spring-press`, a `linear()` transition with `@starting-style`, so a close mid-open turns back); reduced motion is a 120 ms opacity fade. Interaction: Start is 44 px high; Esc closes (native dialog).

**Limit, stated plainly.** Automated browsers (`navigator.webdriver`) skip the title, or every verify script that clicks straight after load would hit the modal. `?title=1` forces it. A real browser always sees it; a human with `webdriver` set would not.

**Checked, Playwright, real clicks.** 1500 and 1920 px, light and dark, normal and reduced motion: dialog modal, all five lines present, Start 120x44, popup against dimmed backdrop 3.35:1 light and 2.92:1 dark (floor 2.5:1), no console error, Start and Esc both close. In the snapshot build: `verify_b9.py` 21/21, `verify_ux1.py` 28 PASS 0 FAIL.

**Housekeeping.** The 34 root-level screenshots moved to `../_archive/twr-screens-20261110/`; `/*.png` added to `.gitignore`. README "Data, licence, privacy" gained the title-screen paragraph. Nothing committed, `crates/` untouched.

---

### Built 2026-11-10: verify debt

**`verify_b2` no longer flakes, and the two Peak long runs C4 listed as not run are done.** Built and run in a snapshot of the folder (no `target/`), then only `tools/verify_b2.py` and `crates/sim/tests/c4_long.rs` copied back.

**`verify_b2`: 16 of 20 passed before, 20 of 20 after** (same machine, same page, 20 runs each, a random seed per run). The 4 failures, by cause:

| Failures | What failed | Cause |
|---|---|---|
| 3 of 20 | "aircraft moves in real time: 0.0 px", "followed path equals drawn path", "aircraft stops at the clicked destination" | The fallback destination (a stand, when no hold point is on screen) was one the A20N is too small for, so Enter refused. **Inferred, not read from the hint:** the three fail together at the first move after Enter, and only the stand fallback can be refused there. |
| 1 of 20 | "a right-click avoid produced a different route" (seed 3221133235) | All 38 preview edges of the one destination tried had no alternative, so every avoid cut every route. The planner was right; the test had one destination. |

- **Fix 1: no stand fallback.** The script zooms out about the canvas centre (up to 8 wheel notches) until 6 hold points are on screen. A hold point has no size rule.
- **Fix 2: one loop over destinations.** A destination whose every avoid cuts every route is dropped and the next of the 12 farthest is tried. The check still fails if none of them has an avoidable edge.
- **Limit:** 20 of 20 is a sample, not a proof. A clean 20 cannot show a failure rate below about 14% (95% confidence). The harness count (29 checks) varies run to run with the via search, as before.

**`crates/sim/tests/c4_long.rs`: Peak, 2 real hours, 4 seeds each, shared auto-controller, `halt_on_end` off.** Both tests pass (42 s). Every run: 0 incidents, every departure out, every arrival parked, every movement on an end of the intended direction. Delay is `Score::delay_min`, **aircraft-minutes summed over aircraft**, not a mean per aircraft; the range is the longest wait of any one aircraft (the shortest was 0 s in all eight runs).

| Seed | Arr / dep | East flow (100/06): delay, longest wait | Tailwind, west plan: wind, delay, longest wait |
|---|---|---|---|
| 1 | 18 / 23 | 136.7 min, 573 s | 100/06, 100.1 min, 442 s |
| 2 | 23 / 25 | 278.2 min, 915 s | 100/07, 320.1 min, 1002 s |
| 3 | 16 / 20 | 54.8 min, 285 s | 100/08, 62.3 min, 405 s |
| 4 | 24 / 20 | 150.9 min, 364 s | 100/09, 154.3 min, 516 s |

- **The tailwind runs use an easterly of 6 to 9 kt with the plan switched to west** (the picker's own `runway_direction`), which gives 5 to 10 kt of tailwind; the test asserts that range. The same seeds draw the same movements in both tests, so the columns compare directly.
- **East flow is forced by the wind (100/06), not picked.** It is the default east plan, not a non-default pick.
- **Tailwind raised delay on 3 of 4 seeds and lowered it on 1:** seed 1 -27% (136.7 to 100.1), seed 2 +15%, seed 3 +14%, seed 4 +2%. The sign is not consistent and four seeds are too few to say tailwind changes delay. **Tailwind has no modelled effect on the aircraft;** the sim uses it only for the warning and the refusal, so any difference comes from which runways the plan picks.
- **Score counts the first 30 game-minute shift**, so `movements` (41, 48, 36, 44) equals the log's arrivals plus departures. The remaining 90 minutes run for the incident and completion checks only.
- **Not run:** `perf_80.py`, by instruction (other windows load the machine); a switch inside a Peak run for more than 6 minutes. **Not rerun:** the rest of the workspace suite and the other verify scripts, which this change does not touch.

### Built 2026-11-10: regression (W6)

**Everything passes except `perf_80.py`, which cannot finish its "moving" state.** Nothing was fixed; this section records what ran. Run on the uncommitted working tree after `./build.sh` (the `dist/index.html` on disk was from 13:11, older than the sources, so it was rebuilt first). **The machine was never quiet:** the 1-minute load average sat between 1.0 and 3.4 for the 9 minutes W6 waited for it to drop under 1.5 (it touched 1.07 and 1.40 for one reading each, never held), and read 4 to 8 during the verify scripts and `perf_80`. Pass counts do not depend on load; every timing below is **unconfirmed**.

| Check | Result |
|---|---|
| `cargo test --release --workspace --no-fail-fast` | **126 passed, 0 failed, 0 ignored** in 16 result blocks (README said 108). Wall time about 267 s including the build, load 2.2 before and 2.95 after: unconfirmed. Slowest binary: `b4_long_run`, 109 s |
| `cargo clippy --workspace --all-targets` | Exit 0, 0 errors, **16 distinct warnings**: 8 collapsible `if`, 4 `is_multiple_of`, and 1 each of `type_complexity`, `too_many_arguments`, `manual_range_contains`, unequal hex digit groups. By file: `tests/common/mod.rs` 7, `conflict.rs` 3, `aircraft.rs` 2, `traffic.rs` 1, `web/src/lib.rs` 1, `b4_rules.rs` 1, `b6_traffic.rs` 1 |
| Stand count | 270, unchanged: 270 `s` records in `data/vidp.twr`, 270 rows in `data/vidp.stands.csv` |

**Verify scripts: 317 checks, 0 FAIL, 13 of 13 scripts pass.**

| Script | PASS | Script | PASS |
|---|---|---|---|
| `verify_a1` | 8 | `verify_c1` | 21 |
| `verify_b2` | 29 | `verify_c2` | 17 |
| `verify_b3` | 24 | `verify_c4` | 22 |
| `verify_b4` | 35 | `verify_r1` | 16 |
| `verify_b5` | 40 | `verify_ux1` | 28 |
| `verify_b6` | 34 | | |
| `verify_b7` | 22 | | |
| `verify_b9` | 21 | | |

- **`verify_b3` read 24, `tools/README.md` says 23.** The table was last updated 2026-11-08; the script gained a check since. Not edited here.
- **`verify_b2` read 29, which matches its known run-to-run spread** (29 or 30, via search).
- **`verify_r1` takes an optional page path, not a screenshot folder.** Given a folder, as the other scripts take, it exits 1 with `ERR_FILE_NOT_FOUND`; run bare it passes 16/16. `tools/README.md` shows it without arguments, so the docs are right and a loop over every script is what trips.

**`perf_80.py` fails: BUG (high) for the script, cause unconfirmed.** Three runs of three (one at load 7, two at load 6.7 and 6.9), the same stack each time. The three parked states finished; the "moving" state dies inside `set_moving` (`tools/perf_80.py:57`) on `a.phase(i)` with `RuntimeError: unreachable` from wasm. `App::phase` is `self.world.aircraft[i].phase` (`crates/web/src/lib.rs:865`), so the likely cause is an index `i` from the strip list that is past the end of `world.aircraft`, a Rust bounds panic. **That is inferred from the source, not reproduced in isolation.** Which side moved (the script's `deps` list or the web crate, whose diff is +514 lines) was not checked.

| Parked state (unconfirmed, load 7 to 8) | `app.frame` median | rAF-gap median | Frames over 20 ms |
|---|---|---|---|
| 1500x950, dpr 1 | 5.30 ms | 16.7 ms | 156 of 599 |
| 1920x1080, dpr 1 | 5.30 ms | 16.7 ms | 189 of 599 |
| 1500x950, dpr 2 | 7.00 ms | 33.3 ms | 367 of 599 |

- **The dpr 2 rAF gap of 33.3 ms is not comparable to the 16.7 ms recorded after the pulse fix.** Load was 7 to 8; no number here counts as a regression or as proof of none.
- **No moving-state number exists from this run.** The earlier "after the pulse fix" figures (§ R1) are the latest valid ones.
