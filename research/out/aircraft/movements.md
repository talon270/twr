# Delhi IGI (VIDP) — how the day looks, as numbers

Researched 2026-11-07. Counts rest on GMR Airports' monthly traffic filings for
August and September 2026 (`gmr-traffic-2026-08`, `gmr-traffic-2026-09`); shares
rest on DGCA's September 2026 domestic figures as reported by the press
(`dgca-sep2026-press`). Every row names its source id from `sources.csv`.

## Daily and annual movements

**The plan's ~1,275 a day is confirmed, but it counts passenger flights only.**
GMR's tables carry the footnote "ATMs are Pax ATMs": cargo, general aviation and
Air Force Palam movements are not in any figure below.

| Period | DEL passenger ATMs | Per day | Source |
|---|---|---|---|
| FY2026 (Apr 2025 – Mar 2026) | 459,537 | 1,259 | gmr-traffic-2026-09, p2 |
| H1 FY27 (Apr – Sep 2026) | 233,331 | 1,275 | gmr-traffic-2026-09, p2 |
| Q1 FY27 (Apr – Jun 2026) | 120,919 (H1 minus Q2; the subtraction is ours) | 1,329 | derived |
| Q2 FY27 (Jul – Sep 2026) | 112,412 | 1,222 | gmr-traffic-2026-09, p2 |
| July 2026 | 36,212 (Q2 minus Aug minus Sep; ours) | 1,168 | derived |
| August 2026 | 38,861 | 1,254 | gmr-traffic-2026-08, p2 |
| September 2026 | 37,339 | 1,245 | gmr-traffic-2026-09, p2 |

**Runway 11R/29L was shut for about seven months and reopened on 20 Sep 2026**
with a new rapid exit taxiway, RET Z1 (Business Today and aviationa2z, 19–20 Sep
2026, via search summary). So most of the H1 FY27 traffic above ran on three
runways, not four.

## Domestic, international, cargo

| Split | Value | Source |
|---|---|---|
| Domestic ATMs, H1 FY27 | ~179,000 (76.5%) | gmr-traffic-2026-09, p3 ("record Domestic ATMs (~179K)") |
| International ATMs, H1 FY27 | ~55,000 (23.5%) | gmr-traffic-2026-09, p3 |
| Domestic passengers, H1 FY27 | 28.1 mn | gmr-traffic-2026-09, p3 |
| International passengers, H1 FY27 | 10.4 mn | gmr-traffic-2026-09, p3 |
| Cargo, September 2026 | ~1.11 lakh tonnes (~111,000 t); H1 FY27 ~6.3 lakh t | gmr-traffic-2026-09, p3 |
| Cargo movements | **gap** | GMR counts passenger ATMs only; AAI's airport-wise movement tables (which include cargo) were not found on aai.aero by search |

**The domestic:international split is 77:23, not the plan's authored 80:20.** The
two GMR figures are rounded ("~179K", "~55K") and sum to 234K against the exact
233,331, so treat the split as ±0.5 points.

## Peak hour and slots

| Figure | Value | Source | Note |
|---|---|---|---|
| Peak movements per hour | 84 | cw-peak-2024 (DIAL CEO, June 2024) | Not re-confirmed for 2026. A 2017–18 report gave 67 declared, 84 recorded |
| Planned peak | 110 per hour | cw-peak-2024 | A target, not reached |
| Slots per day | 1,500, rising past 2,000 | cw-peak-2024 | 2024 figure |
| Peak to average | 84 ÷ 53 = 1.6 | derived | 53 = 1,275 ÷ 24 |
| Runway occupancy | "approx. 40–45 seconds" | Zee Business, undated (DIAL working on ROT) | Undated, so its year is unknown; no per-class split |

## The shape of the day

**Gap: no public hourly movement profile for DEL was found.** Searches for a DIAL
or AAI hourly profile, a slot capacity declaration, and IATA slot-conference data
for DEL returned nothing usable. What could build one, without scraping anything
whose terms forbid it:

