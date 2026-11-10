# Readbacks and hearback

AAI MATS Part 1 (Edition 7, 9 May 2025) is the best India-specific public authority found. Its §4.11.7.5.1 says pilots must read back safety-related parts of voice clearances and always include route clearances; instructions/clearances to enter, land on, take off from, hold short of, cross, taxi and backtrack on any runway; and runway-in-use, altimeter settings, SSR code, levels, headings, speeds and transition levels. §4.11.7.5.2 requires other clearances, including conditional ones, to be read back or acknowledged clearly. §4.11.7.5.3 requires controllers to listen and immediately correct discrepancies. This tracks ICAO Doc 4444 §4.5.7.5.

The required categories are: “ATC route clearances”; instructions to “enter, land on, take off from, hold short of, cross, taxi and backtrack on any runway”; plus runway-in-use, altimeter setting, SSR code, level, heading, speed and transition level. [AAI MATS Part 1, §4.11.7.5.1, 9 May 2025; ICAO Doc 4444 §4.5.7.5.1.]

## What the game should check

| Instruction | Required readback content |
|---|---|
| Start-up approval | Acknowledge start-up; repeat any assigned start time/condition. This is not a runway clearance. |
| Pushback | Confirm pushback and facing direction if specified. Do not infer runway entry permission. |
| Taxi | Repeat the route and clearance limit; identify any runway, hold-short point, crossing instruction and stand. In India, the AAI manual's runway taxi items are explicitly mandatory; include the full taxi route in the game for hearback checking. |
| Hold position / hold short | Say `HOLDING` or `HOLDING SHORT [of runway…]`, respectively. AAI MATS ch.12 says `ROGER` and `WILCO` are not sufficient acknowledgements for HOLD, HOLD POSITION or HOLD SHORT OF. |
| Cross runway | Repeat `CROSS RUNWAY {designator}` with the correct runway. Add `RUNWAY VACATED` only when requested/local procedure requires. |
| Line up and wait | Repeat runway designator and `LINE UP AND WAIT`; for a conditional clearance, repeat condition and subject sufficiently to make it unmistakable. |
| Take-off / cancel take-off | Repeat runway and take-off clearance; cancellation/stop requires immediate acknowledgement (`HOLDING` / `STOPPING` in the AAI/ICAO phrase set). |
| Landing | Repeat runway designator and `CLEARED TO LAND`. |
| Conditional instruction | Repeat the condition and clearance so controller can verify the same aircraft/traffic relationship was understood. |
| Contact Ground | Repeat unit and frequency in a frequency change readback; transmit separately from any instruction that needs a full readback. |
| Go around | Acknowledge briefly, then comply; the instruction should remain short because workload is high. |

Every readback ends with the aircraft's callsign. AAI MATS Table 12-8 says an airline telephony designator plus flight identification (such as `IFLY 512`) has no abbreviated form. Other call-sign forms can be abbreviated only after satisfactory contact and when no confusion is likely (§12.7.2.2). Correct an error directly: AAI/ICAO use `NEGATIVE` followed by the correct item, and the controller must hear the correction accepted.

## Errors documented by sources

| Error / hazard | What the source actually documents | Game treatment |
|---|---|---|
| Hold-short instruction not acknowledged correctly | AAI MATS ch.12 specifically prohibits relying on `ROGER`/`WILCO`; it prescribes `HOLDING SHORT`. It does not call omission the most frequent error. | Good scripted hazard: pilot says `Roger` or omits runway; require controller to catch it. Mark it as a documented hazard, not a statistically frequent event. |
| Wrong runway or incomplete runway readback | AAI MATS §4.11.7.5 requires runway designator for runway instructions. Its §3.20.13.4 lists incorrect instruction, failure to monitor readback, communication errors and non-standard phraseology as runway-incursion risk factors. AAIB's 2026 Goa runway-incursion final report reports a `hold at holding point` instruction was not fully read back / hearback was not ensured; this is outside VIDP but Indian practice. | Compare full runway designator and clearance verb, not just generic “copied”. This is safety-critical; frequency of wrong-runway readbacks is not quantified. |
| Similar flight numbers / callsigns | AAI MATS §15.6.1.3.1.4 explicitly says similar callsigns increase likelihood of the wrong aircraft accepting a clearance and make an unnoticed error more likely; §15.6.1.3.1.7 treats absent pilot readback as possibly blocked and says request repeat/confirm. | Strong candidate hazard. Use close flight-number pairs and require exact callsign in readback. The manual describes elevated probability, but gives no percentage/rate. |
| Readback differs from clearance | AAI MATS §4.11.7.5.3 and §12.9.5 require immediate correction. §12.9.3 says readbacks also check that the right aircraft and only that aircraft acts. | Let controller correct with `NEGATIVE, [correct instruction]`; require a fresh readback. |
| Omitted / garbled parts | AAI MATS §15.6.1.3.1.7 treats no readback as a blocked transmission; §15.6.1.3.1.4 notes blockage/garble can defeat error detection. | Possible game event, but don't label omission as statistically frequent; no public Delhi transcript corpus was found. |

AAIB’s final report on the 3 April 2026 Dabolim runway incursion directly documents the omitted hold-short/holding-point element. The crew read back “Taxi via N2 for RWY 08” after “Taxi via N2, hold at holding point RWY 08”; the report says the hold-at-holding-point element was missed and the incomplete readback was not acknowledged (AAIB Final Report VT-BXL, Synopsis and §§1.1, 1.18.5–1.18.6, 2026). This is an Indian example outside VIDP, not a frequency estimate.

A separate AAIB final report on a 2023 VIDP airprox includes actual Delhi ATC transcript for the radar unit and reports ground instructions and route sequences in narrative (§§1.1, 1.9, 2026). Its transcript shows the `IFLY` telephony designator and digit-by-digit flight numbers, but it is not a surface-control transcript. The public preliminary report on Ariana AFG311 at VIDP describes an unassigned-runway landing but does not publish a usable RT transcript. Therefore the gap is specifically Delhi surface frequency transcripts, not all Delhi ATC transcripts.

## Limits on “frequent”

The authoritative material found describes known error mechanisms, not measured frequencies. AAI MATS says similar callsigns increase probability; the specific hold-short acknowledgement is prescribed because of safety importance. Neither supports a claim that a particular error is the most common. The game may simulate these as plausible documented hazards but must label their event frequency as authored if it assigns a rate.
