# Typed radio line grammar

This is an input grammar for the game, not a radio alphabet. The game should render an accepted command as the full standard phrase in `phrases.csv`, followed by a separate pilot readback. Keywords abbreviate standard phraseology only where a distinctive short form is unambiguous. Never send the typed abbreviation over the simulated radio.

## Root form

```ebnf
command       = callsign, SP, instruction ;
callsign      = full_callsign | approved_non_airline_abbreviation ;
instruction   = startup | push | taxi | hold | cross | lineup | takeoff
              | cancel_takeoff | stop | land | goaround | exit | continue_taxi
              | giveway | follow | contact | ready | vacated ;

startup       = STARTUP, [ APPROVED | UP, APPROVED ] ;
push          = PUSHBACK, [ APPROVED ], [ FACING, direction ] ;
taxi          = TAXI, ( runway_route | destination_route ), [ hold_short ] ;
runway_route  = [ TO, HOLDING, POINT, [ point ] ], RUNWAY, runway, [ VIA, taxiways ] ;
destination_route = [ VIA, taxiways ], TO, ( STAND, stand | point ) ;
hold_short    = HOLD, SHORT, [ OF ], RUNWAY, runway ;
hold          = HOLD, ( POSITION | AT, point | SHORT, [ OF ], RUNWAY, runway ) ;
cross         = CROSS, RUNWAY, runway, [ REPORT, VACATED ] ;
lineup        = [ BEHIND, traffic ], ( LINEUP | LINE, UP ), [ AND, WAIT ], RUNWAY, runway, [ BEHIND, traffic ] ;
takeoff       = [ CLEARED, FOR ], ( TAKEOFF | TAKE, OFF ), RUNWAY, runway ;
cancel_takeoff= CANCEL, ( TAKEOFF | TAKE, OFF ) ;
stop          = STOP, IMMEDIATELY ;
land          = [ CLEARED, TO ], LAND, RUNWAY, runway ;
goaround      = GO, AROUND ;
exit          = ( EXIT | VACATE, RUNWAY, runway ), VIA, point ;
continue_taxi = CONTINUE, [ TAXI ], [ VIA, taxiways ], [ TO, point ] ;
giveway       = GIVEWAY, TO, traffic, [ AT, point ] ;
follow        = FOLLOW, traffic, [ VIA, taxiways ], [ TO, point ] ;
contact       = [ WHEN, VACATED ], CONTACT, GROUND, frequency ;
ready         = READY, FOR, DEPARTURE ;
vacated       = RUNWAY, ( VACATED | VAC ) ;

taxiways      = taxiway, { taxiway } ;
taxiway       = phonetic_letter, [ digit ] ;
runway        = digit, digit, [ LEFT | RIGHT | CENTER ] ;
point         = identifier ;
stand         = identifier ;
traffic       = callsign | aircraft_description | vehicle_description ;
direction     = LEFT | RIGHT | NORTH | SOUTH | EAST | WEST ;
```

`HOLD SHORT` is both a standalone instruction and an optional taxi-route condition. The parser resolves it by context: at the root it stops the aircraft at its current position; after TAXI it constrains the route's clearance limit. `CROSS` is an explicit runway clearance; it must never be inferred from a taxi destination. `EXIT` is the game's routing action (“vacate via {exit}”), not a general aviation command to leave an airport. Standard phrase forms such as `CLEARED FOR TAKE-OFF RUNWAY 29L`, `CLEARED TO LAND RUNWAY 29L`, and `VACATE RUNWAY 29L VIA D6` are accepted. Slots for wind, QNH, and a radio channel can be added to the generated log but are not required command tokens. Airline callsigns of the telephony-designator-plus-flight-number type have no abbreviated form under AAI MATS Table 12-8, so `IFLY 512` is always full.

## Keywords, source phrase, and minimum safe spelling

Minimums below are the shortest prefixes unique among root command names. They are authored parser behavior, not a radio standard. The full word or full phrase is always accepted. Case and repeated whitespace are ignored; punctuation is ignored only in callsign normalization. A first letter is accepted only when it identifies one root command; do not allow `C` (CROSS/CONTACT/CANCEL) or `T` (TAXI/TAKE-OFF).

| Typed token (minimum) | Standard phrase it abbreviates | Accepted full form |
|---|---|---|
| `STA` | “start-up approved” | `STARTUP` or `START UP` |
| `P` | “pushback approved” | `PUSHBACK` |
| `TAX` | “taxi to…” | `TAXI` |
| `H` | “hold position / hold short of…” | `HOLD POSITION`, `HOLD AT`, `HOLD SHORT` |
| `CR` | “cross runway…” | `CROSS` |
| `L` | “line up and wait runway…” | `LINEUP`, `LINE UP` |
| `TAK` | “cleared for take-off runway…” | `TAKEOFF`, `TAKE-OFF` |
| `LA` | “cleared to land runway…” | `LAND` |
| `GO` | “go around” | `GO AROUND` |
| `E` | “vacate runway via…” | `EXIT`, `VACATE` (accept VACATE as the actual phrase) |
| `CONTI` | “continue taxi…” | `CONTINUE`, `CONTINUE TAXI` |
| `GI` | “give way to…” | `GIVE WAY`, `GIVEWAY` |
| `F` | “follow…” | `FOLLOW` |
| `CONTA` | “contact Delhi Ground…” | `CONTACT` |
| `CA` | “cancel take-off…” | `CANCEL TAKEOFF` |
| `STO` | “stop immediately” | `STOP IMMEDIATELY` |
| `R` | “report ready for departure” | `READY FOR DEPARTURE` |
| `VAC` (after `RUNWAY`) | “runway vacated” | `RUNWAY VACATED` |

