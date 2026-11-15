# twr

A tower controller game set at Delhi's Indira Gandhi International (IGI,
VIDP), built in Rust and run as WebAssembly in one offline HTML file. The
airport is OpenStreetMap's geometry (4 runways, 591 taxiway ways,
extracted 2026-11-05) with the eAIP apron charts' 270 stands (AIRAC
2026-10-29). The traffic follows IGI's real rates (DIAL:
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
./build.sh                                        # dist/index.html, copied to docs/index.html
cargo test --release --workspace                  # 157 tests, about 5 min
python3 tools/verify_b9.py /tmp/twr-shots         # Playwright, real clicks
cargo run -p twr-osm -- VIDP                      # re-extract after editing data/vidp.overrides.toml
```

`build.sh` needs the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` at
the version in `Cargo.lock`, and `wasm-opt` (binaryen). It checks the
wasm-bindgen version before it builds. It also copies the page to
`docs/index.html`, which GitHub Pages serves from `main`: commit and push
`docs/` to publish. `tools/README.md` lists every verify script.

## Controls

| Input | Does |
|---|---|
| Click an aircraft or its strip | select it |
| Drag, wheel | pan, zoom (zoom springs; jumps under reduced motion) |
| `P` | push back from the stand |
| `S` | push back, then taxi on the previewed route |
| `T`, click a stand, hold or runway entry, `Enter` | taxi on the previewed route |
| click a taxiway while taxiing | route via it; right-click avoids it; click again to undo |
| `H` | hold position |
| `X` | cross the runway held short of |
| `L` | line up |
| `K` | clear for takeoff |
| `C` | clear to land |
| `G` | go around |
| `E`, click an exit | pick the landing aircraft's exit |
| `J`, click a marked point | hold at a point ahead on the route; `Enter` continues |
| `Y`, click an aircraft | give way to that aircraft at the junction; it releases itself once the other has passed |
| `-` / `=`, `B` | taxi slower / faster; backtrack on the runway |
| `Space` | pause (on a button reached with `Tab`, `Space` and `Enter` press it) |
| End shift | end a Light, Normal, Peak or Surge shift and show the result |
| `Esc` | cancel, deselect, close the result |

| Mode | Traffic | Ends |
|---|---|---|
| Practice | 2 aircraft, west flow, calm | never; no score |
| Light / Normal / Peak | 27 / 53 / 84 movements per real hour | when you press End shift; no time limit |
| Surge | 53 per real hour, plus 10% every 2 minutes | the third strike, or End shift |

**A Surge strike is an incident or a go-around for no landing clearance.**
Incidents alone let a Surge run for ever if you touched nothing: seed 7 ran
111 real minutes idle with no end. The no-clearance go-around is the one
go-around the score already blames on you; idle, seed 7 now ends at 6:28 on
three of them. Pilot and weather go-arounds stay challenges, not strikes.

**The movements best outside Surge is a rate.** With no time limit, the
longest sitting would hold a count record, so Light, Normal and Peak compare
movements per real hour, and the record prints its count and length beside
the rate: "86/h · 43 in 30:00". It needs 10 movements, as the delay best
does; above that a short lucky burst can still flatter the rate. Surge keeps
the count, since its third strike bounds its length.

`?mode=peak&seed=7` replays a shift: same seed, same wind, same traffic.
**Practice lets you choose fog.** Use the Fog control in the wind box to turn
LVP on or off.

**Cards that wait on you move to the top.** A card that asks for something
sits in **Needs you**, green when new, yellow after 15 real seconds, red and
flashing after 40, or at once for a conflict or a landing clearance 1,500 m
from its deadline. The words "Waiting" and "Act now" say it as well, and under
reduced motion the red stays steady. The 15 s and 40 s are authored, not
measured: they are two constants at the top of `crates/web/src/lib.rs`. An
aircraft on its stand that asks for nothing sits in **Parked**.

**A landed aircraft stops clear of the runway.** Its exit ends at least 90 m
in a straight line from the centreline, not after 90 m of taxiway. Before this,
all 8 default exits stopped it 9 to 51 m from the centreline. The price is
time on the runway: 29L to Z2 now takes 98 s, up from 75 s.

**The next arrival waits for the runway, not for a distance.** Final spacing
used to be 3 NM, 79 s at final speed, while the runway stayed occupied up to
98 s. A player clearing every arrival at 2 km then got 27 runway incidents in
ten Peak shifts (201 landings). A follower now appears on final only when its
leader will have cleared the runway 15 s before the follower reaches the
threshold. The same ten shifts raise 0. The price is a longer wait for a
slot: at most 3 aircraft waiting, as before, and total waiting on the busiest
seeds up from about 1.7 to 3.2 aircraft-minutes. When landings fill a runway
there is no gap to cross it until a lull; `tests/spacing_clear.rs` is the
check, and the 15 s is `LANDING_CLEAR_BUFFER_S`.

## The things most tower games get wrong

