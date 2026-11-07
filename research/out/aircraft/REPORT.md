# Aircraft types and traffic at Delhi IGI — research report

Researched 2026-11-07, extending the leftovers of two earlier runs that died at
the usage cap (their downloaded manufacturer documents, FAA database extract and
A20N trace set-up were reused; their logs are empty). The findings rest on Airbus
Aircraft Characteristics documents dated Jul 2026 (A320, A321) and Dec 2025 (A330,
A380), Boeing airport-planning documents Rev G Dec 2024 (777) to Rev Q Oct 2025
(787), the FAA Aircraft Characteristics Database of Oct 2024, EUROCONTROL RECAT-EU
Edition 2.0, FAA Order JO 7340.2P for telephony, and GMR Airports' traffic filings
for August and September 2026. No AIRAC-dated document was needed for this topic.

## Findings that change the plan

**1. The A380 now flies to Delhi daily, so wake category J is in the mix.**
Emirates moved EK512/513 to the A380 from 25 Oct 2026, one of four daily DXB–DEL
rotations (Emirates media centre, 9 Jul 2026, `emirates-a380-del`). That is about 2
movements a day, 0.15% of the passenger total, and the only code F type at DEL
(span 79.75 m, Airbus). `PLAN-twr.md` § B6 "Operators, types, wake" says "No A380
(J) is in the mix: none is confirmed for DEL"; that sentence is now wrong, and the
J → H/M/L spacing row it marks unused is used. Limit: the Emirates page was read
through a search summary, not opened, and whether the swap actually ran from 25
October was not confirmed by a schedule.

**2. Heavies are about 11% of passenger movements, not 20%.** GMR's H1 FY27 split
is ~179K domestic and ~55K international ATMs, 77:23, against the plan's authored
80:20 (§ B6 "What is sourced"). The plan's operator table then gives foreign
carriers 30% of all movements; with the sourced split and an even Indian/foreign
share of international flights (authored), foreign carriers are about 12%. Every
widebody at DEL is H except the A380, so the H share falls to ~11%
(`movements.md` § Fleet mix). The estimate is a model with four authored inputs, so
the 11% is soft; the direction is not, because 23% international caps the
widebody share well under 20% unless Indian domestic flights used widebodies.

**3. `MIN_TAKEOFF_RUN_M` (1,800 m) is short for a full-weight A320neo.** Airbus's
take-off chart (FIGURE-3-3-1-991-009-A01) gives about 1,960 m at 79,000 kg, ISA,
sea level, and about 2,030 m at Delhi's elevation; EUROCONTROL's A20N page gives
1,951 m. A 777-300ER at MTOW needs about 3,050 m (Boeing 3.3.9). The constant
should become per type (`types.csv` `takeoff_distance_m_at_mtow_isa_sl`), which
affects `aircraft.rs` and the line-up rule in § B3. Chart values are read by eye,
±100 m.

**4. The ~1,275 a day in § A2 is confirmed, and it counts passenger flights
only.** H1 FY27: 233,331 ATMs in 183 days (GMR, footnote "ATMs are Pax ATMs").
Cargo, GA and Air Force Palam movements are on top and were not found. Runway
11R/29L was closed for about seven months until 20 Sep 2026, so the H1 figure was
flown mostly on three runways.

**5. Telephony is now sourced for 89 of 90 operators** (§ B6 "Callsigns", which
waits on this). From FAA JO 7340.2P: IndiGo `IGO` IFLY, Air India `AIC` AIRINDIA,
Air India Express `AXB` EXPRESS INDIA, Akasa `AKJ` AKASA AIR, SpiceJet `SEJ`
SPICEJET, Alliance Air `LLR` ALLIED, Blue Dart `BDA` BLUE DART. Several are not
what you would guess: British Airways SPEEDBIRD, flydubai SKY DUBAI, Qatar QATARI,
Air France AIRFRANS, AirAsia X XANADU, Batik Air Malaysia MALINDO, Thai AirAsia X
EXPRESS WING, Nepal Airlines ROYAL NEPAL, Drukair ROYAL BHUTAN, Ural SVERDLOVSK AIR,
Quikjet QUIK LIFT, Indian Air Force `IFC` INDIAN AIRFORCE. Centrum Air (Uzbekistan)
has no entry under that name.

