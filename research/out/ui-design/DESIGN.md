# twr — three working interface directions

10 November 2026. Open [A](a.html), [B](b.html), or [C](c.html) directly from disk. Each file contains its own HTML, CSS, JavaScript, traffic fixture, and Canvas renderer. There are no font files, libraries, fetches, or build steps needed to open a mockup.

These are **interaction prototypes, not a simulator**. Twelve synthetic aircraft, the score, runway occupancy, distances, and weather are authored fixtures. The airfield is a schematic whose horizontal and vertical scales fit the panel independently. It is not an OSM extract or an aerodrome chart. Those limits appear beside the map, command entry, alert, score review, and clock. No production file was edited.

## Recommendation

1. **A — Tower console.** Recommended because its map, four bays, and full-width clearance composer keep traffic and complete taxi instructions visible together. Its stable selection and conflict areas reduce movement between tasks, while the two-column rack uses desktop width well; this is a design judgement, not a measured player-performance result.
2. **C — Surface control.** The strongest spatial picture: the map gets most of the available height, with the queue on the left and decision tools on the right. Choose it for map-first players. Its narrow command field scrolls long instructions, and selection, map, and send span three columns. At laptop height the read-only runway ledger is omitted; runway assignments remain in strips and map labels. This trade-off is visible in the supplied screenshots.
3. **B — The strip desk.** The clearest comparison of the four aircraft phases. Every bay occupies an equal horizontal lane; cream paper, olive ink, and restrained hard edges make a distinct daytime desk. Its short map sacrifices vertical geometry and aircraft data-block detail at 1500 px. Better for a strip-oriented practice view than the primary high-workload screen. That suitability is a hypothesis, not a user-study result.

The recommended implementation should retain A's hierarchy, use the real Rust route and conflict model, and replace the schematic with the existing renderer. The mockup grammar is deliberately small; it must not become a second source of legal actions beside Rust.

## A — Tower console

**Handbook look:** [Nothing OS Classic](../../../../md/designs.md), adapted for a dense desktop. The handbook explicitly grades its sourcing as `blog`; these are house tokens, not official Nothing specifications. Black/white surfaces, hairlines, system mono callsigns, restrained corner radii, and inversion for selection. Red is reserved for the conflict pair. The user's spring, Undo, and 44 px requirements override Classic's old “100 ms linear / no spring / no toasts / 40 px buttons” exceptions.

**Whole-screen structure:** a roughly two-thirds map-and-context region and a 34% strip rack, followed by a full-width clearance composer. Three independent score measures live in the header. The four bays use a two-by-two arrangement. The selected aircraft and conflict stay below the map instead of expanding a strip under the pointer.

