# Separation and runway rules — research report

Researched 2026-11-10. India values rest on AAI MATS Part 1, Fifth Edition (6 July 2020), and eAIP India AIRAC 10/2026 (effective 29 October 2026), especially ENR 1.1, ENR 1.6 and VIDP AD 2.12–2.22. ICAO Doc 4444 is compared at Amendment 9 (effective 5 November 2020); Doc 9870 at its 2007 first edition. The official AAI fifth-edition MATS PDF was available through AAI's public URL and its indexed text; earlier downloaded files in `sources/separation/` included a 2.6 KB failed-download stub, so that stub is not evidence.

## Findings that change the plan

**1. India has explicit local spacing at VIDP, and its independent-operation permissions are pair-specific.** `PLAN-twr.md` § B4's independent parallel-runway assumptions need to use the VIDP AD 2.22 procedures: independent arrivals on any two runways are prohibited and counted as one arrival stream; simultaneous departures are allowed across the Northern and Southern systems only under published SID/non-RNAV instructions, while same-system runway pairs are restricted. In particular, VIDP explicitly disallows simultaneous departures on 27/28, 09/10, 29R/29L and 11L/11R (AAI eAIP AIRAC 10/2026, AD 2.22 II.2(c–g)).

**2. Use India's MATS wake matrices, not a generic four-bin or RECAT-EU matrix.** § B4 currently gives wake values that disagree with AAI MATS. MATS Table 5-8 has A380-800→non-A380 Heavy 120 s; A380→Medium/Light 180 s; Heavy→Medium/Light 120 s; Medium→Light 120 s. Table 5-9 increases intermediate departures to 240 s for A380→M/L and 180 s for H→M/L and M→L. India names three mass categories plus A380-800 “SUPER”; code J is a game alias valid only for A388. No Indian adoption of RECAT-EU was found (MATS Part 1 5.9.1, 5.9.3.1–5.9.3.2, Tables 5-6, 5-8, 5-9, 6-Jul-2020; eAIP ENR 1.6 §7.6, AIRAC 10/2026).

**3. Runway-separation minima are event conditions, not an invented fixed buffer.** § B4 should retain event-based checks: a departure roll starts only when the preceding departure has crossed the runway end or started a turn, or preceding landing traffic is clear; an arrival crosses the threshold subject to the analogous condition. India's MATS uses “normally” and “reasonable assurance” at clearance issue, so the game should flag a violated event at the threshold/roll start and avoid warning when the situation will be safe at that event (MATS 7.9.2.1, 7.9.3.1, 7.10.1.1, 7.10.2.1, 6-Jul-2020).

**4. VIDP low-visibility rules add discrete gates; they do not publish a longer generic runway spacing number.** § B4's LVP row must model specific restrictions: LVP trigger at TDZ/MID/END RVR below 800 m and/or ceiling below 200 ft once safeguarding is complete; under CAT II/III, land clearance no later than 2 NM, and only after prior arrival vacates LSA and prior departure is airborne and over the localizer antenna; protect LSA from the arriving aircraft's 2 NM point through landing roll. Taxi via prescribed one-way LVP routes where possible, using route and A-SMGCS state. The protected-area geometry is not supplied as coordinates in the text and must be read from AD 2.24 charts (VIDP AD 2.22 VIII §§3.2, 4.2–4.4, AIRAC 10/2026).

**5. Neither ICAO nor AAI gives a taxiing-aircraft centreline-spacing minimum.** § B4's 300 m warning and 45 m incident threshold are game design values, not operational minima. A-SMGCS RMCA uses surveillance, protected areas, route geometry, relative speed and locally set parameters; it does not prescribe one universal taxi-taxi distance. Keep G1 explicitly `authored` and route-based to avoid false alerts from nearby parallel taxiways (MATS 7.11.1.2, 8.21.2.3.2; EUROCONTROL SPEC-171 §§3.3.5, Appendix B, Ed 2.0 2020).

## 1. Rule set