**6. Stand sizing (§ B6 A14) now has its aircraft half.** Every row of `types.csv`
carries an ICAO code letter from its span (UK MAA RA 3510 Table 1, which reprints
Annex 14 Table 1-1): ATR 72 C, A320 family and 737 C, 757 D, 787/777/A350/A330 E,
A380 and 747-8 F. **The 777-300ER is code E by 0.20 m** (64.80 m against the 65 m
F boundary). The stand half (which stand takes which letter) is the `vidp` run's.

**7. Two authored constants already match the A320neo.** `CLIMB_MDM_PER_TICK`
(90 m/s) equals EUROCONTROL's A20N initial climb, 175 kt; `LAND_MDM_PER_TICK`
(136 kt) sits between EUROCONTROL's Vat 135 kt and FAA's 137 kt. They stay right
for the A320 class and wrong for everything else (`sim-constants.md`).

## Part 1 — which types

**Method.** No source gives DEL movements by airline or type, so every rank used
the second method the brief allows: fleet counts of DEL operators, weighted by
sourced shares. `research/work/aircraft/estimate_mix.js` (inputs labelled in its
header) splits 1,275 movements 77:23 domestic:international (GMR); domestic by
national DGCA passenger shares for September 2026 (IndiGo 65.4, Air India group
26.8, Akasa 5.6, SpiceJet 0.6, Alliance 0.4); each operator's share by its
Wikipedia in-service fleet; international half Indian, half foreign (authored),
foreign carriers one rotation a day unless sourced (Emirates 4), each with an
authored main type. 72 passenger and 18 cargo operators come from Wikipedia's IGI
table (community grade, enumeration only).

**Result: 13 types above 0.5%, covering 98.3% of estimated passenger movements.**

| Rank | Type | Est. share | Rank method |
|---|---|---|---|
| 1 | A20N | 34.1% | fleet × DGCA share |
| 2 | A21N | 27.0% | fleet × DGCA share |
| 3 | B38M | 10.9% | fleet × DGCA share |
| 4 | AT76 | 5.6% | fleet × DGCA share; probably high (ATRs fly regional sectors from other bases) |
| 5 | A320 | 5.0% | fleet × DGCA share |
| 6 | B738 | 3.3% | fleet × DGCA share |
| 7 | B788 | 3.0% | Air India fleet (authored intl split) + authored foreign types |
| 8 | B77W | 2.4% | as above; Emirates 3 daily sourced |
| 9 | B789 | 2.0% | as above |
| 10 | A321 | 1.6% | fleet × DGCA share |
| 11 | A333 | 1.5% | authored foreign types |
| 12 | A359 | 1.2% | Air India fleet + authored foreign types |
| 13 | A319 | 0.8% | fleet × DGCA share |

Light rows (dimensions and wake only), 22 more: A343, DH8D, B737, A388, B748,
A332, A339, E190, B78X, AT46, B739, A35K, the cargo types B752, B77L (777F), B744,
B763, and Air Force types C17, C30J, IL76, E35L, D228, AN32. 35 rows in all.

## Part 2 — `types.csv`

**Base layer FAA, manufacturer over it.** Every dimension, weight, ICAO wake
category, ADG and approach category starts from the FAA database (secondary); a
manufacturer value replaces it where read, and any difference over 0.1 m or 10 kg
goes to `discrepancies.csv` with both sources.

**Manufacturer cross-check, by `dimcheck.js` and by reading the Boeing sheet:**

