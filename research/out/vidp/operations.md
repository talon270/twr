# VIDP operations — controller view

All current AIP references below mean AAI eAIP India AMDT 10/2026, effective 29 October 2026, unless another document is named. “Primary” is the regulator’s published information; “official” is the airport operator or government release.

## Runway configuration

AAI’s AD 2.20 I.1 noise-abatement runway-use plan varies by calendar day and time in IST. In easterly flow, arrivals and departures are assigned to runway pairs 09/10 and 11R/11L. In westerly flow, departures use 27/28 and 29L/29R; arrivals alternate between 27/28, 29L/29R, or both pairs. For example, dates 2–7 and 18–22 use 27/28 for arrivals 0000–0300, 29L/29R 0301–0600, and both from 0601–2359; the 10–15, 25–29 and 31 group reverses the first two slots. This is a schedule for two parallel runways at a time; AAI says separate procedures are needed if traffic requires all three pairs. ATC may depart from the plan for safety, regularity, efficiency or maintenance. The eAIP does not state that wind alone determines which flow applies [primary: AAI AD 2.20 I.1–6].

AD 2.3 restricts 09/27 to VVIP, state, defence and scheduled civil airline flights. The plan’s available runway picker should not treat that pair as open to every simulated movement [primary: AAI AD 2.3 note 3].

## Runway crossings and Eastern Cross Taxiway

DIAL says the dual elevated ECT connects the north and south airfields. DIAL reports a third-runway-to-T1 taxi-distance change from 9 km to 2 km; AERA describes a 2.4 km Code F elevated taxiway. These sources quantify a route-distance reduction, not runway crossings eliminated. No before/after crossing count was found [official: DIAL 2023 article; official: AERA Consultation Paper 07/2024–25, p. 60].

AAI’s current low-visibility route table shows the complexity the crossings create. For a westerly arrival on 29L to Terminal 1, one route vacates via Z2/Z3/Z6/Z7 then Z; an alternative vacates Z4/Z5, crosses runway 11L/29R via T6/R6 or T7/R7, crosses 10/28 via K6/H6, then continues via D. AAI gives further apron-specific routes and a route via D3 across 09/27 to the Technical Area for code E and smaller aircraft [primary: AD 2.22, section 4.4.4, LVP preferred taxi-route table]. These are charted LVP routes, not proof of ordinary daytime mandatory routes.

## Runway-holding distance

ICAO Annex 14 Vol I Table 3-2 sets 90 m from runway centreline for code number 3 or 4 precision CAT I and CAT II/III runway holding positions. The same annex says to increase the distance where radio-navigation protection or obstacle-free-zone clearance requires it. At VIDP elevation (~778 ft aerodrome elevation from AD 2.2), the general recommendation to increase the 90 m figure above 700 m elevation adds less than one metre when calculated as 1 m per 100 m above 700 m; that calculation does not substitute for chart placement. The standard distance measures laterally from runway centreline, while the game constant appears to measure along the aircraft’s route to a runway graph node. Since the chart hold coordinates and actual graph path were not verified, `HOLD_SHORT_M = 90` remains unvalidated for the sim [primary: ICAO Annex 14 Vol I 2022, §§3.12.6–3.12.9 and Table 3-2; AAI AD 2.2].

## Standard routes and hot spots

AAI publishes preferred exits by runway, aircraft code and exit location in AD 2.20; it also publishes CAT II/III LVP taxi routes. The AD 2.24 chart index lists an Aerodrome Chart and Hot Spot chart, both dated 14 May 2026. The hot-spot chart labels and explanations could not be read, so no hot spot location or cause is asserted. The preferred runway exits listed in AD 2.20 include D3/D2 for runway 09 by aircraft code, J/G exits for 10, D5/D6 for 27, H/K exits for 28, Y exits for 11R, and Z exits for 29L; AD 2.20 has the full per-code distance table [primary: AAI AD 2.20 and AD 2.24 index].

## ATS positions and frequencies

AD 2.18 lists Delivery, Tower and Surface Movement Control channels. These are frequencies, not a number of staffed positions. The AIP frequency remarks identify tower North/Middle/South sectors and SMC sectors; an AAIB report’s Fig. 14 explicitly lays out three tower sectors and four SMC sectors. That is evidence for sectors, not controllers on duty. The table lists all channels here so no implied one-frequency-per-position mapping is made [primary: AAI AD 2.18; primary: AAIB final report, Fig. 14].

| Service | Published frequencies (MHz) | Source |
|---|---|---|
| Clearance Delivery | 121.800, 121.850, 121.950 | Primary, AD 2.18, AMDT 10/2026 |
| Ground / SMC | 118.550, 119.075, 119.425, 119.575, 119.675, 121.625, 121.750, 121.900 | Primary, AD 2.18, AMDT 10/2026 |
| Tower | 118.100, 118.250, 118.750, 123.825, 124.375, 125.850 | Primary, AD 2.18, AMDT 10/2026 |

## Low-visibility procedures

AAI states that 10, 28, 11R, 29L, 11L and 29R are equipped for CAT II/III. Low-visibility take-off is also available on 09/27. Safeguarding starts when runway RVR is below 1,200 m or forecast visibility is 800 m or less, and/or ceiling is 400 ft forecast to fall to 200 ft or less. LVP starts when any TDZ/MID/END RVR is below 800 m and/or ceiling below 200 ft, after safeguarding is complete; CAT I continues until TDZ RVR is below 550 m [primary: AD 2.22, §§2.1, 3.1–3.2].

During CAT III, a Follow Me service is available on request to arriving and departing aircraft down to 50 m RVR. AAI requires simple one-way routes where possible, monitoring of taxiway light status, progress monitoring on ASMGCS, no stop within the localizer sensitive area, and vehicle restrictions below 550 m RVR. When LVP is declared, lead-in lights for CAT II/III-compatible stands are switched on; taxiway lights on unused taxiways may be switched off. The AIP does not in the sections read prescribe a general reduced movement rate or say “follow the greens” [primary: AD 2.22, §§4.3–4.4; AD 2.9].

The AIP notes a seasonal alternate restriction from 15 December to 15 February when forecast general visibility is below 550 m. This is a seasonal planning rule; no climate-derived fog frequency is assumed here [primary: AD 2.22, LVP introduction].

## Air Force Station Palam

AD 2.3 permits defence aircraft on 09/27, alongside VVIP, state and scheduled civil flights. AAI’s March 2026 NOTAM summary carried a temporary instruction for aircraft arriving from or departing the AFS Palam Technical Area to contact Palam SMC; the published validity ended 6 March 2026, so it is not evidence of a current instruction. No public movement-volume split for Palam/military traffic was found [primary: AD 2.3 note 3; primary: AAI Delhi NOTAM summary March 2026, NOTAM A3180/25].

## Movement rates

AAI AD 2.3 note 12 gives an approved peak capacity of 86 movements per hour; its maximum arrival rate is 42 per hour and maximum departure rate 51 per hour. The combined peak is stated directly as 86, so the maxima are not added. The plan’s 84/hour value is an older operator-reported figure and should be replaced with the current AIP value. No published capacity split by runway configuration was found [primary: AD 2.3 note 12; plan comparator: PLAN-twr.md § A2].
