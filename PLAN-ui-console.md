# Tower console port — plan
Written 2026-11-10, 21:44 IST, against `web/shell.html` (899 lines, newest backup `shell.backup-20261110-142457.html`) and Astra's mockup round in `research/out/ui-design/` (DESIGN.md and a.html, both 20:09–20:20 today).
Method: read `DESIGN.md` in full (recommendation, A section, interaction ledger, motion contract, "What the current shell does worse"), then grepped `shell.html` and `crates/web/src/lib.rs` for each claim it makes. Nothing was run. The mockups were not opened, and Astra's screenshots were not re-viewed.
**Nothing below is implemented — this is the plan.** The session had a hard stop at 21:58, so it ends with a handoff (last section).

## Part A — findings, ranked

Each finding comes from DESIGN.md. The line numbers were checked against `shell.html` by grep in this session. The behaviour was not run.

### A1 · DESIGN RISK (medium): one 320 px column carries every job
`shell.html:104` sets `main { grid-template-columns: 1fr 320px }`. Wind, toggles, score, all four bays, incidents and go-around history scroll inside that one column.
**Fix:** use Tower console's grid. The map and its context region take about two thirds of the width, a strip rack takes 34% with the four bays in a 2×2 grid, and a full-width composer runs underneath. The score moves to the header. This is a layout change in CSS and markup only; the wasm App is untouched.

### A2 · MODEL GAP (medium): no typed command composer
The shell sends single keys to `App.key()` (`lib.rs:479`, dispatcher at `shell.html:498`). `PLAN-twr.md:38` already plans a typed line third, after click routes and single-key verbs ("`TAXI 29L VIA A D`").
**Fix:** a persistent composer that offers only tokens the Rust side reports as legal next. **Blocker:** the wasm App has no "legal next tokens" or "parse a line" export today. `verbs(i)` (`lib.rs:924`) returns the verbs for one strip, which covers the first token only. The composer must not become a second source of legal actions beside Rust (DESIGN.md says so too). That makes this the only step that needs Rust work. Grill it before building.

### A3 · INCONSISTENCY: controls at 28 px
`shell.html:75` (buttons) and `:138` (`select`, `input.seed`) are 28 px tall. Every mockup control passed a 44 × 44 check.
**Fix:** set a 44 px minimum on the three rules. This is the smallest fix because the spacing tokens stay as they are.

### A4 · INCONSISTENCY: red means two things
`.banner` (`shell.html:95–98`) uses `--red` for storage, boot and runtime failures, and red also marks conflicts.
**Fix:** give `.banner` a neutral border and a bold heading, and keep `--red` for the conflict pair only. A failure banner still stays visible and persistent (memory.md: degrade visibly).

### A5 · COSMETIC (low): looping request pulse
`@keyframes req` at `shell.html:171` loops forever. The motion contract allows no looping animation.
**Fix:** use static request words and the wait order. This reverses an earlier choice in `PLAN-twr.md` ("request flashing"), so **you decide**. I list it here but leave it out of the build until you say yes.

### A6 · INCONSISTENCY: strips not keyboard-native
Verb buttons set `tabIndex = -1` (`shell.html:578`) and strip roots are `div`s.
**Fix:** make strip roots native `<button>`s and remove the `tabIndex = -1`. Keep the map-focused key dispatcher as it is.

### Not adopted
- **The `navigator.webdriver` title skip (`shell.html:827`).** DESIGN.md flags it. Every `tools/verify_*.py` depends on it, so removing it would break 14 verify scripts for a cosmetic gain.
- **Undo on a sent clearance.** DESIGN.md says production reversibility needs its own decision. A clearance in a sim is not obviously undoable.

## Part B — the build

Before step 1, take a timestamped backup: `web/shell.backup-YYYYMMDD-HHMMSS.html`. Then run every `tools/verify_*.py` against the unchanged shell and save the output as the "before" baseline.

| Step | Work | Ships alone | Proof |
|---|---|---|---|
| 1 | A3 + A4 + A6: 44 px controls, neutral failure banner, native strip buttons | yes | the existing verify scripts pass; a new 44 px check over every control |
| 2 | A1: Tower console grid, with the score in the header, the 2×2 bays, and the selected aircraft and conflict below the map | yes | screenshots at 1500 and 1920, light and dark; a stranded-space check at 1920 (memory.md layout rule) |
| 3 | Lift the Nothing OS Classic tokens from `a.html`: mono callsigns, hairlines, selection by inversion | yes | both themes |
| 4 | Motion: the 215 ms critically damped `linear()` state spring, the 8 px slide for the result dialog, and snap under reduced motion | yes | the reduced-motion run |
| 5 | A2 composer, **only after** Rust exports the legal next tokens (see A2) | no — needs a Rust change and its tests | new `tools/verify_ui1.py`, which drives typed commands with real key presses |