| Type | Checked against the manufacturer | Result |
|---|---|---|
| A20N, A320, A21N, A321 | span, length, wheelbase, outer main gear span | all within 0.02 m of FAA |
| A359, A35K | span, length, wheelbase, outer gear | within 0.01 m |
| A388 | span, length, outer gear | within 0.02 m; wheelbase unresolved (30.37 vs 31.88, likely wing vs body gear) |
| A339 | span, length, outer gear | within 0.01 m |
| A332 | length | **Airbus 58.82 m, FAA 59.01 m** |
| A333 | length, outer gear | within 0.03 m; span not found on the sheets parsed |
| B77W | span 64.80, length 73.86, wheelbase 31.22, track 10.97 (rendered Boeing p18) | within 0.01 m of FAA |
| B38M, B738, B788, B789, AT76 and the rest | not checked: Boeing 737/787 sheets are raster images not read; no ATR document opened | FAA only |

Tail height was not cross-checked: Airbus prints heights at a stated weight and
attitude, FAA gives tail height at OEW, and the parse cannot tell which printed
height is which. Those rows sit in `discrepancies.csv` marked as definitions, not
contradictions. `height_m` is left empty for that reason; `tail_height_m` is FAA.

**Weights.** Manufacturer values replace FAA for A20N, A320, A21N, B77W, B77L,
B788, B789, B38M, B738. Three real disagreements: A321neo MTOW 97,000 kg printed by
Airbus against FAA 95,000; 737-8 82,644 against 82,191; and FAA's "B77L" MALW of
260,816 kg, which is the 777 Freighter's (the 777-200LR's is 223,167).

**Suspect FAA entry:** B38M outer main gear width 5.73 m, where the same database
gives the 737-800 7.01 m. 5.73 m is near a 737 track, not an outer-tyre span. Not
replaced, because the Boeing sheet was not read.

**Turning radius** is the wingtip radius (R4) at the manufacturer's effective or
maximum steering angle, named per row: 13 types (A20N, A320, A21N, A321, B38M,
B737, B738, B739, B788, B789, B78X, B77W, B77L).

**Take-off and landing.** Three take-off figures (A20N ~1,960 m ISA SL, 777-300ER
~3,050 m, A320ceo ~2,210 m at ISA+15 and Delhi elevation, in the note) and one
landing figure (A20N 1,650 m, EUROCONTROL). Airbus publishes ISA and ISA+15
charts; Boeing publishes standard day, +15, +27 and +33 °C. The brief's ISA+20 is
on neither, so the nearest chart is named instead. `vref_kt` is FAA's approach
speed for every type that has an FAA row.

## Part 3 — `geometry/`

