# KLM estimate: one full departure clearance

## Scope and operator values

Card, Moran and Newell's Keystroke-Level Model estimates error-free routine task time for an expert who follows a specified method. It does not predict user errors, learning time, or discoverability. In their 10-system validation, the RMS prediction error was about 21% for individual task-system predictions (Card, Moran & Newell, “The Keystroke-Level Model for User Performance Time with Interactive Systems,” *Communications of the ACM* 23(7), July 1980, pp. 396–410, §4.2.2 and abstract, https://iihm.imag.fr/blanch/ens/2010-2011/M1/EIHM/cours/1980-Card-KLM.pdf).

| Operator | Value | Source and meaning |
|---|---:|---|
| `K` | 0.20 s | Average skilled typist at 55 wpm; each key/button press. Card et al. 1980, Fig. 1, p. 398. |
| `P` | 1.10 s | Average mouse point to target. Their Fitts-derived estimate spans 0.8–1.5 s; click itself is a separate `K`. Card et al. 1980, §3.1/Fig. 1, p. 398. |
| `H` | 0.40 s | Move hand between keyboard and mouse. Card et al. 1980, Fig. 1, p. 398. |
| `M` | 1.35 s | Mental preparation operator. Card et al. 1980, §3.1/Fig. 1, pp. 398–399. |
| `R` | 0 s here | The plan's local click/key responses are assumed immediate and do not make the user wait; KLM counts response only when waiting occurs. |

## Compared task

Task: select an aircraft on stand; push it; route to runway via two named taxiways with a hold-short; line up; take off. The plan does not yet specify a single typed grammar for every verb, and the implementation later added a combined `S` push-and-taxi interaction and a tail picker. These are lower-bound structured estimates, not measured performance. For comparability, each path includes selecting one aircraft and an authored tail choice. Path A follows the plan's route builder; Path B uses key verbs with map route picking; Path C types one complete compact clearance. KLM method/operator counts below are authored assumptions, shown explicitly so they can be changed.

## Operator sequences and totals

| Path | Operator sequence | Calculation | Estimated active input time |
|---|---|---:|---:|
| A. Mouse verb buttons + click route | `M P K` select; `M P K` push button; `M P K` choose tail; `M P K` taxi mode; `M [P K]×4` destination, via A, via D, hold-short point; `K` send; `M P K` line-up button; `M P K` takeoff button | 7M + 10P + 11K = 9.45 + 11.00 + 2.20 | **22.65 s** |
| B. Single-key verbs + click route | `M P K` select; `M K` push; `M K` taxi; `M [P K]×4` destination, via A, via D, hold-short point; `K` send; `M K` line-up; `M K` takeoff | 6M + 5P + 10K = 8.10 + 5.50 + 2.00 | **15.60 s** |
| C. Typed clearance | `P K` focus command line; `M`; type the hypothetical 58-character phrase `IGO512 PUSH TAXI 29R VIA A D HOLD SHORT H4 LINE UP TAKEOFF` at `K`; `K` submit; three more `M` operators for callsign, route/hold-point choices and commit check | 4M + 1P + 60K = 5.40 + 1.10 + 12.00 | **18.50 s** |

**These totals are not a ranking of real player speed.** The hypothetical typed line above is 58 characters including spaces; a different phraseology grammar changes it. Route-building point counts also change with the number of selected vias and clicks. KLM does not count the time to visually locate targets beyond its average `P`, nor the player's map comprehension, the route-preview inspection, errors, or corrections. Add these in an empirical test rather than hiding them inside an unexplained multiplier.

## What the comparison says

The numbers do not establish that mouse routing is slower than command entry or that the one-key path is universally superior. They show that KLM estimates are highly sensitive to how many decisions and pointing actions a method requires. An expert who knows a command string can issue it with fewer pointer changes; a novice has to learn the grammar. The typed phrase is a hypothetical compact grammar, not one specified in the plan; adding one character adds 0.20 s at the chosen `K` rate. The study “Comparing Text-based and Graphic User Interfaces for Novice and Expert Users” reported in its small sample that novice GUI users performed tasks faster and with fewer errors after a brief introduction, while expert GUI/TUI time differences were not statistically significant (Yen et al., 2008, https://pmc.ncbi.nlm.nih.gov/articles/PMC2655855/). That study was not aviation, not under time pressure, and does not resolve this game's design.

For Fitts's law, use the game's hit-tested aircraft shape and actual view size in a future user test. The current 22 px hit-slop outside the silhouette is an authored target rule, not an estimate derivable from Fitts's law. WCAG 2.2 SC 2.5.8 calls for 24×24 CSS px minimum pointer targets with spacing/equivalent-control exceptions; SC 2.5.5 gives 44×44 px enhanced target size (W3C, 12 Dec 2024, https://www.w3.org/TR/WCAG22/#target-size-minimum).

## Suggested validation

Measure completion time and wrong-command/selection rate for the same seeded task with experienced and first-session players. Counterbalance input order. Record route deviations, reissued commands, and whether the player noticed an incorrect preview; compare median and tail times rather than only a mean. This would test the plan’s stated critical invariant, which KLM cannot address.
