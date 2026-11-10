# VIDP airfield research

Researched 10 November 2026. Aerodrome facts use AAI eAIP India AMDT 10/2026 (effective 29 October 2026); its chart index is current, but the Aerodrome Chart and Hot Spot chart themselves are dated 14 May 2026, and many apron charts retain 2025 dates. OSM counts use the supplied extract timestamp 2026-11-05T10:33:03Z. I read the eAIP HTML and chart index; the chart PDFs could be downloaded but could not be inspected in this session, so chart-to-element assignments are left explicitly unresolved.

## Findings that change the plan

**PLAN § A2 should use 86 movements/hour as VIDP’s published approved peak, not 84.** AAI AD 2.3 note 12 reports a peak handling capacity of 86 movements per hour, with maximum arrival and departure rates of 42 and 51 per hour. The two rate maxima are not additive. This is the regulator’s current published figure [primary: AAI AD 2.3, AMDT 10/2026, note 12].

**PLAN runway selection is incomplete if it treats wind as the only selector.** AAI AD 2.20 I.1 publishes a noise-abatement runway-use schedule keyed to day of month and local time. Easterly flow uses runways 09/10 and 11R/11L for both arrivals and departures; westerly flow assigns arrivals among 27/28 and 29L/29R by time slot while departures can use both pairs. It says ATC may use an alternate plan for safety, regularity and efficiency, and the schedule applies to two runways at a time; a separate procedure is required if demand calls for all three parallel runway pairs. The source does not say wind alone selects the flow [primary: AAI AD 2.20 I.1–6, AMDT 10/2026].

**The 90 m game constant is not a verified aircraft stopping offset.** ICAO Annex 14 Vol I, Table 3-2 gives a 90 m minimum from runway centreline for precision CAT I and CAT II/III runways with code number 3 or 4. That is a cross-field design distance, not distance along the taxi route from a hold node to a runway graph node. Annex 14 also says to increase distance where required to protect radio aids and sensitive areas. VIDP’s chart hold positions and their geometry were not read, so this does not validate `HOLD_SHORT_M = 90` in `crates/sim/src/aircraft.rs` [primary: ICAO Annex 14 Vol I, 2022 edition, §§3.12.6–3.12.9/Table 3-2, accessed 10 Nov 2026].

**The available evidence does not support inventing names for the 346 unnamed taxiway ways or 46 holding points.** The current eAIP index confirms the aerodrome and hotspot charts exist; however, no chart label was visually matched to an OSM ID. `overrides.proposed.toml` therefore has no live overrides. It contains all unmatched taxiway-way IDs and holding-node IDs with coordinates in its commented block. This does not unblock extraction; it preserves the review work without attaching a guessed clearance name [primary chart index: AAI VIDP AD 2.24, chart issue dates in AMDT 10/2026; community geometry: supplied OSM extract 2026-11-05].

**The chart/eAIP confirms four runway directions in the operational data, but 09/27 is restricted.** AD 2.3 note 3 permits use of 09/27 only by VVIP, state, defence and scheduled civil airline flights; AD 2.20 schedules use of the other parallel runway pairs by flow and time. No closure/works notice for the AIRAC effective date was identified in the material checked. [primary: AAI AD 2.3 note 3 and AD 2.20, AMDT 10/2026].

## `runways.csv`

`runways.csv` contains all eight runway directions, not four undirected strips. Bearings, physical dimensions, threshold coordinates/elevations, declared distances, approach category and lighting are transcribed from AAI AD 2.12–2.14. Latitude/longitude columns are decimal-degree conversions from the eAIP DMS coordinates. The four entries 11L/29R and 11R/29L distinguish physical runway length from displaced-threshold TORA/LDA. RET names come from AD 2.9 and the preferred exit table in AD 2.20; no values were inferred from OSM.

The current AIP shows CAT III approach facilities on 10, 28, 11L, 29R, 11R and 29L; 09 and 27 are CAT I. AAI AD 2.22 also identifies 09/27 as available for low-visibility take-off. The 09/27 restriction above remains in force. This is AIRAC-current published data, not a live NOTAM check.

## `taxiways.csv`

The file lists the 110 distinct taxiway references already present in the OSM ways. By designator-text match, 102 have width and maximum code-letter values transcribed from AAI AD 2.23; repeated segments retain all distinct widths, and the code-letter field records the largest stated letter. Eight OSM refs did not match the eAIP table text. `connects`, one-way restrictions and elevated status remain unfilled unless the source states them unambiguously. AD 2.23 often gives different restrictions on portions of one named taxiway, so the CSV retains a source entry number and does not pretend those portions are one uniform segment.

AAI lists the dual Eastern Cross Taxiway as a 2.4 km Code F project in the AERA filing (p. 60); DIAL says it links the northern and southern airfields and reduced a third-runway-to-T1 taxi route from 9 km to 2 km. The chart/AD 2.23 entries needed to identify its exact taxiway designator(s) and linked taxiways were not verified, so no ECT name or graph link is asserted in `taxiways.csv` [official: DIAL article, 2023; official: AERA consultation 07/2024–25, p. 60].

