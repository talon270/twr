# Strip clipping and zoom timing repair

Source review only for geometry. Obscura was used only to observe JavaScript state; no real layout, clipping, target-size, or frame-scheduling result is claimed. DESIGN.md and its numbers are unchanged. Nothing was deleted.

## QTR570: vertical budget

QTR570 is the third strip in the last bay. C stacks four `flex:none` bays inside `.bays`, a flex child with `min-height:0` and `overflow:auto`. At 1500×844, the rack has 664 px after the header, toolbar, and footer; its title consumes 32 px, leaving 632 px for bays. Twelve 44 px strips, four headings and borders, three bay gaps, and the old extra gaps between strip rows exceeded that budget. This accounts for the last strip falling below the scroll area's bottom. The reported clipping comes from the user's Chromium run; the ancestor geometry has not been measured here.

The fix retains the scroll container and every 44 px strip target. Strip top borders already separate rows, so `.bay-list` now has zero additional gap in all three directions. Bay headings have an explicit 14 px line height instead of font-dependent `normal`. C's compact headings use 2 px vertical padding and its bays have 6 px gaps. Its initial content budget is now `4 × (2 + 18 + 3 × 44) + 3 × 6 = 626 px`.

The following checks came from reading the shared CSS, variant overrides, and height media queries. Widths use the corresponding verification heights, 844 and 1080. **These are CSS arithmetic, not measured rectangles.**

| Variant / viewport | QTR570's bay and source height budget | Theme review |
|---|---|---|
| A / 1500×844 | Bottom-right bay. Two rows of bays use `2 × (2 + 34 + 3 × 57) + 10 = 424 px`. The rack spans both main grid rows; title and note remain outside the scroll area. | Light and dark use identical geometry. |
| A / 1920×1080 | Bottom-right bay. The tall-height override uses 94 px strips and 44 px headings: `2 × (2 + 44 + 3 × 94) + 10 = 666 px`. This is the tightest A layout and still needs the external rectangle assertion. | Light and dark use identical geometry. |
| B / 1500×844 | Rightmost of four side-by-side bays. Each bay uses `2 + 32 + 3 × 54 = 196 px`, equal to its 230 px rack row less the 34 px title. Removing the two inter-strip gaps removes the prior over-allocation. | Light and dark use identical geometry. |
| B / 1920×1080 | Rightmost bay uses `2 + 32 + 3 × 73 = 253 px`, within the 300 px rack row less the 34 px title. | Light and dark use identical geometry. |
| C / 1500×844 | Last stacked bay. All four use 626 px of the 632 px compact bay area. The rack note is hidden at this height. | Light and dark use identical geometry. |
| C / 1920×1080 | Last stacked bay. All four use `4 × (2 + 24 + 3 × 56) + 3 × 9 = 803 px`. The 900 px rack also contains its 38 px title and the wrapping note; the note is the remaining font-dependent height to inspect in Chromium. | Light and dark use identical geometry. |

This is not a long-field fix: QTR570's state uses `white-space:nowrap`, `overflow:hidden`, and ellipsis. Its callsign and type occupy the same two-column strip template as the other aircraft. The theme overrides change palette tokens, not widths, padding, fonts, or heights. The common gap change also gives A and B more space rather than moving the clipping risk to those variants.

## Spring timing

`zoom()` changes the target and requests a callback without setting `lastTime`. An idle `cameraTick()` therefore integrates with `dt = 0` on its first callback. `springStep()` returns false because the target is still 0.2 away, so another callback is queued. The second callback integrates elapsed time. This is not an internal stall: it continues requesting callbacks until settled. If the first or second callback is delayed, position can remain exactly 1 beyond the old 35 ms sample.

The animation code is unchanged apart from an explanatory comment. `verify.py` now polls every 10 ms, for at most 200 ms, for `1 < camera.position < 1.2`. Reduced motion retains its immediate-position assertion, and reversing the zoom plus the subsequent settling assertion remain in place.

The Obscura JS probe observed all three pages initialize twelve strips, select QTR570, apply a taxi command using the preview route, and restore Holding through Undo. Calling the actual `springStep()` with zero dt returned `false` and retained position 1; a following 16 ms integration produced position `1.010855483478813`. This probes integration only, not browser rAF scheduling. Raw observations are in `obscura-smoke.json` and `obscura-smoke.log`.

No Chromium verification, layout screenshot, or pass count was produced by this repair. The real browser run remains with the user.