`rules.csv` is the code-facing rule table. Every source cell names the paragraph/table and dated edition. `wake.csv` expands leader/follower pairs. Important implementation boundary: time separations are measured on game time because § A2 compresses movement and rules together; they must not be divided by 3 a second time. Each MATS time minimum must be measured at the reference event represented by the game: lift-off to lift-off for departures, and threshold crossing to threshold crossing for arrivals. MATS notes vortex generation begins at nose-wheel lift-off/touchdown (5.9.4 note), while some surface-to-surface timing conventions depend on the scheme. The event is stated as a game implementation convention where MATS does not prescribe a clock event.

### Same-runway occupancy and runway crossing

For same-runway departure spacing, require the preceding departure to be past the runway end or begun a turn before the next roll begins; require all prior arrivals to be clear of the runway-holding position. For an arrival, apply the same departure-end/turn and arrival-vacated checks at threshold crossing. Clearance may be issued earlier only if reasonable assurance exists at the actual event. Clearance to land behind another landing aircraft cannot be issued before that aircraft has crossed the threshold (MATS 7.9.2.1, 7.9.3.1, 7.10.1.1, 7.10.2.1).

A runway crossing requires a specific clearance. There is no ICAO/India time or distance figure for crossing relative to final in the sources reviewed. In the game, protect runway occupancy and use the predicted threshold/roll event; do not declare a conflict solely because an aircraft is on final. Crossing in front of an aircraft on takeoff or landing roll is an incursion. (MATS 7.6.3.1, 16.3; Doc 4444 7.6.3; AAI eAIP ENR 1.1).

Conditional line-up phraseology is permitted only when the relevant traffic is identified and visible to the controller and pilot, with the condition stated before the clearance and read back. There is no maximum LUAW duration in the sources reviewed. Take-off after an immediate take-off clearance is a continuous movement; the game should model a long stationary line-up as a warning/state, not an automatic loss of separation. (MATS 7.9.3.5, 12.14.3 and 12.16.4 conditional-clearance material; VIDP AD 2.22 II.1.3.4–1.3.5).

### Reduced runway separation minima

ICAO Doc 4444 §7.11 permits reduced runway separation only after a documented safety assessment for each runway, publication in the AIP/local instructions, and controller training. Conditions are daylight from 30 minutes after sunrise to 30 minutes before sunset; wake minima still apply; visibility at least 5 km; ceiling at least 300 m; tailwind no more than 5 kt; suitable distance-assessment landmarks or approved surveillance; traffic information to the following crew; separation remains at minimum immediately after the second departure; and braking is not adversely affected by runway contamination. It is never used between a departing aircraft and a preceding landing aircraft. The minimum values are Cat 1 follower 600 m, Cat 2 follower 1,500 m, Cat 3 follower 2,400 m for the specified leader categories and events in `rules.csv` (Doc 4444 16th ed Amendment 9 §7.11.1–.7, 2020-11-05).

India’s MATS 5th edition has phraseology for “when reduced runway separation is used,” but no RRSM conditions or minima; current VIDP AD 2.22 contains no local RRSM procedure in the review. This is not evidence that no airport in India can ever receive approval. For this game, keep RRSM disabled unless a current India/VIDP runway-specific approval is sourced.

### Wake

India applies MATS three-category wake rules with A380-800 SUPER-specific rows. For implementation, map A388 to `J`; map other aircraft by maximum certified take-off mass: L ≤7,000 kg, M >7,000 and <136,000 kg, H ≥136,000 kg. Do not interpret J as any aircraft other than A388. The actual India matrices are in `wake.csv`, including same/interacting departures, intermediate departures, displaced-threshold arrival/departure pairs, opposite-direction low/missed approach, and distance minima for surveillance arrivals. MATS exempts wake separation for arriving VFR behind preceding H/M and for IFR visual approach when the follower reports the leader in sight and accepts own separation; issue wake caution. That exception does not waive runway occupancy or other separation (MATS 5.9.1–5.9.4, 5.9.6, Tables 5-6–5-10; eAIP ENR 1.6 §7.6).