**Movement is compressed; the traffic rate is not.** Taxi, pushback and the
takeoff roll run 3× faster than real time, so a 4-minute taxi takes 80
seconds. The schedule counts per real hour. If it ran on the 3× clock too,
IGI's peak hour would arrive as 252 movements per real hour, a number with no
meaning. Peak mode is 84 per real hour, IGI's measured peak.

**The score is three numbers, never a blend.** Movements (beside what the
shift offered), delay you caused, and incidents print side by side. A
30-minute Peak sitting offered between 35 and 58 movements across 10 seeds, so
"40 movements" means nothing without "of 42 offered".

**Delay counts only what you could have cleared.** A departure waiting out a
2:00 wake gap is following the rule, so its wait is not delay. One that
waits on its stand past a 60-second grace, holds short of an empty runway,
or sits lined up with nothing binding K does count. Pilot and windshear
go-arounds are challenges, not delay. The controller in the tests causes
58–315 game minutes of delay per Peak shift (20 shifts: 10 seeds on their
own wind, 10 forced east), which gives you a number to beat.

**Fog changes when you can clear an arrival.** Low-visibility procedures
(LVP) are drawn for about 10% of shifts with steady wind at or below 6 kt:
293 of 2,781 eligible seeds in the first 10,000 draw LVP (10.5%). Practice
also has a Fog toggle. Under LVP, arrivals need 5 NM spacing on final and
the localizer sensitive area (LSA) must be empty before the next one joins.
Hold C while another aircraft is in the LSA; an uncleared arrival goes around
at 2 NM (3,704 m). Keep ground traffic out of the LSA from a cleared
arrival's 2 NM point until its landing roll ends. The taxi preview gives
published LVP taxiways a lower cost; a via can still take another route.
Ten Peak shifts forced to LVP (`tests/lvp_long.rs`, two real hours each,
the auto-controller at the controls) raise 0 incidents and finish all 436
movements; 27 arrivals go around for an occupied LSA. That is the test
controller's flow, not a rate for IGI: AD 2.2 gives no LVP rate, so the game
claims none.

**A go-around forced by an occupied LSA is not your delay.** Withholding C
while the LSA is busy follows the rule. At 2 NM that go-around is logged as
`LSA occupied`: it adds no delay or challenge, though the missed landing
still costs a movement. Forget C with a clear LSA and `no landing clearance`
still counts as delay. If you held an aircraft in the LSA, its wait counts.

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
| Landing roll inside DIAL's 40–45 s runway occupancy | 60 s stand grace, 60 s "delayed" threshold, Surge ends at 3 strikes (3 is your call; counting no-clearance go-arounds is the 2026-11-13 audit's) | — |
| AD 2.22 VIII sets the 2 NM (3,704 m) LVP clearance deadline | 5 NM final spacing, plus an empty LSA before the next arrival joins; set from the sim's LSA clearance timing | an LVP rate for IGI; ten forced-LVP Peak shifts run clean under the test controller, which is not a measured rate |
| Six AD 2.24 LSA charts, traced as 22 vector stations in `data/vidp.lsa.csv` | straight edges between stations, scaled by each chart's runway fill against OSM because the scale bars disagree with the runway lengths | surveyed LSA boundaries; these are chart shapes mapped to the sim, not surveyed polygons |
| AD 2.22 VIII Tables 2–3 supply 89 west and 92 east taxiway names in `data/vidp.lvp.csv` | a named taxiway outside the flow's set costs 4× its length; the graph lacks F6, H1, H6, Link 19 west and D8, D9, F6, G1, G2, Link 19 east | add the missing taxiways to the airport graph |
| AD 2.2 gives no LVP movement rate | LVP on 10% of eligible steady-wind shifts at or below 6 kt; 293 of 2,781 eligible seeds in the first 10,000 draw it | Delhi visibility and wind climatology |

## Data, licence, privacy

The airport data is © OpenStreetMap contributors (ODbL). The map's corner
shows that credit with the extract's date, and says it is **not for
real-world navigation**. The derived files in `data/` carry the same licence.
The eAIP charts are read as a reference only: no chart image ships.

**A title screen opens on every load** with the name, the credit, the eAIP
AIRAC date (2026-10-29), the OSM extract date and the not-for-navigation line.
Start or Esc closes it; the sim is held until then. Automated browsers
(`navigator.webdriver`) skip it so the verify scripts keep working; add
`?title=1` to force it.

**LVP bests have their own key.** The page stores only your theme and personal
bests in `localStorage` under `twr.v1` (schema 3). Fog shifts use `<mode>-lvp`
in the existing `bests` map, so clear and fog shifts do not compete. Schema 3
stores each movements best with its length in real seconds; a v2 record
outside Surge loads as a 30:00 shift, which is what every one of them was. It
sends nothing anywhere.

BLR and BOM layouts, per-type speeds and the
remaining research runs are planned for 1.1 (`PLAN-twr.md`).
