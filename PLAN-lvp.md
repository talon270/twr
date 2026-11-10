# LVP — low-visibility procedures at IGI — plan

Written 2026-11-10, against the working tree as of 2026-11-10 (after C4, the
runway picker; 86+ tests passing per `PLAN-twr.md` § B9 and later).
Method: line-level re-read of `crates/sim/src/weather.rs`, `conflict.rs`,
`score.rs`, plus the parts of `arrival.rs` (`DECISION_M`, `GoAround`),
`aircraft.rs` (`step`, `clear_to_land`), `traffic.rs` (`final_slot_free`,
`FINAL_MIN_NM`), `lib.rs` (`spawn_arrival`, `rejoin_go_arounds`,
`runway_blocker`, `requesting`) and `route.rs` (`Graph::plan`) that the rules
touch. Read `research/out/separation/rules.csv` (V1–V4, R4, R5, C1, A1, G1)
and `research/briefs/{separation,vidp}.md`. The `vidp` run produced nothing,
so the primary source is read directly: AD 2.22 VIII of
`research/sources/vidp/AMDT11-2026_AD2_VIDP.html`, converted to text, and the
chart `AMDT10-2026_VIDP-ILS-CAT-III-CRTC-SENS-RWY-29L.pdf`, rendered at 200 dpi
and read by eye against its scale bar. Two numbers were measured by script
(scratchpad, no project file touched): the taxiway names the LVP tables use
against the names in `data/vidp.twr`, and the taxiway length outside the LVP
sets. Nothing was run in the sim; every timing below that says **estimate** is
arithmetic on sim constants, and step 2 replaces it with a measurement.

**Nothing below is implemented — this is the plan.**

---

## What LVP changes, in one table