India's AIP directs use of Doc 4444 wake rules and publishes the same distance matrix for surveillance approach/departure. It does not state that RECAT-EU is used. Keep one Indian matrix. RECAT-EU is documented as an alternative scheme and is not a second live game mode (eAIP ENR 1.6 §7.6; EUROCONTROL RECAT-EU Ed 2.0, 5-Dec-2024).

### Runway crossings and landing clearance

A crossing action is a state-authorized runway entry and can become an incursion if it enters the protected area incorrectly. The AIP/MATS specify clearances and holding positions, not a single crossing-to-final separation interval. The model should compute whether the runway remains occupied at the landing threshold, and whether the entering aircraft is ahead of a rolling aircraft. A landing clearance is allowed when runway separation will exist at threshold, but not until a preceding landing aircraft has crossed the threshold (MATS 7.10.2.1). In VIDP LVP, additionally apply the 2 NM deadline and LSA gates above.

### Parallel runways at VIDP

VIDP has four physical runways organized into Northern (09/27 and 10/28) and Southern (11L/29R and 11R/29L) systems. The AIP—not centreline distance alone—defines operating permissions. In dependent modes, movements on one runway count against the paired runway. Arrival streams on any two runways are dependent. Independent departures are allowed between systems only under published SID or non-RNAV instructions; same-system pairs listed above cannot depart simultaneously (VIDP AD 2.22 II.2(c–g), AIRAC 10/2026). Approximate centerline distances derived from the eAIP AD 2.12 runway endpoints are 438 m between 09 and 10 near the western thresholds, 381 m between 11L/29R and 11R/29L, and about 2.2 km between the northern and southern systems; these measurements are derived geometry, not published minima. Therefore code local pairing/mode permissions from AD 2.22, and use Doc 4444's 760 m criterion only for wake applicability, not to decide operational independence.

### Low visibility

LVP at VIDP applies when relevant RVR is below 800 m and/or ceiling below 200 ft after safeguarding is complete. When LVP is active, use the published low-visibility routes and one-way flows; give aircraft route-specific holding points and do not release a taxiway intersection beyond its clearance bar/stop bar/marking. Under CAT II/III, the arriving aircraft must be cleared by 2 NM or instructed to go around; prior arrival must have vacated LSA and its pilot confirmed this; prior departure must be airborne and over the localizer antenna; LSA stays clear from 2 NM until landing roll complete (MATS 7.11.1.1–.2; VIDP AD 2.22 VIII §§3.2, 4.2–4.4). No generic runway time increase is published. The protected LSA shape comes from AD 2.24 chart pages 43–50, not yet vectorized into game geometry.

### Ground spacing

MATS expects pilots to avoid collision while taxiing and controllers to provide concise routes, traffic information and control. The source review found no fixed taxi-taxi separation number. A-SMGCS alerts are configured around surveillance and route/area conflict logic; a single centre-distance radius would incorrectly alert aircraft on nearby parallel paths. Proposed game check remains the prior authored model: test centre-to-centre along common/converging cleared-route geometry, alert only when closing under 300 m, count a game ground incident under 45 m. Exclude parked aircraft and pairs both stopped; do not compare Euclidean position alone. This value is not an operational standard and must remain visibly tagged as `authored` in code and tests (MATS 7.11.1.2, 8.21.2.3.2; EUROCONTROL SPEC-171 Ed 2.0 §§3.3.5, App. B/C).

## 2. Incidents

See `incidents.md`. A runway incursion is the incorrect presence of an aircraft, vehicle or person on the protected runway area (Doc 9870; MATS 16.3.1). Class A–E severity is based on collision proximity, available reaction time and evidence. A pure wake or spacing breach is a loss of separation, not automatically a runway-incursion class. End Surge on the first confirmed runway incursion: the game promises never to miss a conflict, and an incursion is the highest-consequence category the controller directly creates; keep non-incursion losses in the incident count and end the shift under the plan's existing three-incident threshold.

## 3. Test scenarios

`test-cases.md` contains a boundary pair for each rule row and explicit cases for each listed exception. Distances/times are game-clock values. Cases at the minimum are safe; the test must assert no alert/incident there unless the rule is expressly strict (`<`), in which case equality passes. Each breach is one tick/metre beyond the stated minimum. This gives the B4 suite a specific oracle while preserving the distinction between live conflict alerts and incident counts.