Three outlines traced from Airbus vector sheets by `trace.js`: scale from the
length dimension's extension lines only, so the span test is independent. Gear
positions come from the turning-radius tables: the turn centre lies on the
main-gear axle line, so nose to main gear = √(R5² − Y²); the nose gear is one
wheelbase ahead. The A320neo gives 17.73 m and 5.09 m, and √(R3² − Y²) = 12.76 m
against a 12.64 m wheelbase (R3 runs to the tyre's outside face).

`node research/work/aircraft/check.js`, run 2026-11-07:

```
A20N  54 pts  max x 37.60 / 37.57 (+0.08%)  2*max y 35.50 / 35.8 (-0.84%)  ok
A21N  69 pts  max x 44.52 / 44.51 (+0.02%)  2*max y 36.20 / 35.8 (+1.12%)  FAIL: span 1.12%
A321  69 pts  max x 44.52 / 44.51 (+0.02%)  2*max y 34.04 / 34.1 (-0.18%)  ok
1 outline(s) fail
```

**The A21N fails by 0.12 points, and the file is kept with the failure on
record.** It is the same wing as the A20N, which traces 0.84% short, so the two
Airbus sheets disagree by 1.96% on one wing; the sharklet tip is drawn swept aft
on the A321neo sheet, and the centreline (from the nose and tail tips, ±0.3 m)
is the likely error. The A321 outline is the **wing-tip-fence** A321ceo
(34.10 m), which is what Air India's older A321s fly, per the sheet; its
sharklet variant was not traced.

**Not traced:** engines (nacelle labels not mapped), and every other type. The
777-300ER sheet is vector, but its dimension and extension lines have the same
line weight as the airframe and cross it, so the flood fill leaks; it needs
hand-set exclusion boxes. The Boeing 737 and 787 sheets are raster
(`trace_raster.js` exists; the earlier run's 737-8 attempt leaked the same way).

## Part 4 — `operators.csv`

95 rows: 6 Indian scheduled operators, 66 foreign passenger carriers from
Wikipedia's IGI table, 18 cargo operators, Quikjet, and the Indian Air Force.
Telephony is from JO 7340.2P for 89 of them. Fleet counts are Wikipedia's for the
Indian operators; foreign carriers' DEL types are marked `authored` except
Emirates. Terminals are from third-party guides and an IndiGo press release, not
from DIAL, whose airline page carries no data in its HTML.

## Part 5 — `movements.md`

Daily, monthly and annual counts, the domestic/international split, peak-hour
figures, the fleet-mix estimate by class, seasonality. The hourly profile is a
gap; the file says what would build one.

## Part 6 — `sim-constants.md`

One table row per authored constant in `aircraft.rs`, and the per-type
`types.csv` column that should replace each.

## Solid vs. assumed

| Solid | Assumed |
|---|---|
| Dimensions of the 9 Airbus and 1 Boeing types cross-checked against the manufacturer | Dimensions of B38M, B738, B788, B789, AT76 and the light rows: FAA only |
| MTOW/MLW from manufacturer tables (9 types) | DEL fleet mix: a model with four authored inputs |
| ICAO wake per type (FAA database's ICAO WTC column) | RECAT-EU for 11 post-2013 types, inferred from family |
| Code letters (span bands from RA 3510) | Each foreign carrier's main type at DEL |
| Telephony (JO 7340.2P) | Indian/foreign split of international movements (50:50) |
| DEL passenger ATMs by month and the 77:23 split (GMR filings) | Take-off distances: chart read by eye, ±100–150 m |
| A380 daily from 25 Oct 2026 (Emirates) | Taxi speeds: community sources only |
| Turning radii for 13 types (manufacturer tables) | Gear positions: derived from turning tables, not read off a drawing |

## Gaps

| Gap | Where looked |
|---|---|
| **Hourly movement profile and arrival/departure mix per hour** | DIAL flight-information and timetable pages (client-rendered, no data in HTML); searches for slot declarations and hourly profiles; OpenSky needs an account and `OPENSKY_CLIENT_ID` is not set |
| **V2, VR and take-off/landing distances for 34 of 35 types** | Manufacturer charts are on disk for the Airbus and Boeing types (section 3 of each) and were not read for lack of time; EUROCONTROL's Aircraft Performance Database has them all, but its path `/ilp/customs/` is disallowed by robots.txt. One page (A20N) was fetched before robots.txt was read; no other was fetched, and the A20N values are marked |
| **DEL movements by airline or type** | Searches for DEL airline shares (none newer than an OAG blog the plan already rejected); DGCA publishes national shares only |
| Plan-view outlines for 32 of 35 types, and engine positions for all | See Part 3 |
| ATR 72/42, Dash 8-400, Embraer, Do 228, An-32 manufacturer documents | Not downloaded; ATR and De Havilland airport-planning manuals were not searched for |
| Cargo, GA and military movement counts | GMR counts passenger ATMs only; AAI's airport-wise tables not found by search |
| Fog capacity per hour under CAT III; Dec–Jan monthly ATMs | Press reports give cancellations only; GMR Dec 2025 and Jan 2026 filings 404 at the guessed URLs |
| Pushback speed | No manufacturer or tug source in the documents read |
| Turnaround times per class at DEL | One search; none found |
| IATA type and airline codes in the two CSVs | Written from memory and not checked; IATA's code lists are paywalled |
| Engine models in `types.csv` | From memory, except where a manufacturer page is cited in the cell |
| ICAO Doc 8643 itself | Not opened; wake categories come from the FAA database's ICAO WTC column |

Sources: `sources.csv` in this folder (37 rows).
