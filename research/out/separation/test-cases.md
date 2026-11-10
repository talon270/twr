# B4 rule test vectors

All times below are **game-clock seconds**, and state transitions occur at the stated tick. “Inside” means the safe boundary or just beyond the minimum; “outside” means one tick/metre beyond the safe side. Tests should call the rule evaluator on the event named in the action and assert both the live alert and incident result. `no minimum` rows test that the rule does not invent a numerical floor. Unless specified, the aircraft are different, visible, have valid routes, and no unrelated aircraft occupy a runway.

Compact phase names expand directly to the Rust enum: `Final(29L)`, `Landing(29L)`, `TakeoffRoll(29R)`, `Airborne(29R)`, `Departed(29R)`, `LineUp(29L)`, `LinedUp(29L)`, `Taxi`, `HoldShort(29L)`, `Holding`, `OnStand`, `Pushback`, `GoAround(29L)`. Position is metres from the named runway threshold or along the common taxi graph. “At boundary” is inclusive unless the rule uses strict `<`.

| rule_id | setup (aircraft, phases, positions, times) | action | expected verdict |
|---|---|---|---|
| W0 | A388 MTOW 560000 kg; A20N 79000 kg; AT76 23000 kg; D228 6400 kg | Classify wake | A388=J; A20N/AT76=M; D228=L; no alert |
| W0 | Type MTOW exactly 7000 kg, then 7001 kg; exactly 136000 kg | Classify wake | 7000=L; 7001=M; 136000=H; no alert |
| W1 | H dep 29R lifts off t=0; M dep 29R lifts off t=120 | Evaluate follower lift-off | Safe at 120 s; no alert |
| W1 | H dep 29R t=0; M dep 29R t=119.95 | Evaluate follower lift-off | Wake breach; count once |
| W1 | J leader t=0, M follower t=179.95; repeat with H leader and M follower | Evaluate lift-off | J/M breaches at <180; H/M is safe at 120 |
| W1 | M leader then M follower, both same runway, 0 s gap | Evaluate lift-off | No wake minimum; no wake alert (runway occupancy rule remains separate) |
| W1P | H on 11L lifts off t=0; M on 11R t=120; centerline 381 m | Evaluate follower lift-off | Safe at 120 s |
| W1P | H on 11L t=0; M on 11R t=119.95; centerline 381 m | Evaluate follower lift-off | Wake breach |
| W1P | H on 11L t=0; M on 09 t=0; separate systems, no crossing paths | Evaluate follower lift-off | No W1P trigger; no wake alert |
| W1X | H dep 27 t=0; M dep 28 t=120, same altitude, paths cross | Evaluate follower lift-off | Safe at 120 s |
| W1X | H dep 27 t=0; M dep 28 t=119.95, paths cross | Evaluate follower lift-off | Wake breach |
| W1X | H dep 27 t=0; M dep 28 t=119.95 but M is >300 m above leader path | Evaluate follower lift-off | No wake trigger when confirmed vertical separation exceeds 1000 ft; game currently lacks altitude, so this is an unimplemented exception test |
| W2 | J dep full length t=0; M intermediate departure t=240 | Evaluate follower lift-off | Safe at 240 s |
| W2 | H dep full length t=0; L intermediate departure t=179.95 | Evaluate follower lift-off | Wake breach; min 180 s |
| W2 | H dep full length t=0; M full-length departure t=120 | Evaluate follower lift-off | Safe at base minimum; intermediate increment does not apply |
| W2 | H dep t=0; L intermediate on a parallel runway 800 m apart with paths diverging | Evaluate follower lift-off | Intermediate rule does not apply unless same runway or parallel <760 m |
| W4 | J arrival crosses threshold 29L t=0; M departure on paired 29R lifts off t=180 | Evaluate lift-off after local threshold/TDZ geometry qualifies | Safe at 180 s |
| W4 | H arrival 29L t=0; L dep 29R lifts off t=119.95, lift-off upstream of touchdown-abeyam point | Evaluate lift-off | Wake breach; min 120 s |
| W4 | H arrival 29L t=0; L dep lifts off at or beyond touchdown-abeyam point | Evaluate lift-off | Displaced-threshold wake rule does not apply |
| W5 | H departure 29R lifts off t=0; M arrival 29L crosses threshold t=120; lift-off is upstream of TDZ abeam point | Evaluate threshold event | Safe at 120 s |
| W5 | J departure 29R t=0; L arrival 29L crosses threshold t=179.95; lift-off upstream of TDZ abeam point | Evaluate threshold event | Wake breach; min 180 s |
| W5 | H departure 11L t=0; L arrival 11R, departure lifted off beyond the local abeam point | Evaluate threshold event | W5 displacement condition false; no W5 alert |
| W6 | H missed approach on 29L begins t=0; M dep reciprocal 11R begins roll t=120 | Evaluate departure start | Safe at 120 s |
| W6 | J missed approach on 29L begins t=0; M dep reciprocal begins t=179.95 | Evaluate departure start | Breach at <180 s (MATS says “should” for A380 rows; show advisory severity) |
| W6 | H missed approach and M departure on same-direction runway | Evaluate departure start | Opposite-direction rule does not apply |
| W7 | H arrival on final t=0; M following on same final 5.00 NM behind | Evaluate spacing | Safe at 5 NM if no larger ordinary minimum applies |
| W7 | H arrival on final; M follower 4.99 NM behind, both same altitude | Evaluate spacing | Wake distance breach |
| W7 | H preceding; L follower 6.00 NM; repeat at 5.99 NM | Evaluate spacing | Safe at 6 NM; breach below 6 NM |
| W7 | H preceding; M visual approach reports leader in sight and accepts own separation | Evaluate wake rule | Wake-minimum exception applies; record caution, still apply runway occupancy and applicable ordinary minima |
| W7 | H preceding; VFR arrival follows to land on same runway | Evaluate wake rule | MATS arrival exception applies; no wake-minimum alert, issue caution |
| W8 | Surveillance arrival stream: J leader; H follower exactly 6 NM; ordinary radar minimum 3 NM | Evaluate spacing | Safe at 6 NM |
| W8 | Same pair 5.99 NM behind | Evaluate spacing | Wake minimum breach |
| W8 | At/below FL140 and within 60 NM DPN with authorized surveillance; pair whose wake minimum is 0; separation 3 NM | Evaluate ordinary radar spacing | Safe at 3 NM |
| W8 | Same authorization conditions; pair with no wake row at 2.99 NM | Evaluate ordinary radar spacing | Radar minimum breach; do not invent zero separation |
| R1 | A1 TakeoffRoll(29R) begins t=100. A2 Departed(29R) is just beyond runway end at t=100; no arrival on runway | Evaluate roll start | Safe; no alert |
| R1 | A1 TakeoffRoll(29R) begins t=100. A2 Landing(29R) remains 20 m before runway exit/holding position | Evaluate roll start | Runway occupied; alert + incident |
| R1 | Crosser HoldShort(29R) is physically behind A1's start point, outside conflict area | Evaluate roll start | No R1 alert |
| R1 | Prior departure still on runway, then changes to Departed after crossing runway end one tick later | Evaluate second roll at each tick | First attempt breaches; second is safe |
| R3 | Crosser enters runway span 20 m ahead of active TakeoffRoll(29R) | X onto runway | Runway incursion and incident |
| R3 | Crosser enters 20 m behind roller's start, moving away from conflict point | X onto runway | No incursion conflict with that roller |
| R3 | Aircraft on Final 29L is 3 NM from threshold; taxi aircraft waits short of runway | Evaluate occupancy | No incursion; not yet inside authored decision zone |
| R4 | Aircraft nose/tail fully beyond published holding position; entire body clear | Evaluate runway occupancy | Vacated; no alert |
| R4 | Aircraft still has tail over holding position, although nose passed it | Evaluate runway occupancy | Still occupying; alert when a conflicting movement is attempted |
| R4 | Aircraft nose crosses the holding line but tail remains 1 m on the taxiway side of it | Evaluate occupancy/vacated state | Still not vacated; alert if conflicting runway movement is attempted |
| R4 | Entire aircraft tail is 1 m beyond the charted holding line | Evaluate occupancy/vacated state | Vacated; no alert |
| R4 | Annex 14 fallback only; centreline distance is exactly 90 m, but no AIP holding line has been digitized | Evaluate occupancy | Do not infer along-route occupancy from the 90 m value; mark geometry unresolved, no distance-based alert |
| R5 | Arrival Final(29L) crosses threshold t=200; prior departure is beyond runway end at t=199 | Evaluate threshold crossing | Safe |
| R5 | Arrival Final(29L) crosses threshold t=200; prior departure remains 1 m before runway end and arrival is not otherwise separated | Evaluate threshold crossing | Loss of separation; if runway entry is incorrect presence, classify incursion |
| R5 | Arrival threshold crossing after prior landing aircraft fully vacates holding position | Evaluate threshold crossing | Safe |
| R6 | Arrival on 29L and departure on paired 29R; operating in dependent mode; pair is safely in-trail | Evaluate event at clearance time | Apply shared-stream constraints; no alert if required sequence gap exists |
| R6 | Arrival on 29L threshold crossing while 29R departure is still rolling/airborne before runway end | Evaluate threshold crossing | Pair-in-trail breach; alert + incident |
| R6 | Same two aircraft but mode is marked independent by AIP permission for their runway-system pairing | Evaluate event | R6 pair restriction does not apply; other runway/wake rules still apply |
| R7 | Departure on 27 and departure on 29R; published SID/non-RNAV instructions followed | Evaluate simultaneous departure | Independent-mode permission may allow it; no R7 alert |
| R7 | Departure on 27 and 28 at same time | Evaluate simultaneous departure | AIP prohibits this pair; alert |
| R7 | Arrival on 27 plus arrival on 29L, otherwise 6 NM apart | Evaluate arrival independence | Arrivals are one stream; enforce wake/ordinary spacing; no independent-arrival bypass |
| C1 | Preceding landing aircraft has crossed threshold; following Final aircraft receives C with assurance threshold gap will exist | Issue landing clearance | Allowed; no alert |
| C1 | Preceding landing aircraft has not crossed threshold; following Final receives C | Issue landing clearance | Procedural breach; log note, not an incident if safe at threshold |
| C2 | Runway obstruction appears while aircraft is cleared to land; aircraft is >2 NM out and can go around | Issue go-around | Go-around instruction required; no incident if complied with |
| C2 | Obstruction appears after threshold crossing | Evaluate go-around requirement | Too late to require go-around under this rule; R1/R5 occupancy checks govern |
| A1 | Final aircraft threshold ETA 30 s; no runway obstruction predicted at threshold | Evaluate alert | Information boundary only; no conflict alert |
| A1 | Final aircraft ETA 15 s; obstruction predicted to remain on runway at threshold | Evaluate alert | Alarm at T2 boundary (strictly inside); safe-arrival prediction suppresses if obstruction exits before threshold |
| X1 | Taxi aircraft crosses active runway with valid X clearance while no roll/arrival conflict exists | X | Permitted; no alert |
| X1 | Same crossing without X clearance, with runway clear and no imminent arrival | X | Clearance/procedural violation; no numeric time-minimum incident |
| X1 | X enters ahead of aircraft on landing roll | X | R3 incursion; incident |
| L1 | Arrival has passed runway entry point E; line-up aircraft enters behind it | L | Allowed under conditional clearance if visibility/phraseology conditions are met |
| L1 | Arrival on Final inside modeled decision zone; aircraft enters at approach end E ahead of it | L | Incursion/conflict alert |
| L2 | Traffic aircraft is identified and visible to both pilots/controller; condition is repeated/read back; line-up occurs after it passes E | Conditional L | Allowed |
| L2 | Same condition but line-up aircraft enters before traffic passes E | Conditional L | Premature entry; alert/incursion when protected runway area entered |
| L2 | No shared visual contact (scenario flag false) | Conditional L | Exception unavailable; reject condition as valid rule evidence / alert if aircraft enters |
| L3 | Aircraft remains LinedUp 29L for 120 s, runway clear, no landing conflict | Wait | No maximum-duration incident; no alert |
| L3 | Take-off clearance issued; aircraft begins roll immediately | K | Compliant |
| L3 | Take-off clearance accepted as immediate before runway entry; aircraft taxis onto runway and continues without stopping | Immediate K | Compliant continuous-movement exception |
| RR | Daylight, 5 km visibility, 300 m ceiling, tailwind 5 kt, dry braking, approved procedure, wake met; Cat 3 leader airborne 2400 m beyond follower position | Evaluate Cat 3 departure RRSM | Safe at 2400 m boundary if airport/runway authorization flag is true |
| RR | Same case but Cat 3 leader is 2399 m beyond follower position | Evaluate Cat 3 departure RRSM | RRSM breach |
| RR | Exact 2400 m but 4.9 km visibility, tailwind 6 kt, night, contaminated runway, or no local approval (test each guard separately) | Evaluate RRSM | RRSM unavailable; use standard runway separation |
| RR | RRSM attempt between departure and preceding landing aircraft | Evaluate RRSM | Prohibited exception; standard runway rule applies |
| RR | Category 1 arrival follows Cat 1/2 leader who landed, is moving to vacate without backtracking and is 600 m from threshold | Evaluate landing threshold event | Safe at 600 m boundary |
| RR | Category 2 arrival follows Cat 1/2 leader only 1499 m from threshold | Evaluate landing threshold event | RRSM breach |
| V1 | CAT II/III arrival cleared at 2.00 NM from touchdown | C | Compliant boundary |
| V1 | CAT II/III arrival at 1.99 NM with no landing clearance | Evaluate | Missed-approach instruction required; incident only if resulting separation/procedure violation occurs |
| V2 | Prior arrival has vacated LSA and pilot confirmed; prior departure airborne and has passed localizer antenna | C | Landing clearance may issue by 2 NM |
| V2 | Either condition false when C is issued | C | LVP clearance rule breach; alert |
| V3 | Arrival reaches 2 NM; taxi aircraft is outside the LSA throughout to landing-roll end | Evaluate LSA | Compliant; no alert |
| V3 | Taxi aircraft enters LSA after arrival is 2 NM and before landing roll ends | Evaluate LSA | LVP protected-area infringement; incident |
| V3 | Aircraft enters LSA before arrival reaches 2 NM, then exits before 2 NM | Evaluate LSA | No V3 breach; other surface rules still apply |
| V4 | LVP route used, taxiway intersection hold at clearance bar/stopbar | Taxi | Compliant |
| V4 | LVP active; aircraft crosses intersection hold limit without clearance | Taxi | LVP route/hold breach; alert |
| V4 | Operational alternative taxiway explicitly authorized by controller | Taxi | Allowed exception |
| G1 | Two taxi aircraft on common route are 300 m apart and closing at 10 m/s each | Continue taxi | At warning boundary; no warning if condition is strictly <300 m |
| G1 | Same route, distance 299 m and closing; then 44 m | Continue taxi | Live conflict at 299 m; incident at <45 m only |
| G1 | Two aircraft on parallel graph edges 6 m apart, no shared/converging route | Continue taxi | No G1 alert; avoids Euclidean false positive |
| G1 | Aircraft 45 m apart and closing; then 44.9 m | Continue taxi | No incident at 45 m; one incident at <45 m |
| G1 | Two stopped aircraft close, neither route is active | Wait | Excluded by authored stationary-pair exception; no alert |
| G2 | Two simultaneous pushbacks separated by one stand on a non-restricted apron | Push both | Allowed per local rule exception |
| G2 | Two pushbacks overlap in one serialized Apron 32/34/Cargo block | Push second | Alert/procedural conflict; local one-per-block rule |
| G2 | Apron I F3/F5 Code C stands; visibility >=100 m; specific AIP exception applies | Push both | Allowed exception |
