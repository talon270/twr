# twr controls with depth — plan

Written 2026-11-15, against `web/shell.html` at commit `76f21df` (backup `web/shell.backup-20261115-101721.html`).
Method: read your 18 probe picks (`~/Downloads/twr-ui-preferences-v1.json`, recorded in `research/out/ui-pref/NOTES.md § Answers`), `~/.claude/rules/ui.md`, `md/designs.md § Cream neobrutalist`, and the control, bay, strip and dialog rules in `web/shell.html` (lines 30-330). Nothing was run for this plan.
**Nothing below is implemented — this is the plan.** (Header kept as written. Status 2026-11-15: approved and built; see "Built" at the end.)

**Goal.** Give twr's controls the depth you picked (p05, p06, p10, p13) without breaking the console's job: reading 40 strips at Peak and acting in under a second.

## Part A — findings, ranked

### A1 · DESIGN RISK (high): the hard-shadow look has no dark variant, and you picked it on dark

`md/designs.md:19` says the cream neobrutalist family is "Cream in every theme, by choice. No dark variant." Your probe picked Dark over Paper (p04) and the 5 px hard shadow (p05, p06), all in the dark theme. twr is dark by default. So the shadow colour on black is a new decision, not a copied token: a black `5px 5px 0` shadow is invisible on `#000`.

**Fix:** the shadow is ink, so it inverts with the theme: `--sh: 5px 5px 0 var(--fg)` (white on dark, black on light). This matches twr's own rule that press and selection invert. **This is a guess at what you meant;** the probe showed one dark specimen, not white-shadow against black-shadow.

### A2 · DESIGN RISK (medium): depth everywhere would bury the alarm

twr's one loud signal is the red bar on a strip in conflict (`.strip.alarm::after`, line 197) and the Needs-you card. 40 strips each with a 3 px border and a 5 px white shadow would out-shout both.

**Fix:** depth goes on **controls only**: `button`, `select`, `input.seed`, `.seg button`, and the two dialogs. Strips stay flat rows; the bay box takes the 3 px border (p10, "panel boundary") but no shadow. Smallest, because it applies the picks to what they were asked about: p05 and p06 asked about "the control", p10 about "the panel boundary".

### A3 · INCONSISTENCY (medium): two picks contradict a standing rule

| Pick | Rule it meets | Proposal |
|---|---|---|
| p16 Normal wrapping | `ui.md § Balanced headings`: `text-wrap: balance` on h1-h3 | Keep balance. The result title is one line, so the two render the same; the rule wins (`ui.md` "Rules beat the page") |
| p02 32 px rows | `ui.md § Hit slop`: 44 px minimum target. Strips are click targets | Keep 44 px. Read 32 px as "keep rows tight", which they already are (10 px padding) |

### A4 · COSMETIC (low): uppercase headings

p09 picked mixed case; the result title and title-screen heading are uppercase (line 311). The 11 px uppercase labels on buttons and bays stay: they are labels, not headings, and `verify_ui3` checks "no mixed-case text under 12 px".

**Fix:** drop `text-transform: uppercase` on `dialog.result h2` and the title-screen heading only.

## Part B — the build

1. **Tokens.** Add `--bw: 3px` and `--sh: 5px 5px 0 var(--fg)` to `:root`, so both themes get them from `--fg`.
2. **Controls.** `button`, `select`, `input.seed`, `.seg button`: `border: var(--bw) solid var(--fg); box-shadow: var(--sh)`. Height stays 44 px.
3. **Press travel (p13).** `:active` moves the control `translate(5px, 5px)` and drops the shadow to `0 0 0`, on the existing press spring (`--spring-press`, 118 ms). Fill still inverts. Under reduced motion: no travel, the shadow drops at once.
4. **Disabled.** End shift is disabled in Practice: no shadow, dim border, so a flat control reads as "not pressable".
5. **Panels.** `.baybox` and both dialogs: 3 px border. Dialogs also get `--sh`. Strips, the strip rack and the score bar keep their hairlines.
6. **Headings.** Mixed case on the two dialog headings (A4).
7. **Tests.** Update `verify_ui3` only where it asserts a 1 px bay border; add checks: a control's computed `box-shadow` is `5px 5px 0` in `--fg` in both themes, `:active` moves it 5 px, and reduced motion moves it 0 px.
8. **Build and run.** Backup `web/shell.html`, `./build.sh`, then every `verify_*.py` once, in sequence. Screenshots at 1500 and 1920, both themes, Practice and a Peak shift with 40 strips, for your review.