**Motion specimens:** [Toggle travel](../../../../design-catalogue/animations.html#spec-toggle) for reversible state treatment; [Short slide](../../../../design-catalogue/animations.html#spec-slide) for the 8 px review/toast reveal. The latter's illustrative keyframes are replaced by the required critically damped CSS transition. Zoom uses the catalogue's live `springStep`, described below. Conflict onset and score values change immediately.

**Interaction cards:** all rows in the interaction ledger below apply. In particular, `command-menu` supplies the keyboard vocabulary, `hit-slop` supplies separate aircraft targets, and `tabular-numbers` keeps the rack and score stable. There is no dotted numeral hero or animated ornament.

## B — The strip desk

**Handbook look:** [Quiet neobrutalism and the cream family](../../../../md/designs.md), with a restrained olive signal. Quiet neobrutalism is a catalogue **brainstorm**, not researched guidance. The source's light-only treatment is overridden: a complete charcoal/olive dark theme ships alongside warm paper. System Georgia is used only for the desk title and wordmark; operational labels remain sans/mono. Small hard shadows mark the primary action and bay housings, rather than every piece of data.

**Whole-screen structure:** a shallow, wide map and contextual column above four equal flight-strip bays. The clearance line sits under the bays. This gives each phase a stable horizontal address, with no phase hidden behind a tab. At laptop size aircraft labels carry callsigns only; the corresponding strip carries type, wake, phase, and request.

**Motion specimens:** [Tactile press](../../../../design-catalogue/animations.html#spec-press), reduced to a 2 px reversible press with the catalogue's 118 ms press spring; [Short slide](../../../../design-catalogue/animations.html#spec-slide) for review and Undo. State is applied on activation, without waiting for the release animation. No paper-card flipping or strip-entry stagger.

**Interaction cards:** the shared ledger applies. `continuous-corners` is used even on the shallow paper housings; `undo-toasts` replaces a reset confirmation; `balanced-headers` is applied to the serif title and result heading. Bencho was read as an interaction parts bin, but no React block, sound, asset, or code was imported.

## C — Surface control

**Handbook look:** [Technical drafting](../../../../md/designs.md), using a Porcelain & petrol-derived palette. Both are authored catalogue directions; Technical drafting is a **brainstorm**. A complete blue-black/petrol dark theme is newly authored here. Fine airfield rules, runway end labels, taxiway letters, leader lines, and mono data create the drafting character. The grid is deliberately quiet behind the operational marks.

**Whole-screen structure:** 22% strip queue, flexible central map, 25% command/detail rail. The persistent conflict leads the right rail; selection and the composer follow. A read-only runway ledger appears when height permits and now grows into the available rail space, listing callsigns, phases, and notes from the current traffic fixture. The four left-hand bays share spare height through their strip rows instead of leaving a bottom gap. The ledger refreshes after commands and Undo; its heading states that strip assignments do not predict runway occupancy. These spacing fixes are **fixed; Chromium 3 runs after the change, verify.py 24/24 and verify_flows.py 3/3 each, 2026-11-10; b and c 1920 screenshots viewed, overlap and blank band gone**.

**Motion specimens:** [Toggle travel](../../../../design-catalogue/animations.html#spec-toggle) for state feedback, [Short slide](../../../../design-catalogue/animations.html#spec-slide) for review/Undo, and the live spring lab for zoom. Pan follows the pointer directly. There are no scanning sweeps, glowing aircraft, pulsing alarms, or rolling counters.

**Interaction cards:** the shared ledger applies, especially `hit-slop`, `interruptible-animations`, and `tabular-numbers`. The command card is a persistent rail rather than a popup; exact legal tokens replace fuzzy matching.

## Interaction ledger — exact UI-tip cards

Links point to the local cards that were read. This ledger applies to **each of A, B, and C**; the visual placement and motion differences are described above. A card is an adopted technique, not evidence that the entire interaction has been user-tested.

| Implemented interaction | UI-tip card | Implementation and limit |
|---|---|---|
| Select an aircraft on the map or in a bay | [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop), [10 · Interruptible animations](../../../../design-catalogue/ui-tips.html#interruptible-animations) | Separate 44 × 44 aircraft buttons and full-width strip buttons; selection is mirrored immediately. Static callouts choose among candidate placements to reduce overlap. This is not a crowded-airfield label solver. |
| Type a callsign and clearance | [03 · Command menu](../../../../design-catalogue/ui-tips.html#command-menu) | A persistent labelled combobox: arrows choose, Tab completes, Enter completes a word or sends a complete command, Escape clears and returns to the map. Exact legal-next tokens are appropriate to a grammar; fuzzy action matching is intentionally not adopted. It is inline, so it does not need the card's modal shell. |
| Use P/T/H/X/L/K/C/G/E | [03 · Command menu](../../../../design-catalogue/ui-tips.html#command-menu), [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop) | Visible legal verb buttons prepare the same command line. Map-focused shortcuts prepare commands; they never intercept typing or other focused controls. Not every production verb is included. |
| Preview a taxi route | [03 · Command menu](../../../../design-catalogue/ui-tips.html#command-menu), [10 · Interruptible animations](../../../../design-catalogue/ui-tips.html#interruptible-animations) | The 29R map button builds the same complete command as typing. The alternate `VIA A B` has a different visible edge list. Send copies the exact preview object. This verifies identity of the demo route, not routing correctness or aircraft following. Arbitrary waypoint clicks and right-click avoids are not implemented. |
| Send, then undo a command | [04 · Undo toasts](../../../../design-catalogue/ui-tips.html#undo-toasts) | Immediate local state change; six-second Undo restores the preceding traffic, route, selection, and score. Hover, focus, and a hidden document stop expiry. Ctrl/Cmd Z works outside text input, preserving native text editing inside it. One most-recent action is retained; successive actions replace it. Production clearances would require a separate decision about reversibility. |
| Locate the conflict, hold one aircraft | [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop), [03 · Command menu](../../../../design-catalogue/ui-tips.html#command-menu) | Locate selects AIC304; H then Enter applies HOLD. The scripted pair clears immediately, without an incident. Red rings, a dashed connecting line, callsigns, distance, strip markers, and explicit conflict text carry the alert; colour is not its only signal. No prediction model exists here. |
| Next waiting request | [03 · Command menu](../../../../design-catalogue/ui-tips.html#command-menu), [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop) | A visible button and map-focused Tab cycle the fixture's waiting aircraft by descending wait. Elsewhere Tab retains normal focus navigation. Wait times are synthetic and stay fixed even if the demo clock runs. |
| Pan, zoom, fit | [10 · Interruptible animations](../../../../design-catalogue/ui-tips.html#interruptible-animations), [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop) | Drag with pointer capture; wheel or 44 px plus/minus buttons zoom; Fit restores the view. The live zoom spring retains velocity when retargeted. Reduced motion snaps to the destination; pointer pan remains direct. These are schematic camera controls, not the production Rust camera. |
| Change theme; pause/resume clock | [10 · Interruptible animations](../../../../design-catalogue/ui-tips.html#interruptible-animations), [14 · Tabular numbers](../../../../design-catalogue/ui-tips.html#tabular-numbers) | Theme follows the system until explicitly chosen; changing it also repaints the canvas. The clock button changes its label and pressed state. It runs only the demo clock, as stated beside it. No persistent settings are written. |
| Review the score; close review | [03 · Command menu](../../../../design-catalogue/ui-tips.html#command-menu), [11 · Predictive back](../../../../design-catalogue/ui-tips.html#predictive-back) | Native modal `<dialog>` and `::backdrop`; Escape or Back to desk closes the top layer and returns focus. Review pauses the clock. Desktop Escape was tested; mobile back gestures were not. Three raw score measures, with their units and limits, remain separate. |
| Reset from review | [04 · Undo toasts](../../../../design-catalogue/ui-tips.html#undo-toasts) | Reset restores the traffic/score fixture and exposes Undo. No `confirm()` or custom confirmation popup. The running time is not reset; this resets the snapshot, not an actual shift. |
| Read score, time, waits, and changing states | [14 · Tabular numbers](../../../../design-catalogue/ui-tips.html#tabular-numbers), [15 · Balanced headers](../../../../design-catalogue/ui-tips.html#balanced-headers) | Tabular figures inherit across the entire document; score values use mono figures and fixed metric widths. All headings use `text-wrap: balance`. Values update directly, with no count-up interpolation. |
| Press controls and read surfaces | [16 · Continuous corners](../../../../design-catalogue/ui-tips.html#continuous-corners), [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop) | `corner-shape: squircle` only inside `@supports`; ordinary radii are the fallback. Controls have real 44 px boxes instead of overlapping pseudo-element hit areas. |
| Switch between the three studies | [12 · Hit slop](../../../../design-catalogue/ui-tips.html#hit-slop) | Footer links are at least 44 px. Separate files follow the requested deliverable format. No floating switcher obscures the command line. |

## Motion contract

The state spring is the handbook's sampled `linear()` response: mass 1, stiffness 1200, damping ratio 1, approximately 215 ms to its authored settling tolerance. Press feedback uses stiffness 4000, damping ratio 1, approximately 118 ms. Neither overshoots. CSS transitions reverse from their current computed value; they do not retain velocity. The live zoom spring uses the catalogue's `springStep` algorithm with stiffness 400, damping ratio 1, 1/240 s substeps, and tighter rest thresholds because zoom is a scale rather than pixels. Its target can change while running, preserving position and velocity.

Review and toast surfaces move only 8 px. Closing review during its entrance reverses the transition before closing the native dialog. No CSS keyframes or WAAPI animation is used. Under `prefers-reduced-motion: reduce`, CSS transitions are disabled and the camera snaps. A preference-change listener also finishes an already running zoom. Functional conflict and numeric updates are immediate in both modes. The spring constants are authored handbook starting points, not a claim about optimal human response times.

## What the current shell does worse

These are source comparisons against the supplied `web/shell.html`; **the production simulator was not run in this task**. Existing strengths—native dialogs, light/dark tokens, silhouettes, separate scores, and real routing—should be retained.

- **No typed command composer or autocomplete in the shell.** Its key dispatcher and per-strip verbs have no persistent grammar-aware entry surface. All three mockups provide one, with legal-next tokens and a visible Send/Clear path.
- **A fixed 320 px column carries unrelated jobs.** `main` allocates `1fr 320px`; wind, direction, landing/departure toggles, score, all four bays, incidents, and go-around history live inside that one scrolling column. The proposals put score in a stable header and give the bays a larger, explicit place in the workspace.
- **Context expands inside the working list.** The selected strip gains verbs, route hints, notes, and notices. A dedicated context region reduces the amount of list text the player must re-scan. Actual cognitive benefit needs playtesting.
- **The request pulse conflicts with this task's motion contract.** `.strip.req::before` uses an infinite `@keyframes req` animation. The proposals use static request words and wait order, with no looping pulse. This changes the earlier request-flashing choice recorded in the plan to comply with the present brief.
- **The bay chevron uses an ordinary `.15s` transition.** The mockups consistently use springs for animated state feedback. Their bays do not collapse; at the tested baseline all twelve strips are visible.
- **Several real controls remain 28 px tall.** `select` and `input.seed` are 28 px; result buttons inherit 28 px. Start/theme and verb buttons add hit slop, but that does not enlarge the select or seed input. The browser harness checks every rendered mockup control against 44 × 44 px; the user reported five consecutive external Chromium runs of the previous revision, as recorded below; the current visual fixes await a new run.
- **Red is also used for storage/boot/runtime failures.** `.banner` has a red tint/border and red headings, and `report()` uses it for non-conflict failures. Here ordinary validation remains neutral; red belongs only to the scripted conflict.
- **Score explanation is partly hover-only.** The meaning of caused delay is in a `title` tooltip, and “min” does not itself name game time. These mockups state `game min` in the header and print the exclusions and mockup limitations in review. They keep exactly three primary score measures; challenges would remain secondary context.
- **Strip keyboard access depends on custom keys.** Strip roots are `div`s and verb buttons use `tabIndex = -1`. The mockups expose native buttons and preserve normal Tab navigation outside the map/composer. Whether to retain map-focused Tab cycling in the production keyboard model should be decided through playtesting.
- **The title screen has an automation-only skip.** `navigator.webdriver` normally bypasses it. These mockups have the same initial state under human and automated browsing; their review dialog is exercised by actual clicks and keys.

## Verification

### User-run Chromium evidence before the latest visual fixes

The user ran both Python Playwright scripts outside this sandbox after the timing-polling edit and reported:

> verify.py 24/24 pass and verify_flows.py 3/3 pass, 5 runs in a row, zero console errors, zero external requests (see verification.json).

These are **5 runs, 24/24, 3/3**, supplied by the user, not Chromium runs performed in this sandbox. [verification.json](verification.json) contains the latest matrix report: Chromium 156.0.8078.79, 24 cases, zero console errors, zero external requests, and 48 screenshots. [flow-verification.json](flow-verification.json) records the three directions' extended flows with no page or console errors. One report is retained for each script; the five-run repetition is the user's report, not five separate stored reports. The timing flake is resolved according to these external runs.

The matrix covers 3 directions × 2 sizes (1500×844 and 1920×1080) × 2 themes × 2 motion preferences. It checks control sizes, strip clipping, aircraft target overlap, document overflow, balanced margins, contrast, command completion, preview/send identity, Undo, conflict hold, focus, zoom interruption, reduced motion, theme changes, and native dialog behaviour. Extended flows cover the additional clearance verbs, reset/Undo, focus-paused expiry, clock controls, and sibling-file navigation.

### Visual corrections after those runs

The user's 1920 px screenshot review found defects outside the previous assertions. The five-run results above **do not validate the following changes**:

| Change | Implementation | Status |
|---|---|---|
| Map controls cover B's 11R label | Move minus/Fit/plus into the map header in A, B, and C; keep them outside the clipped map stage. Runway-end labels are now visible DOM text using the same projection as the canvas and aircraft. | **fixed; Chromium 3 runs after the change, verify.py 24/24 and verify_flows.py 3/3 each, 2026-11-10; b and c 1920 screenshots viewed, overlap and blank band gone** |
| C has a blank band above Clearance and a gap below its strips | Remove the command panel's auto margin; let the ledger grow with aircraft assignments derived from existing strip notes, and distribute spare bay height across strip rows. The compact strip minimum stays 44 px. | **fixed; Chromium 3 runs after the change, verify.py 24/24 and verify_flows.py 3/3 each, 2026-11-10; b and c 1920 screenshots viewed, overlap and blank band gone** |
| Verifier misses controls/runway-label overlap | Measure all three `.map-controls` children against all eight visible `.runway-label` rectangles in each matrix case, with variant, labels, and rectangles in failure diagnostics; count assertions prevent an empty-selector false success. | **fixed; Chromium 3 runs after the change, verify.py 24/24 and verify_flows.py 3/3 each, 2026-11-10; b and c 1920 screenshots viewed, overlap and blank band gone** |

Source inspection and the existing A/C 1920 screenshots showed clearance between the old controls and 11R there; the shared header placement addresses the same risk in every direction, size, theme, and camera position. The three runs listed in the table passed with the new overlap assertion; the a-direction 1920 screenshot was not re-viewed after the change. All three HTML files have been regenerated from `build.py`. Obscura is used only for JavaScript smoke observations; it supplies no layout, clipping, touch, animation-timing, or screenshot proof. Existing reports and screenshots are preserved as evidence of the revision before these visual changes.

Run from `research/work/` in the browser-capable environment:

```bash
python3 ../out/ui-design/verify.py
python3 ../out/ui-design/verify_flows.py
```

Chromium still cannot start in this sandbox because Crashpad's `setsockopt` is denied. Earlier interrupted logs, [static-validation.json](static-validation.json), [verification-status.json](verification-status.json), and the original screenshot index remain historical artifacts; their earlier blocked status or file hashes do not supersede the later user-run reports or establish verification of the new files. No files were deleted.

### Review limits

- **Production simulator not run.** Rust/wasm integration, real routing, separation prediction, and actual aircraft motion remain untested here.
- **No real-device touch test.** Automated pointer checks do not establish touch usability.
- **Screenshots viewed by one reviewer at 1920 only:** the user's visual review covered `a-1920-dark-motion.png`, `b-1920-light-motion.png`, and `c-1920-dark-motion.png`. The assistant also inspected those supplied images to locate the defects; this is not an additional human review, a 1500 px visual review, or review of the regenerated files.
- Firefox, Safari, an 80-aircraft workload, and player performance have not been evaluated. The ranked recommendation remains A, C, B; automated checks do not establish which direction is easier to play.
