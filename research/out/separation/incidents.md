# Incident model and severity

Use two labels in the sim: **loss of separation** for a quantified minimum breach that is not a runway incursion, and **runway incursion** for incorrect presence on the runway protected area. Keep a live conflict alert separate from both; an alert is not a counted incident unless the model's breach condition actually occurs.

## Severity mapping

Doc 9870 Attachment 7, Table 6-1 defines severity classes by proximity and available reaction time: A, serious incident with collision narrowly avoided; B, separation decreases with significant collision potential and likely time-critical corrective/evasive action; C, ample time/distance remains; D, incursion with no immediate safety consequences; E, evidence insufficient or inconclusive. The sim can assign classes only from its deterministic simulated geometry; it cannot infer a real-world causal investigation result.

| Event | Game classification | Rule trigger | Suggested severity | Surge consequence |
|---|---|---|---|---|
| Aircraft crosses into runway protected area without valid clearance while a landing/takeoff roll is active or imminent | Runway incursion | R3, R4/R5, X1, L1/L2 | D if no immediate threat; C if ample distance; B if time-critical correction required; A only if collision narrowly avoided | End immediately for any confirmed incursion |
| Aircraft enters runway correctly cleared but crosses ahead of a rolling aircraft | Runway incursion | R3 | B/A based on simulated closest approach; otherwise C/D | End immediately |
| Takeoff roll begins before leader crossed runway end / arrival vacated | Loss of separation; incursion only if the aircraft's presence itself meets incorrect-presence definition | R1/R2 | B if little reaction margin; C if ample; A only on near collision | Counts as one incident; end at incident three, unless also an incursion |
| Arrival crosses threshold before preceding departure has cleared or landing aircraft vacated | Loss of separation; potentially incursion where runway is incorrectly occupied | R5/R6 | B/C; A only for narrowly avoided collision | Counts; incursion ends immediately |
| Wake time/distance minimum breached | Wake loss of separation, not a runway incursion | W1–W8 | C if separated and no immediate hazard; B if a close encounter is explicitly simulated; A only if narrowly avoided | Counts once per movement pair; ends at incident three |
| Taxi route conflict below authored threshold | Surface loss of separation / game conflict, not a runway incursion | G1/G2 | C for 45 m breach with time/distance remaining; B only with time-critical avoidance; A only on near collision | Counts once per episode; ends at incident three |
| LVP LSA / stopbar / low-visibility-route infringement | Surface incursion or procedural breach depending on protected area | V2–V4 | D absent immediate hazard; C/B by available margin; A only on narrow avoidance | Count if a safety boundary is crossed; runway incursion ends immediately |
| Landing clearance issued too early but required spacing still exists at threshold | Procedural deviation, not loss of separation | C1 | Not A–E absent incursion; record as procedural note | Does not count as safety incident |
| Required go-around instruction omitted, but no minimum is yet breached | Procedural deviation; if threshold crossed into occupied runway, classify R5/incursion as applicable | C2 | No A–E until a safety occurrence; later severity follows event | Counts only if a separation breach/incursion occurs |
| Unknown/conflicting state data prevents severity classification | Unclassified occurrence | Any | E | Counts as incident if a known rule breach exists; does not invent an A–D rating |

## Surge shift recommendation

End Surge on the first confirmed runway incursion. This is the highest-consequence tower-surface event in scope, directly tests PLAN § Phase 0's “conflict is never missed” promise, and is a clear player-facing failure condition. Do not end on a harmless procedural note or every wake-separation breach: the source rules distinguish these from incursions, and a wake or ground breach can have remaining reaction margin. Keep the plan's three-incident cap for other confirmed safety incidents, but an incursion ends the run immediately. This resolves PLAN § Open questions in favour of “first runway incursion,” with severity class A/B/C/D shown in the log and raw count preserved.

## Counting rules

- Count once per movement pair/clearance episode, not once per tick. A continuing breach must not inflate the incident total.
- Record actual and minimum values, game time, aircraft, rule ID, runway or taxi route, and whether the aircraft was still controllable.
- Never assign Doc 9870 class A solely because a distance threshold was crossed; A means a collision was narrowly avoided. If the model lacks enough reaction-time/proximity evidence, classify E or keep the game event as an ungraded loss of separation.
- A runway incursion severity label is not itself a probability. The class reflects the incident's simulated outcome and available response, not the rule violated.
