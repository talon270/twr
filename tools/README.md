# twr verification scripts

Playwright scripts, one per slice, that drive `dist/index.html` with real
clicks and keys and print one PASS or FAIL line per check. Build the page
first with `./build.sh`. Each script takes a folder for its screenshots. Give
it a scratch folder, not the project:

```sh
python3 tools/verify_b6.py /tmp/twr-shots
```

| Script | Checks | Last run |
|---|---|---|
| `verify_b2.py` | map, pan and zoom, click-to-taxi; the preview equals the route | 29/29, 2026-11-10 |
| `verify_b3.py` | push, hold short, crossings, line-up, takeoff (Line up, Takeoff and Send taxi clearance by button); seed 7, pinned | 24/24 on 3 runs, 2026-11-11 |
| `verify_b4.py` | each conflict type in both themes; the conflict panel under the map lists the pair once, selects it, and clears on H | 38/38, 2026-11-10 |
| `verify_b5.py` | arrival from final to stand, go-arounds, R5 | 40/40, 2026-11-10 |
| `verify_b6.py` | modes, seed replay, traffic, shut-down, bar layout | 34/34, 2026-11-10 |
| `verify_b7.py` | score panel, result dialog, bests, v1 -> v2 storage | 22/22, 2026-11-10 |
| `verify_b9.py` | wind and flow, east flow end to end, fresh and seeded profiles, both themes at 1500/1920, reduced motion, no alert/confirm | 21/21, 2026-11-10 |
| `verify_a1.py` | true-scale silhouettes: A20N 35.8 m on screen, a widebody at its span, the 14 px floor, tail click, hit slop on 20 seeds | 8/8, 2026-11-10 |
| `verify_ux1.py` | Practice strips clear (departed, parked arrival), verb buttons, S push-and-taxi, request flash, reduced motion | 28/28, 2026-11-10 |
| `verify_r1.py` | R1 shell fixes: zoom while paused, keys after a seed and Enter, second finger, pixel-ratio refit, first-paint theme, resize redraw, strip revision, press spring and hit areas. Its optional argument is a **page path**, not a shots folder: run it with none | 16/16, 2026-11-10 |
| `verify_c1.py` | strip bays: header counts, a departure walks Pushback to Runway, an arrival Landings to Ground, nearest threshold first, fold. Reads a strip's bay from the header above it in its column (the 2×2 since PLAN-ui-console step 2) | 22/22, 2026-11-10 |
| `verify_c2.py` | pushback tail picker: keys, Enter, Esc, digit keys, no U-turn after the push | 17/17, 2026-11-10 |
| `verify_c4.py` | runway picker: hidden in Practice, toggles, the last-of-a-kind refusal, a waiting direction switch, the tailwind refusal | 22/22, 2026-11-10 |
| `verify_ui1.py` | radio line by real key presses, both themes at 1500 and 1920; every visible control 44 px tall. Needs the shots folder | 18/18, 2026-11-10 |
| `verify_ui2.py` | PLAN-ui-console step 2: the selected-aircraft and conflict panel under the map. Map, panel and radio line stack with no gap; the panel's verbs work; the canvas and the selected strip keep their height. Needs the shots folder | 23/23, 2026-11-10 |
| `verify_ui3.py` | step 3 tokens: selection by inversion, sans prose and mono codes, no mixed-case text under 12 px, hairline contrast, the red alarm bar on screen over a request tint (pixel test). Both themes, 1500 and 1920 | 38/38, 2026-11-10 |
| `verify_ui4.py` | step 4 motion: the 215 ms state spring, the 8 px review rise that turns back mid-entrance, no `@keyframes`, everything snaps under reduced motion. The turn-back check runs the dialog at 10x its duration with the sampler started before Esc; at 215 ms the read raced the reversal. The shots folder is unused | 17/17 on 10 runs of 10, 2026-11-11 (before the fix: 3 FAIL in 10). A copy of the page without `allow-discrete` fails it: 0.6 straight to 0 |
| `verify_lvp.py` | PLAN-lvp.md step 8: the Practice LVP toggle by click, the LSA outline found by canvas diff and its contrast in both themes at 1500 and 1920, the `lsa` scenario (strip note `LSA: IGO713`, no request, withheld C goes around as `LSA occupied` with 0 incidents, C still works and gives one LSA incident), seeded v2 bests kept | 23/23, 2026-11-11 |
| `perf_80.py` | frame time with 80+ aircraft, parked and with 62 pushing at once, real GPU; prints INVALID if the Surge ends mid-timing. The `-lvp` states run Surge seed 58, which draws LVP, so the LSA outline is drawn; a different world from seed 1 | 2026-11-11, `app.frame` median: parked 4.50 / 4.40 ms at 1500 / 1920, moving 5.10 / 5.05; with the outline, parked 4.50 / 4.20, moving (70 pushing) 5.00; 0 incidents, 0 page errors. Load average 2.2 before, 5.2 after: one run, noise-sized gaps |

`verify_b2.py` draws a fresh seed each load, so its check count varies a little
from run to run. `verify_b3.py` pins seed 7 (see below).

**Since PLAN-ui-console step 2 (2026-11-10),** the selected aircraft's hint, verbs, via and notice are in `#sel` under the map (`#sel-hint`, `#sel-verbs`, `#sel-via`, `#sel-notice`), not on the strip. `#sel` carries `data-i`, the selected strip's index. The `text(k, i)` helpers in b4 and b5, and `keys(pg, i)` in ux1, read these as empty unless strip `i` is selected, which matches what the strip used to show.

**Since step 4 (2026-11-10),** strip selection, runway toggles and the result dialog move on a 215 ms spring. A script that reads a colour or a visibility right after a click reads a value on the way. b4 (wake note colour), b7 (Esc closes the dialog) and ui3 wait 300 ms first; a new check that reads a styled state after a click needs the same wait.

**Known flake in `verify_b3.py`:** "P opens the tail picker" fails when the random seed puts IGO512 on a stand with no taxilane to push along (seen on C31L and A10R). P then pushes at once, which is `start_push` in `crates/web/src/lib.rs` doing what it says. The rest of the script passes. **Fixed 2026-11-10:** b3 now loads `?seed=7`, which puts IGO512 on a stand with a tail choice; 21/21. Seed 1 still shows the old failure, 17 of 18.
