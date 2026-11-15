# twr verification and leftovers — plan

Written 2026-11-14, against the uncommitted working tree of `twr/` (22 files changed since commit `0acf2b8`; `web/shell.html` last saved 09:22, `dist/index.html` and `docs/index.html` built 09:19).
Method: read the three `PLAN-*.md` files, `research/`, `git diff` and the code comments that cite the 2026-11-13 audit; ran `cargo test --release --workspace --no-fail-fast` through a test-runner agent (log: `/tmp/claude-1000/-home-talon-SyncedWork-Claude-twr/74aa4b2b-e243-4235-ab62-1ad63f6d2db1/scratchpad/cargo_test.log`); ran the five changed verify scripts, which all crashed at start; reproduced the one test failure in a scratch copy of `crates/` and `data/` (`.../scratchpad/snap-b6/`), never in the project.
**Nothing below is implemented — this is the plan.** Status 2026-11-15: B1 done (157 passed, 0 failed); A2 done (21 of 21 verify scripts pass after fixes to `verify_b6` and `verify_b7`); A3 recorded in `PLAN-twr.md`; B5 done, `dist` equals `docs`. Nothing committed. No project file was edited to produce it, and nothing is committed.

The change under test is the 2026-11-13/14 audit work: no time limit on any mode, an End shift button, Surge strikes (an incident or a go-around for no landing clearance), bests as a rate outside Surge, and `build.sh` copying the page to `docs/index.html`.

## Part A — findings, ranked

### A1 · BUG (high): `b6_traffic` hangs forever because a Surge now ends after 3 strikes

`spacing_on_final_and_free_stands_with_nobody_at_the_controls` (`crates/sim/tests/b6_traffic.rs:109`) runs `World::shift(seed, Mode::Surge)` with nobody at the controls, and loops until `w.tick` passes one real hour. Before the strike rule an idle Surge never ended. Now `score.rs` counts every no-clearance go-around as a strike, and `World::step_ticks` (`crates/sim/src/lib.rs:250`) returns at once when `halt_on_end` is true and `score.over` is set. The tick stops, the loop condition never changes, and the test spins.

**Proven in a scratch copy.** A probe on the unmodified logic printed, for the default `halt_on_end = true`:

| Seed | Surge over at tick | Real seconds | Tick after 10,000 more step calls | Strikes |
|---|---|---|---|---|
| 1 | 14146 | 235.8 | 14146 | 3 |
| 2 | 12908 | 215.1 | 12908 | 3 |
| 3 | 28768 | 479.5 | 28768 | 3 |

With `w.halt_on_end = false;` added after the `World::shift` line, the same file printed `6 passed; 0 failed; 0 ignored; finished in 46.66s`, and the three seeds ran 1.29, 1.16 and 1.28 real hours with 0 incidents. The test was never given a time limit, so a hang is how it fails; the agent saw it run over 18 minutes.

**Fix:** add `w.halt_on_end = false;` right after `World::shift(seed, Mode::Surge)` at line 113. Smallest, because five other long tests already do exactly this (`tests/common/mod.rs:654`, `c4_runways.rs:19`, `c4_long.rs:19`, `lvp_long.rs:22`, `spacing_clear.rs:19`), and the test checks spacing, not scoring.

**Class check:** any other test that runs a Surge past about 8 real minutes with no controller has the same fault. The full workspace run found only this one, since all 19 other `test result:` blocks finished `ok`. This is a result of one run on one machine, not a proof for every seed.

### A2 · DESIGN RISK (medium): no browser check of the new End shift UI exists

All five changed verify scripts (`verify_b6`, `verify_b7`, `verify_b9`, `verify_ui4`, `verify_r1`) exited 1 with `Exception: Connection closed while reading from the driver`. The cause is outside the project: `node` itself fails with `libsimdjson.so.36: cannot open shared object file`. `pacman -Q` reports `simdjson 5.0.5-1.1` (it ships `libsimdjson.so.34`) next to `nodejs 26.13.1-1`, which wants `.so.36`. **That is a partial system upgrade — inferred from those two facts, not confirmed.** `verify_b9` printed 2 PASS, both static checks (no `alert`, `confirm` or `prompt` in `shell.html` and `index.html`); 0 browser checks ran anywhere.

So the End shift button, the rate best printed as `86/h · 43 in 30:00`, the result dialog and the changed scripts have no run against this tree. Only the Rust side has one.

**Fix:** you run `sudo pacman -Syu` (it changes your system, so it is yours to run), then each script runs once, alone, in sequence.

### A3 · INCONSISTENCY (medium): the audit that drove this change is not on disk

