# Replacements for the authored constants in `crates/sim/src/aircraft.rs`

Written 2026-11-07 against `aircraft.rs` as read today. Each row: the constant,
its value now, what it should become, the real figure, the source id from
`sources.csv`, and its grade. Per-type values live in `types.csv`; the column to
read is named.

**Two authored constants already match a sourced figure for the A320neo.**
`CLIMB_MDM_PER_TICK` (90 m/s) equals EUROCONTROL's initial-climb speed for the
A20N, 175 kt = 90.0 m/s, and `LAND_MDM_PER_TICK` (70 m/s, 136 kt) sits between
EUROCONTROL's Vat of 135 kt and the FAA database's 137 kt. The constants were
chosen as A320-class; for the A320 class they hold. What they cannot do is
describe a 777 or an ATR.

**`MIN_TAKEOFF_RUN_M` is short for the aircraft it was written for.** Airbus's
own chart puts an A320neo at 79,000 kg, ISA, sea level at about 1,960 m, and
about 2,030 m at Delhi's 777 ft. The 1,800 m floor lets a full-weight A320neo
line up where it could not legally go. A 777-300ER at MTOW needs about 3,050 m
at sea level (Boeing chart).

| constant | current value | should become | real figure | source | grade |
|---|---|---|---|---|---|
| `TAXI_MDM_PER_TICK` | 10 m/s (19.4 kt), one speed | two per-class speeds: straight and turning. No per-type source found | Common airline practice ~20 kt straight, ~10 kt in turns; one forum post says the 747-200/300 FCTM publishes no taxi limit; a secondary guide cites ICAO guidance of ~25 kt straight, 10 kt turns, not verified against ICAO | Simple Flying taxi-speed guide; PPRuNe 747 thread; aviationa2z 2025-04-06 (all via one search summary) | community / secondary. **No primary taxi-speed figure found** |
| (new) turn speed | none | 10 kt (5.1 m/s) on edges whose turn exceeds a threshold angle | as above | as above | community |
| `PUSH_MDM_PER_TICK` | 1 m/s | keep, labelled authored | **gap**: no tug or manufacturer pushback speed found. The Airbus and Boeing airport-planning documents read give tow and turning geometry, not speed | searched: one search summary; Airbus 4-3-0, Boeing 4.x | gap |
| `TAKEOFF_ACCEL_MDM` | 2 m/s², all types | per type: `v2_kt²` / (2 × `takeoff_distance_m_at_mtow_isa_sl` / 1.15), as an average to lift-off; 1.15 removes the all-engines margin in the certified distance (FAR 25.113, from memory, not opened) | A20N: V2 145 kt = 74.6 m/s, distance 1,951 m → 74.6² / (2 × 1,697) = **1.64 m/s²** (ours). Other types: V2 is a gap | ec-apd-a20n; airbus-a320 p175 | derived from secondary. The 1.15 factor is memory |
| `ROTATE_MDM_PER_TICK` | 75 m/s (146 kt), all types | per type `rotate_kt`; until sourced, V2 − 5 kt as an authored stand-in | A20N V2 145 kt (VR is below V2; how far below is not published in the documents read). **VR is a gap for every type** | ec-apd-a20n | gap |
| `ROLL_TICKS` | 750 (derived) | derived per type from the two above | — | — | — |
| `CLIMB_MDM_PER_TICK` | 90 m/s (175 kt) | per type initial-climb speed | A20N initial climb 175 kt, 2,200 ft/min | ec-apd-a20n | secondary |
| `LAND_MDM_PER_TICK` | 70 m/s (136 kt) | per type `vref_kt` | A20N 137 kt (FAA), 135 kt Vat (EUROCONTROL), 131.5 kt at MLW 67,400 kg (Airbus); ATR 72-600 113 kt; 737-800 144 kt; 777-300ER 149 kt; A380 138 kt (FAA approach speed, all) | faa-acd; ec-apd-a20n; airbus-a320 | secondary (FAA) / primary (Airbus A320) |
| `LAND_DECEL_MDM` | 1.6 m/s² | keep for now; check against ROT | DIAL quotes runway occupancy of "approx. 40–45 seconds". The current model's 70 → 10 m/s at 1.6 m/s² takes 37.5 s over 1,500 m, plus the turn-off: inside that band | Zee Business, undated (DIAL) | secondary, undated |
| `MIN_TAKEOFF_RUN_M` | 1,800 m, all types | per type `takeoff_distance_m_at_mtow_isa_sl`, scaled for elevation and temperature, or a per-class floor | A20N ~1,960 m ISA SL, ~2,030 m at Delhi elevation; A320ceo (V2500) ~2,210 m at ISA+15, Delhi elevation; 777-300ER ~3,050 m SL standard day, ~3,100 m at Delhi elevation. All read by eye from charts, ±100–150 m | airbus-a320 p175, p179; boeing-777 p54 | primary (chart read by eye) |
| `HOLD_SHORT_M` | 90 m | not this run's topic | Annex 14 Table 3-2 was not opened; the `vidp` run owns it | — | — |

## Where the sim uses one constant but reality varies by type

| Behaviour | One constant now | Per-type column in `types.csv` |
|---|---|---|
| Wingspan on the map, stand fit | none (all A320-sized) | `wingspan_m`, `length_m`, `icao_code_letter`; outlines in `geometry/` |
| Takeoff run needed | `MIN_TAKEOFF_RUN_M` | `takeoff_distance_m_at_mtow_isa_sl` (3 types filled) |
| Rotation | `ROTATE_MDM_PER_TICK` | `rotate_kt` (empty: gap), `v2_kt` (A20N only) |
| Approach and landing speed | `LAND_MDM_PER_TICK` | `vref_kt` (every type with an FAA row) |
| Wake | `Wake` per aircraft (already per type) | `icao_wake`, `recat_eu` |
| Turning room on taxiways | none | `min_turn_radius_m` with `turn_steering_deg` (13 types filled) |

## What the numbers say about the spread

| Type | Span | Vref | Takeoff distance at MTOW | Turn radius (wingtip) |
|---|---|---|---|---|
| ATR 72-600 | 27.04 m (FAA) | 113 kt | gap | gap |
| A320neo | 35.80 m | 137 kt | ~1,960 m | 22.5 m |
| 777-300ER | 64.80 m | 149 kt | ~3,050 m | 48.8 m |
| A380-800 | 79.75 m | 138 kt | gap | gap |

A 777-300ER needs about 1.6 times the A320neo's runway and over twice its turning
room. An A380 approaches slower than a 777-300ER (138 against 149 kt), so speed
on final does not track size.