**Handbook picks.** Look: `designs.md § Cream neobrutalist` control (3 px border, 5 px hard shadow, press travel), adapted to dark by A1. Motion: `animations.html` press spring, already in the page. Interaction: `ui-tips.html` hit slop (44 px kept).

## Out of scope

- **p01 width (86%).** The map fills the window; there is no page column to widen.
- **p08 mono headings.** Not checked against the page yet; the next plan if you want it.
- **A paper (cream) theme for twr.** You picked Dark over Paper (p04).
- **Strips with shadows.** A2.
- **Commit and push.** Each needs your yes.

## Questions that change the work

1. **Shadow colour on dark (A1):** white, as proposed, or a mid grey (for example `#6a6a70`, the hairline) so the map stays the brightest thing?
2. **Scope (A2):** controls and dialogs only, as proposed, or the bay boxes' shadow too?

## Built 2026-11-15

**Approved 2026-11-15 with white ink on dark (question 1) and controls and dialogs only (question 2).** Backup: `web/shell.backup-20261115-121818.html`. Not committed.

**What changed in `web/shell.html`.** Tokens `--bw: 3px`, `--sh: 5px 5px 0 var(--fg)`, `--sh-0`. Every `button`, `select`, `input.seed`, the radio field and both dialogs take the 3 px ink border and the hard shadow. A press travels 5 px and drops the shadow on the press spring (118 ms). Under reduced motion the control does not move. Disabled End shift is flat with a hairline border and dim text. Bay boxes take a 3 px border in `--hair`, with no shadow. Bay headers and strips stay flat. The result title is mixed case, and `text-wrap: balance` stays (A3).

**Three changes the plan did not list.**

| Change | Why |
|---|---|
| Pressed runway toggles sit flat, and the toggle gap is 10 px, not 6 | Seen in the first screenshots: a 5 px ink shadow in a 6 px gap merged two pressed toggles into one block (DEPART 29R and 27 in light) |
| Conflict-list buttons take a red shadow, matching their red border | An ink shadow under a red button read as a second, unrelated colour |
| `.bayhead:active` has no travel | The global `button:active` rule would have moved the bay headers, which are buttons drawn as headers |

**Two plan steps came out smaller than written.** Step 6: the title-screen heading is the lowercase brand `twr`, not uppercase, so only the result title changed. Step 7: `verify_ui3` needed no edit, because the bay border keeps its `--hair` colour and `ui3` checks that colour's contrast, not its width. A new script, `tools/verify_depth.py`, holds the new checks.

**Checked 2026-11-15.**

| Check | Result |
|---|---|
| `verify_depth.py` against the committed page before the change (`76f21df:docs/index.html`) | **19/57**: only the checks for what stays flat pass |
| `verify_depth.py` after | **57/57** |
| All 22 `verify_*.py`, once each, in sequence | **835 PASS, 0 FAIL**, every exit 0; `verify_ui4` 17/17 on its first run |
| `./build.sh` | `dist/index.html` 1,033,349 bytes, byte-identical to `docs/index.html` |

Screenshots (scratchpad, not the project): Practice at 1500 and 1920 in both themes, a Peak shift at 40 aircraft, and its result dialog. Checked by eye: the toggle merge above, found and fixed; nothing else.

**Not done.** `perf_80.py` was not run; a shadow on every control could cost paint time at 80 aircraft, and that has not been measured. The README does not describe the look, so it needs no change. Commit and push each need your yes.
