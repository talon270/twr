# twr

A tower controller game set at Delhi's Indira Gandhi International (IGI,
VIDP), built in Rust and run as WebAssembly in one offline HTML file. The
airport is OpenStreetMap's geometry (4 runways, 591 taxiway segments, 115
stands, extracted 2026-11-05). The traffic follows IGI's real rates (DIAL:
84 movements in the peak hour), and its fleet mix comes from GMR's filings
and DGCA's shares. You push, taxi, line up, clear for takeoff and clear to
land. The score is three numbers, never one.

## Run it

Open `dist/index.html` in Chrome (the verify scripts run Chromium; other
browsers are untested). It needs no server, no install
and no network: the wasm is inlined, and the page makes no request of any
kind.

Rebuild after a change:

```sh
./build.sh                                        # dist/index.html
cargo test --release --workspace                  # 86 tests, about 7 min
python3 tools/verify_b9.py /tmp/twr-shots         # Playwright, real clicks
cargo run -p twr-osm -- VIDP                      # re-extract after editing data/vidp.overrides.toml
```

`build.sh` needs the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` at
the version in `Cargo.lock`, and `wasm-opt` (binaryen). It checks the
wasm-bindgen version before it builds. `tools/README.md` lists every verify
script.

## Controls

| Input | Does |
|---|---|
| Click an aircraft or its strip | select it |
| Drag, wheel | pan, zoom (zoom springs; jumps under reduced motion) |
| `P` | push back from the stand |
| `T`, click a stand, hold or runway entry, `Enter` | taxi on the previewed route |
| click a taxiway while taxiing | route via it; right-click avoids it; click again to undo |
| `H` | hold position |
| `X` | cross the runway held short of |
| `L` | line up |
| `K` | clear for takeoff |
| `C` | clear to land |
| `G` | go around |
| `E`, click an exit | pick the landing aircraft's exit |
| `Space` | pause |
| `Esc` | cancel, deselect, close the result |

| Mode | Traffic | Ends |
|---|---|---|
| Practice | 2 aircraft, west flow, calm | never; no score |
| Light / Normal / Peak | 27 / 53 / 84 movements per real hour | 30:00 real |
| Surge | 53 per real hour, plus 10% every 2 minutes | the third incident |

`?mode=peak&seed=7` replays a shift: same seed, same wind, same traffic.

## The things most tower games get wrong

**Movement is compressed; the traffic rate is not.** Taxi, pushback and the
takeoff roll run 3× faster than real time, so a 4-minute taxi takes 80
seconds. The schedule counts per real hour. If it ran on the 3× clock too,
IGI's peak hour would arrive as 252 movements per real hour, a number with no
meaning. Peak mode is 84 per real hour, IGI's measured peak.

**The score is three numbers, never a blend.** Movements (beside what the
shift offered), delay you caused, and incidents print side by side. A
30-minute Peak shift offered between 35 and 58 movements across 10 seeds, so
"40 movements" means nothing without "of 42 offered".

**Delay counts only what you could have cleared.** A departure waiting out a
2:00 wake gap is following the rule, so its wait is not delay. One that
waits on its stand past a 60-second grace, holds short of an empty runway,
or sits lined up with nothing binding K does count. Pilot and windshear
go-arounds are challenges, not delay. The controller in the tests causes
58–315 game minutes of delay per Peak shift (20 shifts: 10 seeds on their
own wind, 10 forced east), which gives you a number to beat.

**A conflict is never missed.** The conflict monitor checks wake, runway
occupancy, incursions and ground spacing on every tick. The test controller
ran 10 game hours on each of 3 seeds, west and east flow, with 0 incidents.
Each rule is then broken on purpose at its boundary: one tick inside the
minimum is an incident, the minimum itself is not.

**The route you drew is the route flown.** The preview is the route, edge
for edge; the Playwright check compares the aircraft's trail with the
preview. A via that would make the route turn back on itself is refused
(648 of 4,649 single vias at IGI are, all of them bridges), not quietly
rerouted.

**A heavy cannot take off where it cannot take off.** A widebody at MTOW
needs about 3,100 m at Delhi's elevation (Boeing chart), and 27 and 09 have
2.7 km. Heavies depart 29R or 11L (4.4 km), and a line-up with too little
runway ahead is refused, with the metres in the message.

## Assumed vs solid

| Solid (sourced) | Assumed (`authored` in the code) | What would replace the assumption |
|---|---|---|
| Runway, taxiway and stand geometry (OSM, ODbL) | 90 m hold-short distance where OSM has no hold (Annex 14, from memory) | `vidp` research run: eAIP charts |
| Peak hour 84, daily ~1,275 movements (DIAL, GMR) | the 50/50 split between a flow's two runways | `vidp` run (AD 2.20) |
| Domestic 77%, international 23% (GMR, H1 FY27) | the fleet estimate: half of international flights Indian, one rotation a day per foreign carrier, each foreign carrier's type | an OpenSky movement count (needs an account) |
| Wake category per type (FAA database, manufacturers) | the wind: 75% westerly, 2–16 kt, gusts on 20% of shifts; calm below 5 kt keeps west flow | `vidp` run, METAR climatology |
| Departure wake times (GACA AIC 07/2020) | wake distances on final, 3 NM radar minimum (Doc 4444, from memory) | `separation` run; its `rules.csv` is not folded in yet |
| Takeoff run: A320neo ~2,030 m, 777-300ER ~3,100 m (Airbus and Boeing charts, read by eye, ±100–150 m, at MTOW) | MTOW for every departure; a lighter aircraft needs less | a weight model (1.1) |
| Climb 175 kt and approach 136 kt match the A320neo (EUROCONTROL, FAA) | one speed set for every type: taxi 10 m/s, push 1 m/s, 2 m/s² to 75 m/s rotation | per-type speeds; VR is a gap for every type in the research |
| Radio telephony for 68 operators (FAA JO 7340.2P) | pilot go-arounds 5%, windshear 3% on gusty shifts | — |
| Landing roll inside DIAL's 40–45 s runway occupancy | 60 s stand grace, 60 s "delayed" threshold, Surge ends at 3 incidents (your call) | — |

## Data, licence, privacy

The airport data is © OpenStreetMap contributors (ODbL). The map's corner
shows that credit with the extract's date, and says it is **not for
real-world navigation**. The derived files in `data/` carry the same licence.
The eAIP charts are read as a reference only: no chart image ships.

The page stores only your theme and personal bests, in `localStorage` under
`twr.v1` (schema 2). It sends nothing anywhere.

BLR and BOM layouts, low-visibility procedures, per-type speeds and the
remaining research runs are planned for 1.1 (`PLAN-twr.md`).