Code comments cite `audit 2026-11-13` items A1 (no time limit, four places), A2 (`docs/` one commit behind), A4 (Surge never ending), A5, A6 and A11. No audit file exists in `twr/` or its sibling folders (a name search for `*audit*` found other projects' only). A3 and A7 to A10 appear nowhere in the code. `PLAN-twr.md` has no "Built" section for this change; its header now says "Status 2026-11-14: built", and `grep "End shift" PLAN-twr.md` finds nothing.

**Fix:** if you have the audit text, give it to me and I record each item with its state. If not, write a "Built 2026-11-13/14" section from the diff, mark A3 and A7 to A10 as unknown, and say the audit text was not found.

### A4 · INCONSISTENCY (medium): the built page is slightly older than its source

`web/shell.html` (09:22) is newer than `dist/index.html` and `docs/index.html` (09:19, byte-identical to each other). Two lines of `shell.html` are missing from `dist`: the `<!--BUNDLE-->` marker, which is expected, and one comment line about a link with no seed. **That looks comment-only, but I did not diff the build output against a fresh build to confirm it.** Rebuild with `./build.sh` before any commit, then confirm `dist` equals `docs`. GitHub Pages updates only after `docs/` is committed and pushed, and each of those needs your yes.

### A5 · MODEL GAP (medium): researched ground commands that are not built

From `research/out/ground-commands/REPORT.md`, rows built on 2026-11-11 are 1, 3, 5, 6, 7, 8 and 9. Not built: row 2 frequency handoff, row 4 start-up approval, row 10 report vacated, row 11 hold by direction or distance, row 12 cancel take-off, plus Follow. `PLAN-ui-console.md` records why each is its own plan: 2 and 4 need new state (who owns the aircraft; start-up before push), 12 needs an aborted-roll behaviour in the sim, 11 needs an offset hold the node-based stop cannot express, 10 needs an event log. Ranks in the report are `R` (recalled, unverified), not measured Delhi traffic.

**Recommendation, my judgement and not the report's rank:** plan row 12 first. It is the one that adds a decision under pressure.

### A6 · DESIGN RISK (low): the UI preference probe has no answers

`research/out/ui-pref/probe.html` has 18 pairs on 10 axes. It passed `validate_probe.py` (static) and 30/30 in Chromium on 2026-11-11 (`PLAN-ui-console.md:171`). No exported answer file exists in the folder. The three questions `NOTES.md` says would change the model most are unanswered: flat or depth on controls, 65% or 86% shell width with 32 or 48 px rows, and why the pulse was dropped (then superseded by the Needs you flash on 2026-11-11, which `PLAN-ui-console.md` says overrides A5).

**Fix:** you open the probe, pick through the 18 pairs and export the JSON. I cannot supply those answers.

### A7 · COSMETIC (low): known small items, unchanged by this work

- `verify_ui4` "Esc mid-entrance turns back" fails about 1 run in 3 on a timing read; it predates LVP (`PLAN-lvp.md`).
- `cargo clippy --workspace --all-targets` printed 16 warnings on 2026-11-10; not re-run since.
- The interrupted `ui-design` logs (`verification.log`, `flow-verification.log`) end in a browser crash (SIGTRAP). Design A was built afterwards, so they no longer block anything.

## Part B — the order of work

1. **A1 fix**, one line, then `cargo test --release --workspace --no-fail-fast`; the pass is "b6_traffic finishes 6 of 6 and the workspace total has 0 failed". The full run takes about 5 minutes under load 2.
2. **You repair Node** (A2). Check with `node --version`.
3. **Run every verify script once, alone, in sequence**, starting with the five changed ones. Quote PASS and FAIL counts; update the "Last run" column in `tools/README.md` only for scripts that ran.
4. **Record the audit change** in `PLAN-twr.md` (A3), and correct the README only where the run shows a number was wrong.
5. **`./build.sh`**, confirm `dist/index.html` equals `docs/index.html`, take a timestamped backup of `web/shell.html` first (the standing rule for large single-file apps).
6. **Commit** only after your yes in that session; a push needs a second yes.

## Out of scope

- **Fixing the ground-command gaps (A5).** Each is its own plan; one is chosen only after you pick.
- **The preference probe (A6).** It is yours to answer.
- **Clippy, the `verify_ui4` flake (A7).** Pre-existing and unrelated to this change.
- **The extra airports.** Not planned until you ask.
- **Any change to the Surge strike rule itself.** The "3 strikes" number is your call (`README.md` says so); this plan only repairs the test that the rule broke.