## What a game simplification loses

| Real dependency | What the game cannot observe | Conservative, low-false-alarm stand-in |
|---|---|---|
| Visual separation / conditional clearance | Whether pilots and controller continuously see and identify the same traffic | Do not grant the visual-separation exception unless a scripted scenario sets `visual_contact=true` for both parties; otherwise apply full wake rule |
| Pilot responsibility on taxiways | Cockpit sight lines, braking, aircraft length/turn envelopes, phraseology/readback | Flag only predicted route conflicts on shared or converging graph paths, not close map coordinates; classify threshold as authored |
| Surface surveillance | SMR/MLAT/ADS-B track error, update delay, target association and sensor coverage | Use exact deterministic simulated tracks but do not model sensor confidence; do not flag when route geometry cannot establish a conflict |
| Controller judgement / reasonable assurance | Whether traffic will clear by roll/threshold time and whether a crew can comply | Predict occupancy at the event using modeled movement; a predicted safe clearance is not an incident |
| SIDs, altitude and wake transport | Published climb tracks, vertical differences, wind drift and wake decay | Apply wake when the AIP/MATS category/geometry test says so; where game lacks altitude/SID, assume the conservative triggering condition only for the explicit dependent runway pair, not all nearby runways |
| LSA shape and ILS signal effect | Chart-specific antenna protection geometry and signal distortion | Treat the AIP LSA as an authored polygon digitized from AD 2.24 before implementing LVP; until then do not claim geometric LSA coverage |
| Aircraft body and runway holding position | Tail beyond holding line, stopbar state, actual aircraft length | Use aircraft length plus full-body clear check; 90 m is the minimum lateral distance from runway centreline for the applicable Code 4 precision runway case; it is not a taxi-route setback and does not replace charted holding positions |

## Solid vs. assumed

| Solid (sourced) | Assumed or derived (must be tagged) |
|---|---|
| AAI wake values and category limits; VIDP's exact runway pairing restrictions; 2 NM LVP landing-clearance deadline; LSA/departure conditions; runway incursion definition and A–E classes | Taxi conflict at 300/45 m; 150 m intermediate-runway coding cutoff; straight-line projected departure path where no SID is represented; wake event timing where a publication does not specify the event; any LSA polygon not digitized from AD 2.24 |
| 760 m wake applicability criterion; no universal taxi-taxi minimum in sources reviewed | Derived runway spacing values from AIP coordinates; “first incursion ends Surge” recommendation; game tick integration and conflict prediction |

## Gaps

- **VIDP LSA polygons and holding positions:** AD 2.24 lists CAT III critical/sensitive-area and LVP routing charts, but their geometry has not been transcribed to game coordinates. Looked in the current AIRAC 10/2026 eAIP AD 2.24 list; exact chart polygons and holding-point positions require reading charts 43–50 and the aerodrome chart.
- **VIDP MATS Part 2 / locally approved RRSM and runway procedures beyond AD 2.22:** no separate publicly readable MATS Part 2 or RRSM approval was found. The MATS says Annex differences are listed in GEN 1.7, but the current GEN 1.7 page could not be retrieved; the similarly numbered ENR 1.7 is altimeter-setting procedure, not ICAO differences. Do not infer non-public local values.
- **Ground taxi alert thresholds:** MATS, AIP, and EUROCONTROL A-SMGCS specify responsibilities and configurable alerts, not one numeric taxi-taxi centreline minimum. 300 m/45 m remain authored game choices.
- **ICAO Doc 9643 detailed parallel operations:** official store catalogue metadata was accessible, but full text was not openly readable. Local runway independence is resolved by VIDP's own published procedures, not a Doc 9643 inference.
- **Runway holding position geometry:** Annex 14 Table 3-2’s 90 m is a minimum lateral offset from the runway centreline for the applicable precision Code 4 case, not an along-route setback from a runway-node; use AD 2.24 chart geometry and aircraft dimensions.