## `holds.csv` and `overrides.proposed.toml`

OSM has 46 `holding_position` nodes and none carries `ref`; the extractor therefore has zero named holding points before overrides. `holds.csv` preserves each node ID and coordinate. The apparent nearest runway is geometry-only and is explicitly not a chart-confirmed protection assignment. No node receives a category or runway-protection label without chart evidence.

The OSM baseline is 245 named taxiway ways out of 591 (41.5%); the proposed file names 245 of 591 because no chart-backed mapping could be verified. Holding points remain 0 of 46 named. The unmatched block includes all 346 unnamed taxiway ways and all 46 holding nodes with coordinates, each commented and excluded from TOML live tables. The 9 OSM `parking_position` ways are not included in the 591 taxiway-way baseline.

## `stands.csv`

The file contains the 115 OSM parking-position nodes; 106 have an OSM `ref`, while nine do not. Stand number, code letter, apron/terminal allocation, gate pairing, power and pushback are not filled from OSM because the eAIP parking charts were not read. Three additional source-only rows preserve AAI AD 2.8’s explicit data for special stands 801–803: all remote, power-in pushback, code E for 801 and F for 802/803. Their chart coordinates and graph mapping remain unjoined.

AAI AD 2.9 distinguishes stand guidance: continuous lead-in lines on MARS centre stands and single-aircraft stands, broken lead-in lines on left/right MARS stands for narrowbodies, nose-wheel stop lines, and AVDGS on a named list of T3 and T1 stands. These are operational stand facts; they do not map passenger gates to stands. No gate-to-stand mapping was found in the sources read.

## Sources and scope

`sources.csv` records publisher, URL, document date/cycle, source grade and intended use. The source index’s chart dates matter: ADC and hotspot chart 14 May 2026; Apron II 19 March 2026; most apron charts 12 June 2025. No current closure/NOTAM summary was checked, and the AAI GEN 1.7 differences table was not checked for marking/sign differences.

## Solid vs. assumed

| Solid (sourced) | Assumed or unresolved |
|---|---|
| Runway physical data and declared distances: primary, AAI AD 2.12–2.14, AMDT 10/2026. | Decimal coordinates are arithmetic conversions from the AIP DMS values. |
| 86 movements/hour peak; 42 ARR/hour and 51 DEP/hour maxima: primary, AAI AD 2.3 note 12, AMDT 10/2026. | Whether the game’s shift rate should start at the AIP peak is a design choice, not an AIP rule. |
| Calendar/time runway-use schedule: primary, AAI AD 2.20 I.1, AMDT 10/2026. | Wind as the sole flow-selection rule is unsupported by this source. |
| OSM counts and IDs: community, supplied extract timestamp 2026-11-05. | The nearest runway column in `holds.csv` is only a geometric nearest-way calculation, not the protected runway. |
| 90 m minimum centreline-to-holding position for code 3/4 precision runways: primary, ICAO Annex 14 Vol I 2022, Table 3-2. | Treating that cross-field minimum as a 90 m along-network game trigger is not validated. |
| AAI documents enhanced taxiway centreline marking and lists stop-bar locations: primary, AD 2.9, AMDT 10/2026. | Exact mark geometry, complete chart labels, and per-element OSM assignments remain unverified. |
| DIAL ECT route-distance statement: official DIAL source (2023). | Before/after runway crossing counts and exact taxiway links: not found. |

## Gaps

- **Chart-to-OSM joins:** no chart-derived taxiway-way names, holding labels/protected runways/categories, or complete stand chart mapping. Looked at AAI VIDP AD 2.24 chart index and downloaded ADC, hotspot, taxiway-composite, apron and LVP chart PDFs; chart PDF visual reading was unavailable in this session. This is the main blocker.
- **Current closure/work status:** AMDT 10/2026 text and chart index checked; a current VIDP NOTAM summary and current AIP SUP list were not checked. The source index confirms all four runway directions in published data but is not proof against a day-of-operation closure.
- **Runway-holding geometry:** ICAO Table 3-2 checked; VIDP chart coordinates/distances to runway centreline, ILS sensitive-area holds, and `HOLD_SHORT_M` along-network validation not established.
- **ICAO/India marking differences:** AAI GEN 1.7 was not read. Annex 14 exact dimensions for all renderer elements need a full figure-by-figure extraction before treating this marking note as a build specification.
- **Hot spots:** AAI publishes a VIDP Hot Spot chart dated 14 May 2026, but its IDs, locations and explanations were not read.
- **ECT crossings:** DIAL gives taxi distance reduced 9 km to 2 km; no official before/after count of runway crossings was found in the sources checked.
- **Stand/gate pairs, taxiway one-way restrictions, standard day routes, Air Force movement volume, controller staffing, and hourly capacity by configuration:** not established from the sources read. AAI AD 2.18 lists frequencies and sector remarks, not controllers rostered per shift.