Runway-side words map directly: `VIA` → “via taxiway”; `HOLD SHORT` → “hold short of runway”; `TO HOLDING POINT` → “taxi to holding point”; `RUNWAY`, `STAND`, `FACING`, `POSITION`, `AT`, `AND WAIT`, `TO`, `OF`, `FOR`, `REPORT`, `IMMEDIATELY`, `LEFT`, `RIGHT`, and `CENTER` remain ordinary words because shortening them saves little and increases ambiguity. Taxiway letters are accepted as chart identifiers and expanded to phonetic words only in the log.

## Legal next words for autocomplete

The parser should build candidates from current selected aircraft state as well as the grammar. The table gives syntax-level next tokens; invalid runways, routes, aircraft phases and unreachable destinations are filtered by simulation state.

| After token(s) | Legal next words / values |
|---|---|
| callsign | `STA`, `P`, `TAX`, `H`, `CR`, `L`, `TAK`, `LA`, `GO`, `E`, `CONTI`, `GI`, `F`, `CONTA`, `CA`, `STO`, `R`, `RUNWAY` (phase-legal subset only) |
| `TAXI` | `TO`, `VIA`, `RUNWAY`, `HOLDING`, `STAND`, a known point/stand |
| `TAXI TO HOLDING POINT` | known holding point; then `RUNWAY` |
| `TAXI … RUNWAY` | valid runway designator; then `VIA`, `HOLD`, end |
| `… VIA` | known taxiway identifier; then another taxiway, `HOLD`, or end |
| `… HOLD` | `SHORT`, `POSITION`, `AT` |
| `… HOLD SHORT` | `OF`, `RUNWAY`; if `OF`, then `RUNWAY` |
| `… HOLD SHORT OF RUNWAY` | valid runway designator; then end |
| `HOLD AT` | known holding point |
| `CR` / `CROSS` | `RUNWAY`; then valid runway; optional `REPORT VACATED` |
| `L` / `LINEUP` | `BEHIND`, `AND WAIT`, or `RUNWAY`; after `BEHIND`, identify traffic then `LINEUP`; after `AND WAIT`, `RUNWAY`; then valid runway; optional repeated `BEHIND` |
| `TAK` / `TAKEOFF` | `RUNWAY`; then valid runway |
| `LA` / `LAND` | `RUNWAY`; then valid runway |
| `GO` / `GO AROUND` | end |
| `P` / `PUSHBACK` | `FACING` or end; after `FACING`, direction |
| `CONTA` / `CONTACT` | `GROUND`; then one of the configured VIDP Ground frequencies |
| `GI` / `GIVE WAY` | `TO`; then aircraft/vehicle callsign; optionally `AT` plus point |
| `F` / `FOLLOW` | aircraft/vehicle callsign; optional `VIA` or `TO` |
| `R` / `READY FOR DEPARTURE` | end |
| `RUNWAY` (pilot report) | `VAC` or `VACATED`; end |

## Rewrite of the plan example

**Plan line:** `IGO512 TAXI 29L VIA A D HOLD SHORT 28`

**Radio wording:** “IFLY five one two, taxi to holding point runway two niner left via taxiway Alpha, Delta, hold short of runway two eight.” Pilot readback: “Taxi to holding point runway two niner left via taxiway Alpha, Delta, hold short of runway two eight, IFLY five one two.” The `IFLY` telephony designator comes from `out/aircraft/operators.csv`; `IGO` is the operator's ICAO code, not its spoken designator. Keep the full `IFLY 512` callsign on every exchange: AAI MATS Table 12-8 says this airline-call-sign type has no abbreviated form.

**Typed form under this grammar:** `IFLY 512 TAXI TO HOLDING POINT RUNWAY 29L VIA A D HOLD SHORT OF RUNWAY 28`

Differences from the plan's version:

1. `IGO512` conflates the ICAO operator code and flight number; use the published telephony designator `IFLY` plus `512`, and do not abbreviate this callsign type.
2. `TAXI 29L` omits the standard clearance-limit words `TO HOLDING POINT` and the runway marker `RUNWAY`.
3. `A D` must be understood as taxiway identifiers and expanded in the log to `taxiway Alpha, Delta`.
4. `HOLD SHORT 28` omits `OF RUNWAY`; runway identity must be explicit in both controller instruction and readback.
5. The original is compressed parser syntax, not something a Delhi controller says. The full phrase should be rendered to the log; a spoken `TAXI` keyword is not a substitute for `TAXI TO…`.
6. A real clearance limit may be an identified holding point rather than the runway itself. This example uses “holding point runway 29L” only because the plan gives no named holding-point slot.