| Area | Today | Under LVP | Source |
|---|---|---|---|
| Landing clearance deadline | C by `DECISION_M` = 1,160 m, else go-around `no landing clearance` | C by **2 NM = 3,704 m**, else go-around | V1, AD 2.22 VIII 4.2.1 |
| Landing clearance condition | none checked; R5 logs an occupied runway at the threshold | C only when the preceding arrival has left the **localizer sensitive area (LSA)** and a preceding same-runway departure has passed the localizer | V2, 4.2.3 |
| Protection after clearance | runway span only (R4, R5) | the LSA in front of an arrival is protected from 2 NM until its landing roll ends: anything in it is an incident | V3, 4.2.4 |
| Arrival spacing (approach's job) | 3 NM or the wake minimum (`FINAL_MIN_NM`, `final_wake_nm`) | wide enough that the leader clears the LSA before the follower reaches 2 NM | 4.1.4 |
| Taxi routes | shortest path, via and avoid | the planner prefers the published LVP taxiways for the flow; the player may still force another route | V4, 4.3.2, 4.4.4 Tables 2–3 |
| Ground spacing (G1), wake (W1–W8), R1–R5 | as built | **unchanged** | V4 game note; no LVP minimum exists |
| Departure runways | any end with the run | **unchanged**: all four runways are LVTO runways | 2.1.3 |

---

## Part A — findings, ranked

### A1 · MODEL GAP (high): the sim has no visibility, so V1–V4 are unreachable

`weather.rs` draws only `Wind { dir, kt, gust }` (`weather::draw`), and
`traffic.rs::Schedule` holds only `wind` and the runway `Plan`. Nothing in
the sim, the hooks (`crates/web/src/lib.rs`) or the page knows whether a
shift is foggy. Every LVP rule needs one shift-level flag first.

**Fix:** `Wind` gains nothing. A separate `weather::Vis { lvp: bool }` is drawn
on its **own salted stream** in `weather::draw_vis(seed)`, stored on
`Schedule` beside `wind`, and exposed as `World::lvp()`. A separate stream is
the smallest fix that leaves every existing seed's wind, schedule and stand
draws byte-identical, so B6–B9 tests do not move for non-LVP seeds.

### A2 · DESIGN RISK (high): at today's 3 NM spacing, LVP sends every follower around

`traffic.rs::final_slot_free` spawns a follower once the leader is
`FINAL_MIN_NM` (3 NM, 5,556 m) ahead along the approach. Under V1 + V2 the
follower can be cleared only after the leader leaves the LSA, and must be
cleared by 2 NM.

**Estimate, unconfirmed:** on 29L the chart puts Z2 about 2,500 m past the
landing threshold (by eye, ±10%). With `LAND_MDM_PER_TICK` (70 m/s),
`LAND_DECEL_MDM` (1.6 m/s²) and late braking to taxi speed (10 m/s), the
leader reaches Z2 about **52 game s** after the threshold, before it starts
down the exit. A 3 NM follower reaches 2 NM **26.5 game s** after the leader
crosses the threshold (1,852 m at 70 m/s). It misses by 25 s or more on every
pair. Even a follower spawned only when the leader crosses the threshold
(4.32 NM, `FINAL_SPAWN_M`) reaches 2 NM after 61 s, which is barely enough.

The concrete failure: an LVP shift where the player does everything right
logs a `no landing clearance` go-around on every second arrival.

**Fix:** under LVP, `final_slot_free` uses `LVP_FINAL_MIN_NM` and treats an
aircraft still inside the LSA as a leader even after it has turned off. The
value comes from step 2's measurement, not from this estimate. Two lines in
the one function every spawn and rejoin already goes through
(`traffic.rs` spawns, `lib.rs::rejoin_go_arounds`), so both callers get it.

### A3 · DESIGN RISK (high): the score would charge the player for go-arounds the rule forces

`aircraft.rs` `Aircraft::step`, the `Phase::Final` arm, fires
`go_around_now(GoAround::NoClearance)` whenever `!self.cleared_to_land` at the
decision point. `score.rs::waiting` counts every tick of a
`GoAround::NoClearance` circuit as delay you caused. Under LVP a correct
controller *must* withhold C while the LSA is occupied (V2). That go-around is
the rule, not the player. B7 already settled this case for departures:
`score.rs::lined_up_free` does not count a wait for wake or a busy runway.

**Fix:** a new `arrival::GoAround::LsaOccupied` variant, text `LSA occupied`.
`challenge()` stays false and `waiting` does not list it, so it counts as
neither delay nor challenge. It still costs a movement, which the score shows.
`lib.rs` reclassifies at the point it already logs go-arounds
(`GoAroundLog` push in `step_ticks`): `NoClearance` + LVP + LSA occupied
becomes `LsaOccupied`. That is smaller than passing world state into
`Aircraft::step`, which sees only its own aircraft.

**Edge stated now:** a player who parks an aircraft in the LSA makes the next
arrival's go-around free of delay. The parked aircraft is in `HoldPosition`,
which `waiting` already counts, so the cost still lands on the player.

### A4 · MODEL GAP (medium): the decision point is a constant

`arrival.rs::DECISION_M` = 1,160 m (the CAT I 200 ft point) is read directly
at `aircraft.rs` `Aircraft::step`
(`!self.cleared_to_land && out <= DECISION_M * MDM_PER_M`). V1 needs
3,704 m. The player's window from spawn to the deadline shrinks from
(8,000 − 1,160) / 70 = **98 game s (33 real s)** to (8,000 − 3,704) / 70 =
**61 game s (20 real s)**.

**Fix:** a `decide_mdm: u64` field on `Aircraft`, set to `DECISION_M` in
`Aircraft::on_final` and to `LVP_DECISION_M` by the world when LVP is on. The
world sets it in `lib.rs::draw_pilot_ga`, which its own doc names as the hook
for "each time an aircraft comes onto final" (spawn and rejoin both call it).
`aircraft.rs` compares against the field. No signature changes.

### A5 · MODEL GAP (medium): no LSA geometry, and R4's "vacated" is not "clear of the LSA"

`conflict.rs::on_runway` ends an arrival's occupancy at
`exit.vacate_mdm` (R4, `HOLD_SHORT_M` = 90 m). The 29L chart (read by eye at
200 dpi) shows the LSA running about **3.9 km**, from the localizer about
930 m beyond the 11R end to roughly the 29L landing threshold, and about
**400–450 m wide** at its widest, reaching taxiway Z on the north side and
about 185 m south of the centreline. An arrival that vacated via Z2 or Z3 and
taxis on Z is off the runway by R4 and still inside the LSA. Today nothing
can tell the two apart.

Only the 29L chart is on disk. AD 2.22 VIII 2.4.3 lists charts for 11L, 29R,
29L, 11R and 28. **RWY 10 has no chart listed, although 2.1.1 names 10 as
CAT II/III equipped**: an inconsistency in the AIP itself, and 10 is an east
flow landing runway.

**Fix:** `data/vidp.lsa.csv`, one row per station along each landing end:
`end, along_m, left_m, right_m, grade`. Distances are measured from the
landing threshold along the runway axis (from `conflict.rs::runway_dirs`),
so the file does not depend on the map projection. One function,
`lvp::in_lsa(end, pos) -> bool`, projects a position onto that axis. A
position counts with `conflict.rs::BODY_M` (20 m) of margin: AD 2.22 VIII
4.3.4 asks for the *entire* aircraft to be clear. 29L is graded
`primary (chart, by eye)`. The other four come from fetching their AD 2.24
charts (step 1). RWY 10 is `authored`: a copy of 28's shape, labelled so.

### A6 · MODEL GAP (medium): the taxi planner has no notion of a preferred route

`route.rs::Graph::plan` costs every taxi edge by its length alone. AD 2.22
VIII 4.4.4 Tables 2 and 3 publish the LVP routes per runway and terminal.
Measured against `data/vidp.twr`:

| Flow | Taxiway names in the table | Missing from `vidp.twr` | Named taxiway length outside the set |
|---|---|---|---|
| West | 89 | F6, H1, H6, LINK 19 | 9,483 m of 64,660 m (15%) |
| East | 91 | D8, D9, F6, G1, G2, LINK 19 | 8,970 m of 64,660 m (14%) |

The graph also holds 29,162 m of unnamed taxiway (`t`), mostly apron
taxilanes. These must stay free, or no route could reach a stand. H6 matters:
the west-flow route from 29L and 29R to T1 crosses 10/28 via K6 then H6.

**Fix:** `Graph::plan` takes the set of preferred names. Under LVP, a *named*
taxiway edge outside the set costs `LVP_OFF_ROUTE_COST` times its length.
Unnamed edges, runway crossings and stand links cost as today. A cost, not an
avoid list: 4.3.2 allows "alternative taxiways" for operational reasons, and
the player's `via` still wins. Deviation is not an incident, because no
minimum exists (V4).

### A7 · INCONSISTENCY (medium): a final strip asks for a clearance you must not give

`score.rs::World::requesting` returns `!a.cleared_to_land` for every
`Phase::Final`, so the strip flashes. `lib.rs::World::runway_blocker`, the
`Phase::Final` arm, names only runway occupants (`conflict::occupants`). Under
LVP the strip would flash for C while an aircraft on Z sits in the LSA,
and the strip note would say nothing.

**Fix:** mirror `lined_up_free`. Under LVP, `requesting` is false while the
LSA of that end is occupied, and `runway_blocker` names the occupant
(`LSA: IGO512`). No verb refuses: C still works, as the module rule in
`conflict.rs` requires ("the model flags; it never prevents").

### A8 · DESIGN RISK (medium): personal bests cannot tell a fog shift from a clear one

`web/shell.html` keeps bests per mode (`bests.normal.movements`). An LVP
shift lands fewer arrivals by design (A2), so it can never set a best and
its result is compared with clear-weather shifts. See open question 2.

### A9 · COSMETIC (low): stale pointers in `rules.csv`

V1–V3's `game_note` say "B9". `PLAN-twr.md` § B9 moved LVP to 1.1. A1 holds
LVP alert times (T2 30 s, T1 45 s) for an alert that does not exist yet.
**Fix:** none in code. Step 9 updates the three `game_note` cells to point
here.

---

## Part B — the build

Each step ships on its own and leaves the workspace green. The B7 rule holds
throughout: no verb refuses on separation grounds.

### Constants (all in a new `crates/sim/src/lvp.rs`, top of file)

| Name | Value | Grade | Reason |
|---|---|---|---|
| `LVP_DECISION_M` | 3,704 | primary | 2 NM, AD 2.22 VIII 4.2.1 |
| `LVP_PCT` | 10 | authored | share of eligible shifts that are foggy. Delhi's fog season is 15 Dec–15 Feb (AD 2.22 VIII preamble); the game has no date |
| `LVP_MAX_WIND_KT` | 6, steady only | authored | radiation fog needs light wind. From memory, unconfirmed. Below `CALM_KT` (5) IGI keeps west flow, so most fog shifts are west |
| `LVP_FINAL_MIN_NM` | from step 2 | authored, measured | see A2 |
| `LVP_OFF_ROUTE_COST` | 4 | authored | high enough that the preview follows the published route when one exists within 4× the length; low enough that the player's `via` still works |
| LSA stations | `data/vidp.lsa.csv` | 29L primary by eye; 28, 29R, 11L, 11R after step 1; 10 authored | see A5 |

### 1 · Data: the LSA and the LVP taxiway sets

- Fetch the AD 2.24 critical and sensitive area charts for 11L, 29R, 11R and
  28 from eAIP India, the same amendment as the 29L chart, into
  `research/sources/vidp/` (git-ignored).
- `research/work/vidp/lsa_from_charts.md`: per chart, the scale-bar pixels
  per metre and the stations read, so the numbers can be re-read.
- `data/vidp.lsa.csv`, as in A5, with a header line naming the charts and
  amendment.
- `research/work/vidp/lvp_routes_from_aip.py`: reads AD 2.22 VIII 4.4.4
  Tables 2 and 3 from the AIP HTML and writes `data/vidp.lvp.csv`
  (`flow, taxiway`), with LINK n written as `n`, the graph's name for it. The
  same pattern as `stands_from_charts.py`.

**Done when:** the script reproduces 89 west and 91 east names. A sim test in
`lvp.rs` asserts the missing-name lists in A6 exactly, so an OSM refresh
that adds H6 fails the test loudly instead of passing silently. Every landing
end of the five charted ones has at least two stations, and each LSA contains
its own runway's centreline from the threshold to the far end (test).

### 2 · Measure, then set the LVP spacing

A sim test (`tests/lvp.rs`) lands one arrival of each wake class on each of
the six landing ends with `default_exit`. It records ticks from the
threshold to (a) `exit.vacate_mdm` and (b) the first tick `in_lsa` is false
with `BODY_M` margin. It prints both.

`LVP_FINAL_MIN_NM` = the smallest whole NM at which the slowest case clears
the LSA before a follower at 70 m/s reaches 2 NM, plus one tick. The comment
above the constant quotes the measured worst case.

**Done when:** the test prints the table and the constant's comment quotes
it. Expectation (from A2): about 4–5 NM, so one arrival per runway on
final at a time.

### 3 · The fog draw and the LVP flag

- `weather::Vis` and `weather::draw_vis(seed)`, own salt, three draws always
  made, as `weather::draw` does.
- `Schedule.vis`; Practice is clear unless open question 1 says otherwise.
- `World::lvp()`.
- Hooks: `lvp` added to the wind hook in `crates/web/src/lib.rs`.

**Done when:** 10,000 seeds give LVP on 10% ±1 of eligible shifts and never on
a gusty or over-6-kt shift. A test proves every non-LVP seed in 0..10,000
draws the same wind as before the change. The whole workspace passes
unchanged.

### 4 · V1: the 2 NM deadline

`Aircraft.decide_mdm` per A4.

**Done when:** a test at the boundary passes. With LVP on, an uncleared
arrival goes around at 3,704 m ±one tick of travel (3.5 m). With LVP off,
the same arrival goes around at 1,160 m, as in the existing
`b5_arrivals` assertion.

### 5 · V2 + V3: the LSA in the conflict model

- `Rule::Lsa` added to `conflict.rs::Rule`, at the end. No variant renamed:
  the Debug text is read by scripts.
- In `Monitor::runway`, under LVP: for each arrival on `Final(d)` inside
  2 NM, or `Landing(d)` until its speed reaches taxi speed (the end of the
  landing roll), any other aircraft in a ground or rolling phase with
  `in_lsa(d, pos)` is an incident. The text reads
  `LSA · {intruder} in the {d} localizer sensitive area with {arrival} inside 2 NM`.
  It uses the `"rwy"` key and goes into the tick's `seen` set. While the
  pair stays open, R5 at the threshold does not log the same pair a second
  time: one mistake, one incident.
- `Monitor` gets the LSA table and the flag at `Monitor::new`.
- V2 at C: no new log. `requesting` and `runway_blocker` change per A7. A
  same-runway departure counts as in the LSA until `Departed`. That is
  ~10 s early against "passed over the localizer" (930 m past the end at
  90 m/s), stated in the comment.

**Done when:** sim tests cover each case:

- an arrival held on Z (H) as the next reaches 2 NM logs exactly 1 `Lsa`
  incident and 0 `Occupied` incidents for that pair;
- the same move with LVP off logs nothing;
- an aircraft that leaves the LSA 1 tick before 2 NM logs nothing;
- `requesting` is false and `runway_blocker` names the occupant while the
  LSA is busy.

### 6 · B7 score: the forced go-around

`GoAround::LsaOccupied` per A3.

**Done when:** a test shows a correctly withheld C at 2 NM goes around as
`LSA occupied`, adds 0 delay ticks and 0 challenges, and still leaves
`movements` one short. A forgotten C with a clear LSA still goes around as
`no landing clearance` and counts delay.

### 7 · V4: LVP taxi routes in the planner

`Graph::plan` per A6. `World::plan` passes the flow's set when LVP is on,
and an empty set otherwise. The empty set means today's costs exactly.

**Done when:**

- with LVP off, every planned route in the existing tests is unchanged
  (byte-equal);
- with LVP on, west flow, a 29L arrival to a T1 stand is planned via Z, T6
  or T7, A, N7, K6 (the table's route, minus the missing H6);
- a `via` off the set is still honoured.

### 8 · The page

Take a timestamped backup of `web/shell.html` first.

- Status bar: `LVP` beside the wind text. It is state, not a banner.
- The result dialog reads `wind 290/03, west flow, LVP`.
- The LSA of each active landing end is drawn as an outline under LVP only,
  in both themes, at 3:1 or better against the map background (non-text
  contrast). The player cannot keep an area clear that they cannot see.
- The strip note shows `LSA: <callsign>`.

**Done when:** `tools/verify_lvp.py` passes. It drives Playwright with real
keys and clicks. It forces LVP on a seed and checks:

- the status bar reads `LVP`;
- a held aircraft on Z shows the strip note, and the final strip stops
  flashing;
- C still works;
- the incident appears;
- the outline contrast is measured in both themes at 1500 and 1920 px;
- 0 console errors on fresh and seeded v2 profiles.

### 9 · Long runs and regression

- `b6_long_run`-style run, Peak, 10 seeds forced to LVP, each on its own
  wind. Bar: 0 incidents, every movement handled, and the go-around count by
  cause printed.
- Whole workspace, `verify_b2`–`verify_b9`, `verify_r1`, `verify_ux1`,
  `perf_80.py`. The LSA outline is drawn every frame: quote the median.
- `rules.csv` V1–V3 `game_note` cells point to this plan.
- README: LVP joins "the things most tower games get wrong" (the forced
  go-around is not your delay), and the assumed-vs-solid table gets the
  authored rows above.

**Done when:** all of the above is quoted with numbers in a "Built" section
here.

**Numbers that will move:** seeds that now draw LVP change their long-run
movements and delay. Non-LVP seeds must not move (step 3 proves it).

### Files

| File | Change |
|---|---|
| `crates/sim/src/lvp.rs` (new) | constants, LSA table, `in_lsa`, taxiway sets |
| `crates/sim/src/weather.rs` | `Vis`, `draw_vis` |
| `crates/sim/src/traffic.rs` | `Schedule.vis`; `final_slot_free` under LVP |
| `crates/sim/src/arrival.rs` | `GoAround::LsaOccupied` |
| `crates/sim/src/aircraft.rs` | `decide_mdm` |
| `crates/sim/src/conflict.rs` | `Rule::Lsa`; LSA check in `Monitor::runway` |
| `crates/sim/src/score.rs` | `requesting` under LVP |
| `crates/sim/src/route.rs` | preferred-name cost in `Graph::plan` |
| `crates/sim/src/lib.rs` | `lvp()`, `decide_mdm` set, go-around reclassify, `runway_blocker` |
| `crates/web/src/lib.rs`, `web/shell.html` | hook, status, outline, dialog |
| `data/vidp.lsa.csv`, `data/vidp.lvp.csv` (new) | as step 1 |
| `crates/sim/tests/lvp.rs`, `tools/verify_lvp.py` (new) | proofs |

---

## Out of scope

- **One-way taxi flow.** The AIP routes imply direction, but its
  `OR`/`/` grammar is irregular. Turning it into a direction per edge is a
  parser of its own. G1 still catches two aircraft meeting head-on.
- **Stop bars, clearance bars, intermediate holding positions, and CAT II/III
  holding points further back from the runway.** The graph has no data for
  any of them (`h` records are OSM's holds), and the sim holds only at
  runways.
- **RVR values and the CAT I / II / III split.** The sim would act on one
  flag either way. A number that drives nothing is decoration.
- **Safeguarding procedures, and LVP starting or ending mid-shift.** The
  runway does not change mid-shift either (`weather.rs` header).
- **A1 runway alert**, with or without its LVP times. It is not built for
  clear weather yet.
- **Taxi speed under LVP, follow-me, vehicles.** There is no primary figure
  for speed, and the sim has no vehicles.
- **Approach rules**: localizer intercept at 10 NM, no speed control inside
  20 NM. The game starts arrivals at 4.3 NM and flies one speed.
- **GP critical areas.** They sit inside the LSA on every chart read so far,
  so the LSA test covers them.
- **An LVP arrival or departure rate from IGI.** The `vidp` run found none,
  and AD 2.2 gives only clear-weather rates (86 movements, 51 departures,
  42 arrivals per hour). The rate here is whatever step 2's spacing
  produces, and the README says so.

## Open questions for you

1. **How does a player get fog?** (a) Drawn by the seed only, 10% of
   light-wind shifts. (b) The draw plus an LVP toggle in Practice. (c) The
   draw plus a toggle in every mode. I recommend (b): you can learn the LSA
   without hunting seeds, and scored modes stay seed-fair.
2. **Bests for fog shifts.** (a) Keep one best per mode; fog shifts rarely
   set one. (b) A separate best per mode for LVP, stored as a new key
   (`normal-lvp`) inside the existing `bests` map. I recommend (b). It adds
   keys, not a new shape, so v2 data loads unchanged and no
   `SCHEMA_VERSION` bump is needed. If you read an added key as a shape
   change, it becomes v3 with a migration in the same step.

## Built — 2026-11-10/11

Approved 2026-11-10 with fog option (b), a Practice toggle, and bests option (b), a `<mode>-lvp` key in the same map with no schema bump. Steps 1-9 are built (step 9 on 2026-11-11). The two browser checks were closed the same night; see "Not done". Backup: `web/shell.backup-20261110-234648.html`. Nothing is committed.

| Step | State | Proof |
|---|---|---|
| 1 Data | `data/vidp.lsa.csv` (22 stations, six ends) from the chart **vectors**, not by eye; `data/vidp.lvp.csv` (89 west, 92 east). Scripts and notes in `research/work/vidp/` (`lsa_from_charts.py`, `.md`, `lvp_routes_from_aip.py`) | `lvp.rs` unit tests, 3/3: missing names exactly `F6, H1, H6, Link 19` west and `D8, D9, F6, G1, G2, Link 19` east; each LSA holds its own centreline |
| 2 Spacing | `LVP_FINAL_MIN_NM = 5` plus "LSA empty" in `World::final_free`, which spawns and rejoins both call | `measure_lsa_clear_times`: worst 3,133 ticks (28, taxied on the tick it stops) against 1,228 from a spawn to 2 NM |
| 3 Fog draw | `lvp::draw`, its own salt; `Monitor.lvp`; `World::lvp`, `set_lvp` | 293 of 2,781 eligible seeds (10.5%); wind equals `weather::draw` on 2,000 shifts |
| 4 V1 | `Aircraft.decide_mdm` | around at 3,702.0 m with LVP, 1,157.5 m without |
| 5 V2, V3 | `Rule::Lsa` in `Monitor::runway`'s pass; `lsa_blocker`; `requesting` false while busy | held on Z: 1 LSA incident, 0 Occupied; LVP off: 0. Boundary: B inside 2 NM the tick A leaves: 0; one tick earlier: 1 |
| 6 Score | `GoAround::LsaOccupied` | withheld C: no waits for B, 0 challenges, movements 1, 0 incidents; forgotten C with a clear LSA: `NoClearance`, waits > 0 |
| 7 V4 | `Request.off_lvp`, `lvp::off_route_mask`, cost 4 | Z2 to all 270 stands: 43 routes change; off-route taxiway 15,548 m to 4,937 m; a `via` on R1 is kept |
| 8 Page | LVP tag and Practice toggle in the wind box; dashed `--dim` LSA outline; `LSA: <cs>` strip note; `, LVP` in the result line; `lsa` scenario | `tools/verify_lvp.py` 23/23; outline 6.11:1 dark, 6.94:1 light, at 1500 and 1920 |
| 9 Long runs | `tests/lvp_long.rs`: 10 Peak seeds, `set_lvp(true)`, each on its own wind, two real hours. The shared controller (`tests/common/mod.rs`) gains an LVP mode: C between `LVP_CLEAR_M` (4,000 m) and 2 NM, only with the LSA empty (`lsa_blocker`), and no G of its own; `lsa_holds` stops a taxiing aircraft short of an LSA while an arrival is on final for that end or rolling on it, and sends it on after | Before the controller change: 10 of 10 seeds fail, about 340 `no landing clearance` go-arounds each, 0 arrivals parked. After: 10 of 10 pass, 0 incidents, 232 departures out, 204 arrivals parked, 27 `LSA occupied` go-arounds (0 to 10 per seed), delay 70.5 to 331.3 min per seed over the two hours. With `lsa_holds` switched off: 7 of 10 fail, 15 LSA incidents on 28 and 10. Per-seed lines: run the test with `--nocapture` |
| Workspace | | `cargo test --workspace --release --no-fail-fast` on 2026-11-11: 143 passed, 0 failed |

**Deviations from the plan, each for a reason found while building:**
- **The chart scale bars are wrong for the strips.** 29L's bar gives 3.09 m/pt; its runway gives 5.02. Every chart is scaled on its own black runway fill mapped to the OSM runway. 10 and 28 are drawn at two scales and agree within 1%; 11L and 29R agree too.
- **RWY 10 has a chart** (eAIP index), so no end is authored.
- **The east set is 92, not 91.** `M` is real ("LINK 32, C, M"); the plan's count came from a script that no longer exists. The missing lists match exactly.
- **The graph names links `Link 19`, not `19`.** The CSV uses the graph's form.
- **The 29L route test was too specific.** A cost on names cannot force the table's order. Z2 to stand 150 runs Z, R5, Q, A, N7, K6, D, F with LVP on or off, all published names. The test now proves what the cost does across every stand.
- **V3 protects a cleared arrival only.** An uncleared arrival spends one tick inside 2 NM before its go-around fires; counting that tick logged a false incident. `verify_lvp` caught it (22/23 before, 23/23 after).
- **No `Vis` struct and no `Schedule.vis`.** One bool on `Monitor` covers both the seed draw and the Practice toggle.

**Not done:**
- **The step 9 numbers are the test controller's, not IGI's.** It holds ground traffic short of the LSA rather than at CAT II/III holds, which the sim does not have. No LVP rate is claimed.
- Nothing open. **`perf_80.py` with the outline drawn**, 2026-11-11: Surge seed 58 draws LVP (`set_lvp` refuses in scored modes), so the `-lvp` states run a different world from seed 1. `app.frame` median with the outline: parked 4.50 ms at 1500x950 and 4.20 at 1920x1080, moving with 70 pushing 5.00; seed 1 without it: 4.50, 4.40 and 5.10 (62 pushing). 0 incidents, 0 page errors, no INVALID. One run, load 2.2 rising to 5.2: the outline costs nothing measurable, not a speed-up claim.

**README entries:** written 2026-11-11 (LVP rules, the forced go-around is not your delay, the authored rows, `<mode>-lvp` bests). The long-run sentence quotes step 9.

**Verify suite after step 8** (every `tools/verify_*.py`, one at a time): 436 PASS, 1 FAIL. The FAIL is `verify_ui4` "Esc mid-entrance turns back", a timing read. **It predates LVP:** the pre-LVP shell (`shell.backup-20261110-234648.html`) failed it 1 run in 5, the current one 1 in 3; the readings jump to 0 within one sample. `verify_a1` timed out once on `goto` and passed on retry.

**`verify_ui4` fixed 2026-11-11: the read was wrong, not the page.** Before: 3 FAIL in 10, first samples 0, 0 and 0.04 after Esc, because the sample started after Esc came back through Playwright and a 215 ms reversal from about 0.3 lasts about 3 frames. The check now runs the dialog at 10x its duration (an inline `--spring-state-ms`) and starts the sampler before Esc. After: 10 of 10 pass, 42 to 75 frames in between. Control: a scratch copy of `dist/index.html` without `allow-discrete` fails it (0.6 straight to 0). The 215 ms itself is still checked by the rise test. The full verify suite was not rerun: this session changed no page or `crates/*/src` file, only `crates/sim/tests/` and two tools scripts.