Each step uses the real `App`: the renderer, routing and conflict model. None of a.html's fixture, schematic airfield or demo grammar moves into the shell.

## Out of scope
- **B and C.** You chose A.
- **Touch and Firefox/Safari.** Astra did not test them either. They come after the port.
- **An 80-aircraft workload on the new layout.** `tools/perf_80.py` exists; run it after step 2 to check that the layout did not slow the frame loop. It is not a design target here.

## Handoff — state at 21:56, 2026-11-10

Approved 21:43: Part B, drop the pulse (A5), build the Rust export (A2). Backup: `web/shell.backup-20261110-214358.html`.

| Item | State | Proof |
|---|---|---|
| A3 44 px controls | **Built** | `tools/verify_ui1.py`: every visible control 44 px or more, 1500 dark and 1920 light |
| A4 neutral failure banner | **Built** | CSS only; no run puts the banner on screen, so this is unverified by eye |
| A5 pulse dropped | **Built** | `verify_ux1.py` check 5 now asserts `animation none` (edited); result in the suite run below |
| A2 Rust export | **Built**, v1: `crates/sim/src/radio.rs`, `App.complete` / `App.send_line` | `cargo test -p twr-sim --test u1_radio`: 5/5 |
| A2 radio line in the shell | **Built**, v1 | `tools/verify_ui1.py`: 18/18, 0 console errors |
| A6 strips keyboard-native | **Dropped, 22:20.** A strip cannot be a `<button>` because it holds verb buttons, and Enter is the global "send clearance" key, so a focused strip would send, not select. You chose to drop it: each verb button shows its key, and the radio line is the typed path | none needed |
| Step 2 grid, first part | **Built 22:40.** Map 66% and rack 34%. Score is a second header row. Bays sit in a 2×2 grid of four boxes, each box scrolls inside 40vh, and an empty bay keeps its header with a 0. Backup: `web/shell.backup-20261110-221645.html` | full suite below; screenshots at 1500 dark and 1920 light, Peak seed 7: 0 console errors, Landings header at y 667 and 719 |
| Step 2, selected aircraft and conflict under the map | **Built 2026-11-10, late session.** A fixed 184 px row `#ctx` under the map: `#sel` (callsign, type, telephony, state, request, note, via, verbs, hint, notice) and `#conflicts` (one row per live pair, red rule, the row selects the first aircraft). Strips no longer grow verbs or hints. Backup: `web/shell.backup-20261110-223536.html` | suite below, 339 PASS, 0 FAIL; a scratch panel check, 23/23: canvas height unchanged when the panel's content changes, strip height unchanged on select, panel buttons 44 px, 0 px stranded at 1500 dark and 1920 light |
| Step 3 tokens | **Built 2026-11-10, late.** Body text in Liberation Sans (named: generic sans-serif is JetBrains Mono on this machine); mono for callsigns, numbers, labels, keys and controls. `--hair` hairlines from mockup A (#b8b8bc / #3e3e44, about 2:1 on both themes; old dark 1.47:1); canvas `--line` untouched because it paints runways. Each bay is one hairline box with strips as rows. Selection is inversion. Prose that was 11 px is 12 px; uppercase mono labels stay 11 px. Backup: `web/shell.backup-20261110-225101.html` | `tools/verify_ui3.py`, 38/38 |
| Step 4 motion | **Built 2026-11-10, late.** The 215 ms state spring (k 1200, zeta 1, the handbook's routine curve) on strip inversion, runway toggles and the bay chevron; the result dialog rises 8 px and fades on it, with `allow-discrete` so Esc mid-entrance turns back from where it is. No `@keyframes`. Everything snaps under reduced motion. The title dialog keeps its earlier 12 px, 200 ms press-spring entrance: the plan named only the result dialog | `tools/verify_ui4.py`, 17/17 on 5 runs in a row |

**v1 limits of the radio line, stated in `radio.rs`:** single-word verbs only (`PUSHBACK`, `TAXI`, `HOLD POSITION`, `CROSS`, `LINE UP`, `TAKEOFF`, `LAND`, `GO AROUND`, `EXIT`). No runway or route arguments; `TAXI` and `EXIT` open the existing map pickers. No minimum prefixes. C3 ground verbs (J, Y, B, slower/faster) are not in the grammar yet.

**Suite results.** Before = unchanged build; after1 = A3+A4+A5. Count is PASS lines.

| Script | Before | After1 | Note |
|---|---|---|---|
| b2 / b4 / b5 / b7 | 29 / 35 / 40 / 22 | 29 / 35 / 40 / ... | b3 is 21 vs 18; README says its count varies by seed. 0 FAIL in both |
| b6 | 34/34 | 32 + 2 FAIL | Fails were "shift clock counts down" and "Surge at 10:00: 85/h". A solo rerun on the current build failed a different check: "shift clock '29:59'". Two suites ran in parallel during after1, so this looks like a timing flake, but it is **unconfirmed**. Rerun b6 alone on the backup build and the current build |
| b9 | crashed | | My harness fault: b9 reads `web/shell.html`, which I did not copy to the scratch tree. Run it from the project |
| a1 | 8/8 | 8/8 | |
| c1 / c2 / c4 / ux1 | ... / 18 / 23 / 29 | 22 / 18 / 23 / 29 | 0 real FAIL lines in either run (grep `^FAIL`); the "fail 1" in my count was the summary line. ux1 includes the edited pulse check, which passed |
| r1 | crashed | crashed | Both runs: `ReferenceError: TWR is not defined` in `page.evaluate`. Pre-existing, not caused by this change; the edited 44 px check in it has therefore never run |

The full "after2" run (with the radio line) was still going at 21:56. Its output is in `/tmp/claude-1000/-home-talon-SyncedWork-Claude-twr/9829ff13-beb7-49e0-a1a5-bb80958f3bde/scratchpad/after2/summary.txt`; the scratchpad dies with the session, so rerun every `tools/verify_*.py` from the project.

**Update 21:57.** The after2 run (current build, with the radio line) so far: a1 8/8, b3 24, b4 35, b5 40, **b6 34/34** (so the after1 b6 failures look like load flakes, still not proven), **b2 28 + 1 FAIL: "1920x1080: map reaches the bottom (1019)"**. This one is caused by the change and is expected: the radio line now sits under the map, as Tower console asks. Fix the assertion in `verify_b2.py` to "map reaches the top of `#radio`", not the layout. b7 onward had not finished.

**Next session, in order:**
1. Done 21:58: the b2 assertion now checks map meets `#radio` and `#radio` reaches the bottom; `verify_b2.py` 29 PASS, 0 FAIL ("map meets the radio line (1019 vs 1019), radio line reaches the bottom (1080)"). after2 also had b7 22/22; b9 crashed in the scratch copy (harness fault, as before). after2 finished 21:58: c1 22, c2 18, c4 23, ux1 29, each 0 `^FAIL` lines; r1 crashed (pre-existing `TWR is not defined`). Next: rerun `verify_b6.py` alone against the backup build and the current build. If the backup also fails alone, it is pre-existing.
   Done 22:16: `verify_b6.py` alone, 34/34 on the current build and 34/34 on the backup shell (same wasm). The after1 failures were load from two parallel suites, not the change.
2. Run `verify_b9.py` from the project. Fix or report the `verify_r1.py` crash (`TWR is not defined`, pre-existing), then run its edited 44 px check.
   Done 22:18: `verify_b9.py` from the project, 21/21. `verify_r1.py`, 16/16, including the edited 44 px check. The crash was my harness, not the script: r1 takes an optional **page** path (`verify_r1.py:23`), and I passed it a shots folder, so it loaded a directory listing with no `TWR`.
3. Done 22:20: A6 dropped (see table).
4. Step 2 first part done 22:40 (see table). Next: the selected-aircraft and conflict region under the map, then steps 3 and 4.
5. `tools/README.md`: add the `verify_ui1.py` row once its results are final.

**Suite at 22:45**, current build, every script alone in sequence: a1 8, b2 29, b3 21, b4 35, b5 40, b6 34, b7 22, b9 21, c1 22, c2 17, c4 22, r1 16, ui1 18, ux1 28. Count is `^PASS` lines; 273 PASS, 0 FAIL. The 21:58 counts above for c2, c4 and ux1 (18, 23, 29) were one too high: that grep also counted each script's summary line. The check lists match line for line.

**Step 2 changes to scripts:** `verify_c1.py` reads a strip's bay from the nearest header above it **in its column**, and checks that all four headers show (`1, 0, 0, 1` in Practice). Both follow from the 2×2.

**perf_80 at 22:55**, pre-step-2 shell against the current one, same wasm, run alone in sequence. Parked states only:

| Screen | Before: `app.frame` median / p95, frames over 20 ms | After |
|---|---|---|
| 1500×950, 1x | 4.1 / 9.3 ms, 33 | 3.8 / 7.8 ms, 3 |
| 1920×1080, 1x | 3.9 / 7.3 ms, 15 | 4.2 / 8.1 ms, 7 |
| 1500×950, 2x | 4.5 / 11.1 ms, 58 | 3.9 / 10.8 ms, 26 |

The layout did not slow the frame loop. One run each, so the gaps between runs are noise-sized, not a speed-up claim. **The moving states crash on both pages, so the cause predates step 2.** The Surge shift ends during `set_moving`, the result dialog opens, and the script's next Enter presses its autofocused "Again · same seed". The world restarts empty, and `a.phase(11)` panics: "index out of bounds: the len is 0 but the index is 11" (`lib.rs:866`). Proven by a wrapped `restart` that logged its caller: the result dialog's `close` handler. Why the shift ends (incidents from the mass pushback, most likely) is **unconfirmed**. `perf_80.py` is not fixed; it needs your call on how a 60-aircraft push should avoid ending the shift.

**Known limit:** the rack still scrolls as a whole at 1500×950 when Landings holds many strips. Pushback with 10 strips no longer hides Landings.

**Step 2, second part, 2026-11-10 (late).** Two bugs found on the way, both fixed and re-run:
- My first edit dropped the `cs` write, so every strip showed "-" for its callsign. `verify_b6` caught it ("first strip - 'B738 M'"); after the fix, 34/34.
- The conflict row was rebuilt every frame because its distance changes every tick, so no click landed (Playwright: "element was detached from the DOM", 20 retries). Now the row is rebuilt only when the set of pairs changes, and the distance is written into the existing button. `verify_b4` gained 3 checks for this (35 to 38).

Script changes: the hint, verbs, via and notice selectors in b2, b3, b4, b5, c1, c2 and ux1 now read `#sel-*`; b2's layout check is map, then panel, then radio line, then window bottom. `tools/README.md` says how the helpers keep their old meaning.

**Suite after step 2**, current build, every script alone in sequence: a1 8, b2 29, b3 24, b4 38, b5 40, b6 34, b7 22, b9 21, c1 22, c2 17, c4 22, r1 16, ui1 18, ux1 28. 339 PASS, 0 FAIL. b3 is 24 vs 21 at 22:45: its count varies by seed (README).

**Known limit:** the panel is 184 px tall in every state. A Practice strip on stand uses about 130 px of it; the space is held for the tail picker's extra row and the notice, so the canvas never resizes as content changes.

**Steps 3 and 4, 2026-11-10 (late).** Bugs found on the way, each fixed and re-run:
- Step 3: the red alarm bar vanished on a strip that was also requesting. It was an inset shadow, and the request tint (`::before`) painted over it. It is now its own `::after` layer; `verify_ui3` reads the screen pixel at the strip's edge (215, 25, 33) with and without a request. Seen by eye in a screenshot, not reproduced on the old build by script.
- Step 3: the radio input stayed sans, because `.radio input { font: inherit }` outranked the mono rule. The mono rule now sits last in the sheet.
- Step 4: the reduced-motion block sat above the `.seg` and dialog rules, which would have won. It now sits last too.
- Step 4 made three script reads land mid-transition (b4 note colour, b7 dialog visibility, ui3 inversion). Each now waits 300 ms. The page was right; the reads were early.

**Suite on the step 4 build**, every script alone in sequence: a1 8, b2 29, b3 17 + 1 FAIL, b4 38, b5 40, b6 34, b7 22, b9 21, c1 22, c2 17, c4 22, r1 16, ui1 18, ux1 28, ui2 23, ui3 38, ui4 17. b4, b7 and ui3 are from solo reruns after the settle waits. 410 PASS, 1 FAIL.

**The b3 FAIL predates this session.** "P opens the tail picker" fails when the random seed puts IGO512 on a stand with no taxilane to push along: P pushes at once (`start_push`, `crates/web/src/lib.rs`). Probed: seeds 2401904318 (stand C31L) and 3342721359 (A10R) give phase `Pushback` straight after P. No Rust file changed after 21:47. Fixing it means pinning b3's seed or skipping the check on those stands; you decide.

**Both open calls, settled 2026-11-10 (late) after "continue ahead after the fix":**
- **b3 pins seed 7.** It puts IGO512 on a stand with a tail choice, so the picker check always runs: 21/21. Seed 1 still shows the old failure, 17 of 18. Branching on "did P push at once" was rejected: a broken picker that pushed at once would then pass.
- **perf_80 times the moving state during the push, not the taxi.** Probe on seed 1: 62 uncontrolled pushes gave 3 GROUND incidents (44.9 m against 45) before Holding, and Surge ends on the third. Peak cannot stand in: it reaches 37 aircraft before its 30 minutes end. The taxi loop is gone; a run where the Surge ends prints INVALID instead of a timing. Result, one run each: moving 1500×950, 62 pushing, `app.frame` median 5.30 ms, p95 9.90, 0 incidents; 1920×1080, median 5.40, p95 10.00, 0 incidents; 0 page errors in all five states. This measures 62 pushing, not 42 taxiing, so the number is not comparable to B9's 3.8 / 4.1 ms.

**Next:** this plan is finished. `PLAN-lvp.md` is next in the folder, unapproved, with two open questions.

Nothing is committed.
