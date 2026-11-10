# Wireframes

One information-only wireframe at 1920×1080. Fixed regions are not style recommendations. Findings in braces point to the ranked claims in `REPORT.md`.

```text
1920 x 1080
+------------------------------------------------------------------------------+
| MODE / time / rate     Movements 38 of 42 | Delay 14.5 game min | Incidents 1 |
| Queue: ready 3 | runway blocker 1 | forecast conflict 1 {4, 6}               |
+----------------------------------------------------+-------------------------+
|                                                    | PUSHBACK (2)             |
|                                                    | IGO512 A20N M             |
|                                                    | ready to push · P / S     |
|                                                    |                           |
|                  AIRPORT MAP                      +-------------------------+
|                                                    | GROUND (4)                |
|   runway line pattern / taxi centerline pattern   | IGO611 B738 M             |
|   aircraft true-scale outline or min-size symbol  | taxi · next: hold H4       |
|   small labels only when they do not collide      |                           |
|   selected label has leader line + full data      +-------------------------+
|                                                    | RUNWAY (1)                |
|   forecast: amber/neutral outline + clock + pair  | IGO613 B77W H             |
|   current conflict: red ring + connector + text   | lined up · wake 0:42       |
|   incident: square + INCIDENT tag                 |                           |
|                                                    +-------------------------+
|   overlapping hit regions -> candidate chooser   | LANDINGS (2)              |
|   [IGO611 Taxi] [AXB206 Hold] [UAE14 Final]      | IGO713 Final 29L 4.2 km    |
|                                                    | requests landing · C / G  |
|                                                    +-------------------------+
|                                                    | ALERT / EVENT LOG         |
|                                                    | 00:41:12 WAKE · IGO514    |
|                                                    | 1:31 behind AIC101; min 2  |
|                                                    | [replay from event] {4}    |
+----------------------------------------------------+-------------------------+
| Selected IGO512 · push/taxi/hold/line-up/takeoff · keys and legal actions      |
| TAXI 29R VIA A D HOLD SHORT H4  [legal next: A | D | H4 | SEND] {3, 5}       |
+------------------------------------------------------------------------------+
```

- Separate score measures stay labeled with units and the offered denominator; no combined rating is introduced (Finding 6 and B7).
- Forecast, current conflict, and incident use a different text/shape state as well as color (Findings 1 and 6).
- Strip bays expose count and next action. Ordering may prioritize actionable urgency, with wait age as a tie-breaker (Findings 2 and 5).
- The selected map target can be reached through its strip; overlap opens a chooser rather than an implicit topmost hit (Finding 7).
- Dense labels are suppressed until selected/needed; runway and taxiway use line patterns in addition to hue. Palette values remain with the missing house design references (Finding 6).
