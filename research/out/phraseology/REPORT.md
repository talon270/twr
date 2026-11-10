# Radiotelephony phraseology for the VIDP game

This research was completed 10 November 2026. It uses Airports Authority of India (AAI) MATS Part 1 Edition 7 dated 9 May 2025, the current eAIP India VIDP AD 2.18 available in AIRAC 2026-10, ICAO Doc 4444 readback rules cross-checked against AAI MATS, and UK CAA CAP 413 Edition 24 effective 1 July 2026 only for worked examples. Public AAIB material includes a 2026 report with Delhi ATC transcript from a 2023 VIDP event and a 2026 India report describing a missed holding-point readback at Goa.

## Findings that change the plan

1. **Replace the sample typed phrase with a full phraseology renderer and explicit runway limit.** `PLAN-twr.md` Part B, §Input proposes `IGO512 TAXI 29L VIA A D HOLD SHORT 28`. The phrase should render “IFLY five one two, taxi to holding point runway two niner left via taxiway Alpha, Delta, hold short of runway two eight.” AAI MATS ch.12 §12.3.4.7(e) gives taxi-to-holding-point wording, and §§12.3.4.8–9 define hold-short and crossing phrases. AAI MATS §12.9.4 requires the relevant runway instructions to be read back in full. See [grammar.md](grammar.md) for the typed form and each difference.
2. **Keep airline flight callsigns complete.** `PLAN-twr.md` Part B, §Input uses `IGO512`. The existing aircraft table identifies `IGO` as IndiGo's ICAO operator code and `IFLY` as its telephony designator. AAI MATS §12.7.2.2 and Table 12-8 say a telephony designator plus flight identification has “No abbreviated form”; use `IFLY 512` throughout. This contradicts a blanket rule to abbreviate once contact is established; only other callsign types can be abbreviated under the conditions in §12.7.2.2.
3. **Do not treat a taxi-to-runway clearance as crossing permission.** `PLAN-twr.md` Part B, §Input promises legal-next-word autocomplete. AAI MATS §12.3.4.9 has a separate CROSS RUNWAY clearance, and §12.9.4 requires readback of runway crossings. The parser should require an explicit `CROSS` command before any runway crossing and should distinguish `HOLD POSITION`, `HOLD AT`, and `HOLD SHORT`.
4. **Add a correct pilot readback beside every controller instruction.** This is requested in the brief and should be implemented in the Part B §Input radio log. AAI MATS §4.11.7.5 requires the controller to monitor and correct incorrect readbacks; its §12.3.4.8 note says `ROGER` / `WILCO` do not acknowledge HOLD / HOLD POSITION / HOLD SHORT OF. The typed command is not itself the spoken clearance or readback.
5. **Delhi Ground and Tower are unit labels with multiple channels.** `PLAN-twr.md` Part B §Input should let a radio transfer name the particular frequency. VIDP AD 2.18 in AIRAC 2026-10 lists eight Delhi Ground frequencies and six Delhi Tower frequencies, with assignments/remarks. A single fixed “Delhi Ground” frequency would be invented.

## Scope and phrase set

`phrases.csv` covers the verbs in the plan (`P`, `T`, `H`, `X`, `L`, `K`, `C`, `G`, `E`) and the requested start-up, pushback, runway hold/cross, taxi, give-way/follow, continue, conditional line-up, cancellation, stop, landing, go-around, vacate, frequency transfer and pilot reports. The source phrase is shown separately from the compact parser token. `primary` means AAI/ICAO/AAIB; CAP 413 examples are marked `secondary` because they are UK procedure, not India authority. `authored` is used for grammar abbreviations and game-only distinctions.

AAI MATS is unusually useful here: it is an India-specific, public, 2025 manual prepared for AAI ATS staff and incorporates ICAO Doc 4444 Amendment 12 (MATS preface). The manual warns against non-standard phraseology (ch.12 §12.1.1–12.1.2). The AAIB report on the 2023 VIDP airprox narrates pushback/start-up approval, progressive taxi routes, runway holding point and line-up workflow (§1.1) and reproduces an APAD transcript (§1.9). It records IndiGo callsigns in the spoken form `IFLY TWO ONE ONE THREE` and `IFLY TWO TWO ZERO SIX` (AAIB report §1.9, 2026). This is a primary Delhi example, although it concerns radar after departure rather than ground RT.

## Recommendation for the command surface

Treat `phrases.csv` as the canonical phrase template set. The game accepts the safe abbreviated tokens in `grammar.md`, but logs the full controller phrase and pilot readback. Parser keywords are authored affordances and are never presented as standard spoken phraseology. The minimum safe command prefixes in `grammar.md` avoid one-letter collisions (`C` could mean cross/contact/cancel; `T` taxi/take-off). Full phrase input is accepted. Autocomplete filters each next token by syntax and the selected aircraft's legal sim state.

For a later wrong-readback hazard, begin with a small set that AAI actually documents: missed hold/hold-short element, runway designator mismatch, no/blocked readback, and similar-call-sign confusion. Do not assign “most common” status or a probability from this literature: the manuals name hazards but provide no frequency estimate. AAIB's Goa runway-incursion report gives one specific missed holding-point readback; one event is not a population rate.

## Solid vs. assumed

| Solid | Assumed / authored |
|---|---|
| AAI MATS ch.12 phrase templates; mandatory readback categories in §4.11.7.5; special `HOLDING` / `HOLDING SHORT` acknowledgement; callsign abbreviation rules (AAI MATS, 2025). | Root command prefixes (`PUSH`, `CRO`, `TAK`, etc.) and the legal-next-word implementation. |
| VIDP unit call signs and channel list in AD 2.18, AIRAC 2026-10. | Which one of the listed Ground or Tower frequencies a game aircraft is on at a given moment, unless the game models published sectors. |
| IndiGo telephony designator `IFLY` in existing `out/aircraft/operators.csv`; AAI MATS Table 12-8 says telephony + flight ID has no abbreviated form. | Typed input may display a callsign compactly as `IFLY512`; log rendering uses `IFLY 512`. |
| AAIB reported a specific missed hold-at-holding-point element in a Goa event (Final Report VT-BXL, 2026). | The probability/rate of a simulated readback error; source material does not quantify it. |
| AAI MATS and ICAO specify phraseology and safety readback principles. | The example's named holding point is omitted because the plan supplies none; `holding point runway 29L` is a generic template instantiation. |

## Gaps

- ICAO Doc 4444 and Doc 9432 full text are not freely accessible from ICAO's public store. Doc 9432 is sold behind account/purchase access; its metadata confirms the 4th edition is from 2007. The accessible AAI MATS provides the India implementation and Doc 4444 readback content, while CAP 413 supplies secondary examples.
- DGCA's current FRTOL/RTR study materials were searched but no accessible DGCA-owned phraseology text suitable for validating local differences was located. AAI MATS is the usable Indian primary source.
- The AAIB material found includes a Delhi radar transcript and Delhi ground-control narrative, but no detailed public VIDP surface-control transcript that can validate exact local live wording. The 2025 AFG311 preliminary report does not supply such a transcript.
- No authoritative frequency table for the precise controller split a game should model was established beyond the AD 2.18 service list. The game can use the listed unit/frequency pairs, but should avoid implying each unit has only one channel.
- Wind/report-to-log wording is standard phraseology but operational detail such as when to repeat the surface wind depends on circumstances and local procedures. CAP 413 is UK guidance; use AAI/ICAO material if stricter Indian differences are identified later.

## Source register

See [sources.csv](sources.csv). Every phrase row names its source paragraph; the source register gives publisher, document date/AIRAC, grade and URL.
