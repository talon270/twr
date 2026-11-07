# Brief: Delhi IGI (VIDP) — the airfield as a tower controller sees it

Output folder: `research/out/vidp/`. Topic name for `research/sources/` and
`research/work/`: `vidp`.

## Why this exists

The game routes every aircraft along named taxiways and holds it at named holding
points. OpenStreetMap gives the geometry but names only 245 of 591 taxiway segments
(41%) and **none** of the 46 holding points (`PLAN-twr.md` § A3). The game cannot
issue "TAXI 29L VIA A D HOLD SHORT 28" until those names exist, and they have to be
the real ones. The eAIP aerodrome charts are where the real ones live. This run's
main job is to read them and turn them into data the extractor can use.

## Inputs already on disk

- `data/vidp.osm.json` — raw Overpass answer from 2026-11-05: every runway,
  taxiway, taxilane, stand, holding point, apron and terminal in the VIDP area,
  with OSM ids and node coordinates.
- `data/vidp.overrides.toml` — the empty override file the extractor reads. Read
  its header for the format. **Do not edit it.** You write a proposed version.
- `crates/osm/src/main.rs` — the extractor; read it to see which tags it uses.

## Primary sources to find and read

The eAIP lives at `https://aim-india.aai.aero.in/` (checked 2026-11-05). The
older `aim-india.aai.aero` answers with a cross-host redirect there, which
WebFetch will not follow; use the `.in` host directly, or curl with `-L`.

| Source | What it holds | Notes |
|---|---|---|
| AAI eAIP India, AD 2 VIDP | AD 2.8 aprons/taxiways, AD 2.9 SMGCS and markings, AD 2.12 runway physical data, AD 2.13 declared distances, AD 2.14 lighting, AD 2.18 ATS frequencies, AD 2.20 local traffic regulations, AD 2.22 flight procedures (low visibility) | Record the AIRAC effective date of the issue you read |
| eAIP charts for VIDP | Aerodrome Chart (ADC), Aircraft Parking/Docking Chart (APDC), Ground Movement Chart, hot-spot chart, any standard taxi route chart | These carry the taxiway letters, holding point names, stand numbers and hot spots |
| AIP Supplements and the current NOTAM summary for VIDP, where public | Closures, new taxiways, runway works that the base AIP does not show yet | Note anything that changes the layout |
| ICAO Annex 14 Vol I, Chapter 5 (visual aids) | What each marking, sign and light looks like and means | For the drawing rules, not for VIDP specifics |
| AIP India GEN 1.7 | India's filed differences from ICAO standards | Check whether any affect markings or signs |
| DIAL (newdelhiairport.in) | Terminal gate numbers (T1, T2, T3), which gates are contact vs. remote/bus, terminal-to-airline allocation | Gates (boarding doors) are not stands (aircraft positions). Keep them separate and map one to the other where a source does |

## What to produce

### 1. `runways.csv`

One row per runway direction: `designator, true_bearing_deg, length_m, width_m,
surface, threshold_lat, threshold_lon, threshold_elev_ft, displaced_threshold_m,
tora_m, toda_m, asda_m, lda_m, ils_category, approach_lighting, rets (rapid exit
taxiway names), notes, source_id`. Four runways: 09/27, 10/28, 11L/29R, 11R/29L.
Check whether all four are in service on the AIRAC date you read, and record any
closure or works.

### 2. `taxiways.csv`

One row per taxiway designator: `name, width_m, max_code_letter (A–F),
connects (runways/taxiways/aprons it joins), one_way (Y/N + direction),
restrictions, is_rapid_exit, is_elevated, source_id`. Include the Eastern Cross
Taxiway (the elevated one) with its opening date and the taxiways it links.

### 3. `holds.csv`

One row per holding point: `name, runway_protected, on_taxiway, category
(CAT I / CAT II/III / intermediate), source_id`. Holding points are where the
game's "HOLD SHORT" commands stop aircraft; each one needs its real name.

### 4. `stands.csv`

One row per stand: `stand, apron, terminal, max_code_letter, max_type_example,
contact_or_remote, gate (if mapped), pushback_required, power_in_out, source_id`.

### 5. `overrides.proposed.toml`

The same format as `data/vidp.overrides.toml`, filled from the charts. **This is
the file that unblocks the game.** Method:

1. Read the chart; note each taxiway letter's route and each holding point's
   position relative to runways and junctions.
2. Write a node script under `research/work/vidp/` that loads
   `data/vidp.osm.json`, lists unnamed taxiway ways and holding-position nodes
   with their coordinates and their neighbours (which named ways they touch, which
   runway they are nearest), so each unnamed element can be matched to the chart.
3. Assign a name only where the chart and the geometry agree unambiguously. Above
   each entry, a comment: `# chart: <chart name, AIRAC>, evidence: <one line>`.
4. Elements you could not match go in a commented `# UNMATCHED` block at the end
   with the reason, never in the live tables.

Report in `REPORT.md`: named taxiway segments before and after (245 of 591 today),
holding points before and after (0 of 46 today), and any OSM element that
contradicts the chart (a taxiway the chart says no longer exists, a stand
renumbered). Those contradictions are findings.

### 6. `markings.md` — how to draw it

For a top-down renderer: every marking, sign and light the game might show, as a
table: `element, colour (with ICAO name), pattern and dimensions, where it appears
at VIDP, what it means to the pilot, source`. At minimum: runway designation,
threshold, centreline, touchdown zone, aiming point, edge; taxiway centreline,
enhanced centreline, edge; runway-holding position patterns A and B; intermediate
holding position; ILS critical/sensitive area; stand lead-in and stop lines;
mandatory instruction signs, location signs, direction signs; stop bars; RETILs.
Note whether VIDP uses enhanced taxiway centreline markings and where.

### 7. `operations.md` — how the airfield is used

What the tower actually does with this layout, because it decides what a shift
feels like:

- **Runway configuration:** which runways are used for arrivals and which for
  departures in each flow (easterly 09/10/11 vs. westerly 27/28/29), mixed-mode
  or segregated, and how wind and time of day pick the flow. Quote AD 2.20.
- **Runway crossings:** which runways arriving or departing traffic must cross to
  reach which terminal, and how the Eastern Cross Taxiway changed that. Find
  what DIAL or AAI say it removed, with the number of crossings before and after
  if published.
- **Runway-holding distance:** `crates/sim/src/aircraft.rs` uses
  `HOLD_SHORT_M = 90`, from memory of ICAO Annex 14 Table 3-2 and never checked.
  Check it against the table for each VIDP runway's code and approach category,
  and against the holding-point positions on the chart.
- **Standard taxi routes**, if published.
- **Hot spots:** each one, where it is, why it is one.
- **ATC positions and frequencies:** Delivery, Ground (how many), Tower (how
  many), with frequencies from AD 2.18.
- **Low visibility procedures:** Delhi's winter fog season, CAT IIIB runways,
  what changes on the ground (stop bars, follow-the-greens, reduced rates).
- **Air Force Station Palam:** the military side shares the airfield. What
  public sources say about its movements and how they mix with civil traffic.
- **Movement rates:** declared hourly capacity per runway configuration, if
  published. `PLAN-twr.md` § A2 uses 84 per hour as the peak; check it.

## Out of scope for this run

Approach and departure procedures beyond the runway (SIDs, STARs, vectoring) —
the game starts aircraft on final. Airline fleets and traffic mix — that is the
`aircraft` brief.
