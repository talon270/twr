# Brief: aircraft types and traffic at Delhi IGI

Output folder: `research/out/aircraft/`. Topic name for `research/sources/` and
`research/work/`: `aircraft`.

## Why this exists

Every aircraft in the game today is the same authored A320-class object. The
constants at the top of `crates/sim/src/aircraft.rs` say so in their own comments:
taxi 10 m/s, pushback 1 m/s, takeoff acceleration 2 m/s², rotation 75 m/s, climb
cap 90 m/s, all `authored`. This run replaces them with real per-type numbers, and
pins down which types, operators and callsigns actually fly through Delhi, in what
mix, at what hours.

**"Perfect" means each aircraft is the right size, the right shape and behaves in
its real envelope.** A 777 must be 64.8 m wide on the map if the manufacturer says
so, must be refused at a stand built for code C, and must need more runway than an
ATR. A drawing that looks roughly like a jet is not good enough; a silhouette
traced from the manufacturer's three-view drawing is.

## Part 1 — which types to cover

Build the list from evidence before researching any type in depth:

1. The fleets of every operator with scheduled service at DEL: IndiGo, Air India,
   Air India Express, Akasa Air, SpiceJet, Alliance Air, and the foreign
   carriers (Emirates, Qatar, Etihad, flydubai, British Airways, Lufthansa,
   Singapore, Thai, and the rest — find the full current list). Include cargo
   operators (Blue Dart, Quikjet, foreign freighters) and Air Force Station Palam
   VIP/transport types where public sources name them.
2. Rank types by share of DEL movements where any source gives it; otherwise by
   fleet count of DEL-heavy operators. Say which method each rank used.
3. **Cover every type with an estimated share above 0.5% in depth.** Below that,
   dimensions and wake category only. Expect roughly 25–40 types, counting
   variants that differ in size (A320neo vs. A321neo; 777-200LR vs. 777-300ER;
   ATR 72-600 vs. ATR 42).

## Part 2 — per-type data: `types.csv`

One row per type variant, keyed by ICAO type designator (`A20N`, `A21N`, `B738`,
`B38M`, `B77W`, `B788`, `AT76`, ...). Columns:

| Group | Columns |
|---|---|
| Identity | `icao, iata, manufacturer, model, variant, engines (count + model), first_delhi_operator_example` |
| Size | `wingspan_m, length_m, height_m, wheelbase_m, main_gear_track_m, outer_main_gear_span_m, tail_height_m` |
| Classification | `icao_code_letter (A–F), icao_wake (L/M/H/J), recat_eu, faa_adg, faa_aac` |
| Weights | `mtow_kg, mlw_kg` |
| Ground | `min_turn_radius_m` (from the turning-radius figure; name the steering angle used), `pushback_note` |
| Takeoff | `takeoff_distance_m_at_mtow_isa_sl`, `v2_kt`, `rotate_kt` (where published) |
| Landing | `vref_kt` or `vat_kt` at MLW, `landing_distance_m_at_mlw`, `approach_category` |
| Grade | `source_ids` per group, and `grade` (`primary` when from the manufacturer's airport-planning document or type certificate) |

Sources, in rank order:

| Source | Covers |
|---|---|
| Manufacturer airport-planning documents: Boeing "Airplane Characteristics for Airport Planning", Airbus "Aircraft Characteristics — Airport and Maintenance Planning", ATR, Embraer, De Havilland equivalents | Dimensions, gear geometry, turning radii, takeoff and landing field length charts, three-view drawings. All are free downloads. These are the ground truth |
| EASA and FAA type certificate data sheets | MTOW, MLW, variants |
| ICAO Doc 8643 (aircraft type designators) | Designators and wake category |
| EUROCONTROL RECAT-EU and the Aircraft Performance Database | RECAT category, typical approach and climb speeds |
| FAA Aircraft Characteristics Database (xlsx) | A second source for every dimension; cross-check, do not copy blind |

**Cross-check every dimension against a second source.** Record mismatches over
0.1 m in a `discrepancies.csv` with both values and both sources.

Takeoff and landing figures depend on weight, temperature and elevation. Delhi
sits at about 777 ft and reaches 45 °C in May. Record the standard figure, then,
where the manufacturer chart allows, the figure at Delhi elevation and ISA+20 °C,
and say which chart and curve you read it from.

## Part 3 — shapes: `geometry/<icao>.json`

The top-down map needs a plan-view outline per type, at real scale. For each
in-depth type, trace from the manufacturer's three-view drawing in the
airport-planning document:

```json
{
  "icao": "A20N",
  "source_id": "airbus-a320-ac-rev-xx, fig 2-1-1",
  "units": "m",
  "origin": "nose tip, x aft, y right",
  "length_m": 37.57, "wingspan_m": 35.80,
  "outline": [[0,0], [x,y], ...],
  "engines": [{"x": .., "y": .., "length_m": .., "diameter_m": ..}],
  "main_gear": {"x": .., "track_m": ..}, "nose_gear": {"x": ..},
  "pivot_note": "where the aircraft turns about, for drawing rotation"
}
```

The outline is one closed polygon of the right half (the left is its mirror),
with enough points to show nose, wing root and tip leading and trailing edges,
winglet or sharklet, horizontal tail, and vertical tail position. Scale it from
the dimensioned drawing, not from a photo. Write a node script under
`research/work/aircraft/` that checks every outline: its maximum x equals
`length_m` and twice its maximum y equals `wingspan_m`, both within 1%. Report
the check's output in `REPORT.md`.

## Part 4 — who flies: `operators.csv`

`operator, icao_code, iata_code, telephony (the spoken callsign, e.g. IGO →
"IFLY"), country, delhi_terminal, fleet_types_at_del, fleet_counts, source_id,
source_date`. Telephony designators come from ICAO Doc 8585 or the operator;
check each one, several are not what you would guess.

## Part 5 — traffic: `movements.md`

How Delhi's day actually looks, as numbers:

- Daily and annual movements, domestic vs. international vs. cargo split, from
  AAI monthly traffic statistics and DIAL. The plan uses ~1,275 per day and a
  peak of 84 per hour (`PLAN-twr.md` § A2); confirm or correct both.
- **The shape of the day:** movements per hour across 24 hours. Delhi has
  pronounced morning and evening banks and an overnight international wave.
  Find a published hourly profile; if none is public, say so and describe what
  public schedule data (airline timetables, DIAL's flight information pages)
  could build one, without scraping anything whose terms forbid it.
- Arrival vs. departure mix per hour if available.
- Fleet mix as shares: narrowbody / widebody / turboprop / regional / cargo.
- Typical runway occupancy times and turnaround times per class, if published
  by AAI, DIAL or a study of Indian airports.
- Seasonality: winter fog (December–January) cancellations and capacity
  reductions, monsoon effects.

OpenSky Network's historical arrivals and departures API needs an account; if
the environment variable `OPENSKY_CLIENT_ID` is not set, record it as a gap and
describe what that data would add.

## Part 6 — replacements for the sim's constants: `sim-constants.md`

For each `pub const` in `crates/sim/src/aircraft.rs` that says `authored`, a
table: `constant, current value, what it should become (a per-class or per-type
value), the real figure, source, grade`. Taxi speed (straight and turning),
pushback speed, takeoff acceleration, rotation speed and climb-out are the
minimum. Where the sim uses one constant but reality varies by type, say so and
give the per-type column in `types.csv` that should replace it.

## Out of scope for this run

Airline liveries and logos (trademarks; the game will not ship them). Cockpit
procedures, flight dynamics beyond the runway, en-route performance.