| Route | What it gives | Why not done |
|---|---|---|
| DIAL live flight information / flight timetable pages | Every scheduled flight with time, airline, terminal | Both URLs return a client-rendered shell (423,877 bytes, no flight data in the HTML). The data comes from an API whose terms were not checked; not touched |
| Airline published timetables (IndiGo, Air India) | Their own DEL flights by hour | Two airlines are ~80% of domestic movements; possible, but hours of collection per airline |
| OpenSky Network historical arrivals/departures | Actual movements by hour, with ICAO 24-bit address, so type via a registry join | Needs an account; `OPENSKY_CLIENT_ID` is not set in this environment (checked by `research/work/aircraft/envcheck.js`) |
| OAG or Cirium schedules | The full scheduled profile and fleet per flight | Paid |

**What OpenSky would add:** real arrival and departure counts per hour for a chosen
week, the arrival/departure ratio per hour, and (joined to its aircraft database)
the type of each movement. That would replace both the estimated fleet mix below
and the plan's flat-rate modes with a measured 24-hour curve.

**Arrival vs. departure mix per hour: gap.** The only figure is the record hour,
82 movements = 40 landings + 42 takeoffs (PSU Connect, undated, as quoted in
`PLAN-twr.md` § B6; not re-checked in this run).

## Fleet mix

**Estimated, not measured.** No source gives DEL movements by airline or type.
`research/work/aircraft/estimate_mix.js` builds the estimate from the 77:23 split
(GMR), national domestic passenger shares (DGCA via press), Wikipedia fleet counts,
Emirates' published DEL schedule, and four authored inputs it labels: Indian
carriers fly half of the international movements; their international split is
Air India 55 / IndiGo 35 / AIX 10; each foreign carrier flies one daily rotation
unless a source says otherwise; and each foreign carrier's main type at DEL. The
model's 79 foreign rotations (158 movements) land within 5% of the 150 a day the
split implies, which checks the one-rotation guess and nothing finer.

| Class | Types | Est. share of passenger movements |
|---|---|---|
| Narrowbody jet | A20N 34.1, A21N 27.0, B38M 10.9, A320 5.0, B738 3.3, A321 1.6, A319 0.8, B737 0.2, B739 0.03 | 83.0% |
| Widebody | B788 3.0, B77W 2.4, B789 2.0, A333 1.5, A359 1.2, A343 0.3, A388 0.15, B748 0.15, A332 0.15, A339 0.15, B78X 0.15 | 11.1% |
| Turboprop | AT76 5.6, DH8D 0.2, AT46 0.03, D228 0.01 | 5.8% |
| Regional jet | E190 0.15 | 0.15% |
| Cargo | excluded: not in passenger ATMs | gap |

**Heavies are about 11% of passenger movements, not the plan's 20%.** Every
widebody above is ICAO H except the A380 (J). The plan's 20% came from an authored
fleet table that gave foreign carriers 30% of all movements; GMR's split leaves
them about 12%.

**The ATR share is the weakest number here.** IndiGo's 44 ATRs are counted in
proportion to its fleet, but they fly regional sectors from other bases, so 5.6% at
DEL is probably high. Direction unconfirmed.

## Turnaround times

**Gap.** No AAI, DIAL or study figure for DEL turnaround by class was found in one
targeted search. The plan's 35–60 min (§ B6) is authored.

## Seasonality

| Season | Effect | Source |
|---|---|---|
| Monsoon (Jul) | July 2026 averaged 1,168 movements a day, the lowest month in FY27 so far; Apr–Jun averaged 1,329. 12% lower, though July also had 11R/29L closed | derived from gmr-traffic-2026-08/-09 |
| Winter fog (Dec–Jan) | 15 Dec 2025: 60+ cancellations, 250+ delays. 29 Dec 2025: 128 cancellations, 8 diversions. 31 Dec 2025: 78 arrivals and 70 departures cancelled (148, about 12% of a 1,275 day) | fog-bt-2025-12-15, fog-ada-2025-12-29, fog-gn-2025-12-31 |
| CAT III | Three runways (10/28, 11L/29R, 11R/29L) CAT III at both ends for winter 2025–26 | dial-cat3-2025 |
| Fog capacity per hour | **gap**: no source quantified arrivals or departures per hour under CAT III | — |
| December and January monthly ATMs | **gap**: GMR's Dec 2025 and Jan 2026 filings returned 404 at the guessed URLs | — |
