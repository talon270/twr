//! PLAN-twr.md § C3: more control on the ground. One test per verb, each at
//! its boundary: the tick a hold point stops the aircraft, the tick a give
//! way releases it, the node a stale reroute is still accepted from, the
//! distance one tick covers at each taxi speed, the runway state that
//! decides a crossing given early, and the entry where L is refused and B
//! is not.

use twr_sim::aircraft::{hold_limits, GIVE_WAY_GAP_M, MDM_PER_M, SLOW_MDM_PER_TICK, EXPEDITE_MDM_PER_TICK, TAXI_MDM_PER_TICK};
use twr_sim::airport::EdgeKind;
use twr_sim::conflict::{on_runway, GROUND_MIN_M};
use twr_sim::route::{Request, Route};
use twr_sim::{Aircraft, Phase, TaxiSpeed, World};

/// An aircraft holding at `node`, as if pushed back there.
fn holding_at(w: &mut World, callsign: &str, node: u32) -> usize {
    w.add(Aircraft { phase: Phase::Holding, ..Aircraft::parked(callsign, node) })
}

/// Step until aircraft `i` leaves `phase`; returns the ticks taken.
fn run_while(w: &mut World, i: usize, phase: Phase, max: u32) -> u32 {
    for t in 0..max {
        if w.aircraft[i].phase != phase {
            return t;
        }
        w.step_ticks(1);
    }
    panic!("{} still {:?} after {max} ticks", w.aircraft[i].callsign, w.aircraft[i].phase);
}

/// A taxi from one stand's junction to another's that crosses no runway,
/// so the whole route rolls on T alone and ends Holding, not on a stand.
fn plain_taxi(w: &mut World) -> (usize, Route) {
    let (ap, g) = (&w.airport, &w.graph);
    let junction = |s: u32| ap.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s || e.b == s)).map(|e| if e.a == s { e.b } else { e.a }).unwrap();
    let stands: Vec<u32> = ap.stands.iter().map(|s| s.node).collect();
    let (from, route) = stands
        .iter()
        .flat_map(|&a| stands.iter().map(move |&b| (a, b)))
        .find_map(|(a, b)| {
            let from = junction(a);
            let r = g.plan(ap, &Request { from, to: junction(b), ..Default::default() })?;
            (r.edges.len() >= 20 && hold_limits(ap, g, &r, 0, 0).is_empty()).then_some((from, r))
        })
        .expect("IGI has a 20-edge taxi between stands with no runway on it");
    let i = holding_at(w, "IGO512", from);
    w.taxi(i, route.clone()).unwrap();
    (i, route)
}

// ---- 1. Hold short of a point ------------------------------------------------

/// J: the aircraft stops exactly at the node, on the tick it gets there, and
/// stays. Behind it, the route's end and runway nodes are refused. H, then H
/// again, replaces the hold point: the aircraft then runs straight past it.
#[test]
fn hold_at_point_stops_exactly_at_the_node() {
    let mut w = World::empty(1);
    let (i, route) = plain_taxi(&mut w);
    w.step_ticks(3);
    let points = w.aircraft[i].hold_points(&w.graph);
    assert!(points.len() >= 8, "{} points", points.len());
    let p = points[5];
    let k = route.nodes.iter().position(|&n| n == p).unwrap();
    let at = w.aircraft[i].dist_to(k, &w.graph);

    assert_eq!(w.hold_at(i, route.nodes[0]).unwrap_err(), format!("node {} is not ahead on the route", route.nodes[0]));
    assert_eq!(w.hold_at(i, route.end()).unwrap_err(), "the route ends there: it stops there anyway");
    assert!(w.ground_verbs(i).contains(&("J", "Hold at point")));
    w.hold_at(i, p).unwrap();

    let mut last_taxi_pos = 0;
    while w.aircraft[i].phase == Phase::Taxi {
        last_taxi_pos = w.aircraft[i].pos_mdm;
        w.step_ticks(1);
    }
    let a = &w.aircraft[i];
    assert_eq!(a.phase, Phase::HoldPosition);
    assert_eq!(a.pos_mdm, at, "stops at the node, not short of it or past it");
    assert!(at - last_taxi_pos <= TAXI_MDM_PER_TICK, "moving until the tick it arrives");
    assert_eq!(a.route.nodes[a.leg], p);
    assert_eq!(a.stop, None, "a hold point is spent once reached");
    w.step_ticks(1_000);
    assert_eq!((w.aircraft[i].phase.clone(), w.aircraft[i].pos_mdm), (Phase::HoldPosition, at), "waits for Enter");

    // H twice drops a hold point set further on: it runs past it.
    let q = w.aircraft[i].hold_points(&w.graph)[2];
    w.resume(i).unwrap();
    w.hold_at(i, q).unwrap();
    w.hold(i).unwrap();
    w.hold(i).unwrap();
    assert_eq!(w.aircraft[i].stop, None);
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    assert_eq!(w.aircraft[i].phase, Phase::Holding, "ran to the end of the route");
    assert_eq!(w.aircraft[i].trail, route.edges);
    println!("hold at point: node {p}, {} m along, stopped on the tick it arrived", at / MDM_PER_M);
}

// ---- 2. Continue ---------------------------------------------------------------

/// Enter resumes after H and after a hold point, from exactly where it
/// stopped, and the path followed is still the path drawn. It refuses at
/// the route's end and held short of a runway, where X or L is the answer.
#[test]
fn continue_resumes_the_cleared_route() {
    let mut w = World::empty(1);
    let (i, route) = plain_taxi(&mut w);
    assert_eq!(w.resume(i).unwrap_err(), "nothing to continue: not holding");
    w.step_ticks(40);
    w.hold(i).unwrap();
    let held = w.aircraft[i].pos_mdm;
    w.step_ticks(200);
    assert_eq!(w.aircraft[i].pos_mdm, held);
    assert!(w.ground_verbs(i).contains(&("Enter", "Continue")));
    w.resume(i).unwrap();
    assert_eq!(w.aircraft[i].phase, Phase::Taxi);
    w.step_ticks(1);
    assert_eq!(w.aircraft[i].pos_mdm, held + TAXI_MDM_PER_TICK, "one tick on from where it stopped");

    // Enter while still moving toward a hold point drops it.
    let p = w.aircraft[i].hold_points(&w.graph)[1];
    w.hold_at(i, p).unwrap();
    w.resume(i).unwrap();
    assert_eq!(w.aircraft[i].stop, None);
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    assert_eq!(w.aircraft[i].phase, Phase::Holding);
    assert_eq!(w.aircraft[i].trail, route.edges, "the path followed is the path drawn");
    assert_eq!(w.resume(i).unwrap_err(), "nothing to continue: not holding");

    // Held short of a runway, Enter is not the answer.
    let mut w = World::empty(1);
    let (ap, g) = (&w.airport, &w.graph);
    let (from, r) = ap
        .stands
        .iter()
        .find_map(|s| {
            let r = g.plan(ap, &Request { from: s.node, to: ap.runway_exits()[0], ..Default::default() })?;
            Some((s.node, r))
        })
        .unwrap();
    let i = holding_at(&mut w, "IGO513", from);
    w.taxi(i, r).unwrap();
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    let Phase::HoldShort(rwy) = w.aircraft[i].phase.clone() else { panic!("{:?}", w.aircraft[i].phase) };
    assert_eq!(w.resume(i).unwrap_err(), format!("holding short of {rwy}: cross (X) or line up (L)"));
}

// ---- 3. Reroute mid-taxi -------------------------------------------------------

/// A route planned from the next node goes stale the tick the aircraft
/// passes that node. Still accepted when the aircraft went the route's own
/// way (cut to where it now is); refused, changing nothing, when it went
/// another way. The tick before it passes, both are accepted.
#[test]
fn reroute_planned_while_moving_survives_one_node_of_staleness() {
    // Two identical worlds: one to find the tick, one to test the tick before.
    let mut w = World::empty(1);
    let (i, r1) = plain_taxi(&mut w);
    // Node j: the first one ahead with a way on that avoids the route's own.
    let mut t = 0;
    let mut pick = None;
    while pick.is_none() {
        w.step_ticks(1);
        t += 1;
        let j = w.aircraft[i].leg + 1;
        assert!(j + 6 < r1.nodes.len(), "no junction early enough on the route");
        pick = w.plan(i, w.airport.runway_exits()[0], &[], &[r1.edges[j]]).map(|o| (j, o));
    }
    let (j, other) = pick.unwrap();
    // Same way: the old route's next edges, ending 6 nodes on.
    let same = Route { nodes: r1.nodes[j..j + 6].to_vec(), edges: r1.edges[j..j + 5].to_vec() };
    assert_eq!(other.nodes[0], same.nodes[0]);
    assert_ne!(other.edges[0], same.edges[0]);

    while w.aircraft[i].leg < j {
        w.step_ticks(1);
        t += 1;
    }
    // The tick before: both apply as planned.
    for route in [&same, &other] {
        let mut v = World::empty(1);
        let (k, _) = plain_taxi(&mut v);
        v.step_ticks(t - 1);
        assert_eq!(v.aircraft[k].leg, j - 1);
        v.taxi(k, route.clone()).unwrap();
    }
    // The tick it passes node j: on edge j now.
    assert_eq!(w.aircraft[i].leg, j);
    let before = format!("{:?}", w.aircraft[i]);
    let err = w.taxi(i, other.clone()).unwrap_err();
    assert_eq!(err, format!("route starts at node {}, aircraft continues from {}: plan again", other.nodes[0], r1.nodes[j + 1]));
    assert_eq!(format!("{:?}", w.aircraft[i]), before, "a refusal changes nothing");
    w.taxi(i, same.clone()).unwrap();
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    assert_eq!(w.aircraft[i].phase, Phase::Holding);
    assert_eq!(w.aircraft[i].route.end(), r1.nodes[j + 5]);
    assert_eq!(w.aircraft[i].trail, r1.edges[..j + 5], "the old route to node j, then the reroute's own edges");
    println!("reroute: stale by one node (node {j}) at tick {t}; same way accepted, other way refused");
}

// ---- 4. Give way ---------------------------------------------------------------

/// Two aircraft taxi into one junction, timed to meet there, and leave by
/// different taxiways. Without Y they meet (a GROUND incident). With
/// IGO601 giving way to IGO602, IGO601 stops GIVE_WAY_GAP_M short and goes
/// on the very tick IGO602 is GROUND_MIN_M past the junction: 0 incidents.
#[test]
fn give_way_waits_until_the_other_has_passed_the_junction() {
    let base = World::empty(1);
    let (ap, g) = (&base.airport, &base.graph);
    let taxi_edges = |n: u32| -> Vec<(u32, u32)> {
        if g.runway_at[n as usize].is_some() {
            return Vec::new();
        }
        let mut v: Vec<(u32, u32)> = ap
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EdgeKind::Taxiway && (e.a == n || e.b == n))
            .map(|(k, e)| (k as u32, if e.a == n { e.b } else { e.a }))
            .filter(|&(_, v)| g.runway_at[v as usize].is_none())
            .collect();
        v.sort();
        v
    };
    // Out from `n` along edge `e`, then on along the lowest-numbered taxiway
    // edge each time, never revisiting a node, until `min_m` is covered.
    let walk = |n: u32, e: u32, min_m: u64| -> Option<Route> {
        let first = taxi_edges(n).into_iter().find(|x| x.0 == e)?;
        let mut r = Route { nodes: vec![n, first.1], edges: vec![e] };
        while g.run_len_dm(&r) < min_m * 10 {
            let next = taxi_edges(r.end()).into_iter().find(|(_, v)| !r.nodes.contains(v))?;
            r.edges.push(next.0);
            r.nodes.push(next.1);
        }
        Some(r)
    };
    let join = |into: &Route, out: &Route| {
        let mut nodes: Vec<u32> = into.nodes.iter().rev().copied().collect();
        nodes.extend_from_slice(&out.nodes[1..]);
        let mut edges: Vec<u32> = into.edges.iter().rev().copied().collect();
        edges.extend_from_slice(&out.edges);
        Route { nodes, edges }
    };
    // A three-way junction: IGO601 in by e1, IGO602 in by e2, both out by e3.
    // IGO601 stops at least 150 m on; IGO602 goes 150 m further, so it is
    // never parked in front of IGO601.
    let (ra, rb) = (0..ap.nodes.len() as u32)
        .find_map(|n| {
            let out = taxi_edges(n);
            if out.len() < 3 {
                return None;
            }
            let (e1, e2, e3) = (out[0].0, out[1].0, out[2].0);
            let (a_in, b_in) = (walk(n, e1, 120)?, walk(n, e2, 120)?);
            let a_out = walk(n, e3, 150)?;
            let b_out = walk(n, e3, g.run_len_dm(&a_out) / 10 + 150)?;
            let (ra, rb) = (join(&a_in, &a_out), join(&b_in, &b_out));
            let mut all: Vec<u32> = ra.nodes.iter().chain(&b_in.nodes[1..]).copied().collect();
            all.sort();
            let unique = all.windows(2).all(|w| w[0] != w[1]) && rb.check(ap).is_ok() && ra.check(ap).is_ok();
            unique.then_some((ra, rb))
        })
        .expect("IGI has a three-way taxiway junction with 120 m of taxiway on each side");
    // The in-walks share no node, so the first node the routes share is it.
    let junction = *ra.nodes.iter().find(|n| rb.nodes.contains(n)).unwrap();
    let ka = ra.nodes.iter().position(|&n| n == junction).unwrap();
    let kb = rb.nodes.iter().position(|&n| n == junction).unwrap();
    // The farther one starts first, so both reach the junction on one tick.
    let ticks = |r: &Route, k: usize| (g.run_len_dm(&Route { nodes: r.nodes[..=k].to_vec(), edges: r.edges[..k].to_vec() }) * 1000).div_ceil(TAXI_MDM_PER_TICK) as u32;
    let (ta, tb) = (ticks(&ra, ka), ticks(&rb, kb));
    let set_up = || {
        let mut w = World::empty(1);
        let a = holding_at(&mut w, "IGO601", ra.nodes[0]);
        let b = holding_at(&mut w, "IGO602", rb.nodes[0]);
        let ((f, rf), (sc, rs)) = if ta >= tb { ((a, &ra), (b, &rb)) } else { ((b, &rb), (a, &ra)) };
        w.taxi(f, rf.clone()).unwrap();
        w.step_ticks(ta.abs_diff(tb));
        w.taxi(sc, rs.clone()).unwrap();
        (w, a, b)
    };

    // Control: no Y.
    let (mut w, a, _) = set_up();
    run_while(&mut w, a, Phase::Taxi, 1_000_000);
    let control = w.monitor.incidents.len();
    assert!(control > 0, "the set-up must conflict without Y");

    // Refusals first.
    let (mut w, a, b) = set_up();
    w.step_ticks(1);
    assert_eq!(w.give_way(a, a).unwrap_err(), "an aircraft can't give way to itself");
    let c = holding_at(&mut w, "IGO603", ap.stands[0].node);
    assert_eq!(w.give_way(a, c).unwrap_err(), "IGO603 is not moving on the ground");
    assert!(w.ground_verbs(a).contains(&("Y", "Give way")));
    w.give_way(a, b).unwrap();
    let stop_at = w.aircraft[a].dist_to(ka, &w.graph) - GIVE_WAY_GAP_M * MDM_PER_M;

    let past = |w: &World| w.aircraft[b].pos_mdm.saturating_sub(w.aircraft[b].dist_to(kb, &w.graph));
    let (mut stopped_at, mut released_at, mut t) = (None, None, 0u64);
    while w.aircraft[a].phase != Phase::Holding && t < 100_000 {
        w.step_ticks(1);
        t += 1;
        let pa = &w.aircraft[a];
        if pa.phase == Phase::HoldPosition && stopped_at.is_none() {
            stopped_at = Some(t);
            assert_eq!(pa.pos_mdm, stop_at, "stops {GIVE_WAY_GAP_M} m short of the junction");
            assert_eq!(pa.stop.as_ref().and_then(|s| s.give_way.clone()).as_deref(), Some("IGO602"));
        }
        if stopped_at.is_some() && released_at.is_none() {
            let gone = past(&w) >= GROUND_MIN_M * MDM_PER_M;
            assert_eq!(pa.phase == Phase::Taxi, gone, "tick {t}: released exactly when IGO602 is {GROUND_MIN_M} m past, {} m past now", past(&w) / MDM_PER_M);
            if gone {
                released_at = Some(t);
                assert_eq!(pa.stop, None);
            }
        }
    }
    let (s, r) = (stopped_at.expect("IGO601 stopped"), released_at.expect("IGO601 released"));
    assert_eq!(w.aircraft[a].phase, Phase::Holding);
    assert_eq!(w.aircraft[a].trail, ra.edges);
    assert_eq!(w.monitor.incidents.len(), 0, "{:?}", w.monitor.incidents);
    println!("give way at node {junction}: control {control} incident(s); stopped tick {s}, released tick {r}, 0 incidents");

    // Inside the gap it is refused: H is the instruction there.
    let (mut w, a, b) = set_up();
    let to_node = |w: &World| w.aircraft[a].dist_to(ka, &w.graph) - w.aircraft[a].pos_mdm;
    while to_node(&w) - TAXI_MDM_PER_TICK >= GIVE_WAY_GAP_M * MDM_PER_M {
        w.step_ticks(1);
    }
    assert!(to_node(&w) >= GIVE_WAY_GAP_M * MDM_PER_M);
    w.step_ticks(1);
    assert!(to_node(&w) < GIVE_WAY_GAP_M * MDM_PER_M);
    let err = w.give_way(a, b).unwrap_err();
    assert!(err.ends_with("from the junction; giving way needs 60: hold (H) instead"), "{err}");
}

// ---- 5. Taxi speed -------------------------------------------------------------

/// One tick covers 2,500, 5,000 and 7,500 mdm (5, 10, 15 m/s). The speed
/// stays through a new route, and an expedited aircraft still stops exactly
/// on its hold line.
#[test]
fn taxi_speed_sets_the_distance_per_tick() {
    let mut w = World::empty(1);
    let (i, route) = plain_taxi(&mut w);
    let step = |w: &mut World| {
        let p = w.aircraft[i].pos_mdm;
        w.step_ticks(1);
        w.aircraft[i].pos_mdm - p
    };
    assert_eq!(step(&mut w), TAXI_MDM_PER_TICK);
    assert_eq!(w.set_taxi_speed(i, TaxiSpeed::Normal).unwrap_err(), "already taxiing at normal");
    assert!(w.ground_verbs(i).contains(&("-", "Slower")) && w.ground_verbs(i).contains(&("=", "Faster")));
    w.set_taxi_speed(i, TaxiSpeed::Slow).unwrap();
    assert_eq!(step(&mut w), SLOW_MDM_PER_TICK);
    assert!(!w.ground_verbs(i).contains(&("-", "Slower")), "nothing slower than slow");
    w.set_taxi_speed(i, TaxiSpeed::Expedite).unwrap();
    assert_eq!(step(&mut w), EXPEDITE_MDM_PER_TICK);
    assert_eq!((SLOW_MDM_PER_TICK, EXPEDITE_MDM_PER_TICK), (2_500, 7_500), "5 and 15 m/s at 20 Hz");

    // A new route keeps the speed.
    let r2 = w.plan(i, route.nodes[8], &[], &[]).unwrap();
    w.taxi(i, r2).unwrap();
    assert_eq!(w.aircraft[i].taxi_speed, TaxiSpeed::Expedite);
    w.step_ticks(1);

    // On stand there is nothing to set.
    let node = w.airport.stands[3].node;
    let s = holding_at(&mut w, "IGO599", node);
    w.aircraft[s].phase = Phase::OnStand;
    assert_eq!(w.set_taxi_speed(s, TaxiSpeed::Slow).unwrap_err(), "taxi speed is for an aircraft taxiing or about to");

    // Expedite toward a runway: stops on the hold line exactly.
    let mut w = World::empty(1);
    let (ap, g) = (&w.airport, &w.graph);
    let (from, r) = ap.stands.iter().find_map(|s| Some((s.node, g.plan(ap, &Request { from: s.node, to: ap.runway_exits()[0], ..Default::default() })?))).unwrap();
    let lim = hold_limits(ap, g, &r, 0, 0)[0].0;
    let i = holding_at(&mut w, "IGO598", from);
    w.set_taxi_speed(i, TaxiSpeed::Expedite).unwrap();
    w.taxi(i, r).unwrap();
    let ticks = run_while(&mut w, i, Phase::Taxi, 1_000_000);
    assert!(matches!(w.aircraft[i].phase, Phase::HoldShort(_)));
    assert_eq!(w.aircraft[i].pos_mdm, lim, "on the line, not past it");
    assert_eq!(u64::from(ticks), lim.div_ceil(EXPEDITE_MDM_PER_TICK), "{} m at 15 m/s", lim / MDM_PER_M);
}

// ---- 6. Cross without stopping -------------------------------------------------

/// A departure whose route crosses one runway, then ends on another.
fn crossing_set_up() -> (World, usize, usize) {
    let mut w = World::empty(1);
    let (ap, g) = (&w.airport, &w.graph);
    let (from, r) = ap
        .stands
        .iter()
        .flat_map(|s| ap.runway_exits().into_iter().map(move |to| (s.node, to)))
        .find_map(|(from, to)| {
            let r = g.plan(ap, &Request { from, to, ..Default::default() })?;
            let l = hold_limits(ap, g, &r, 0, 0);
            (l.len() == 2 && l[0].1 != l[1].1).then_some((from, r))
        })
        .expect("IGI has a departure that crosses a runway");
    let i = holding_at(&mut w, "IGO520", from);
    w.taxi(i, r).unwrap();
    let rwy = w.airport.runways.iter().position(|x| x.name == hold_limits(&w.airport, &w.graph, &w.aircraft[i].route, 0, 0)[0].1).unwrap();
    (w, i, rwy)
}

/// X while taxiing toward the hold line: if no one is on the runway when
/// the aircraft reaches the line, it rolls on without a single tick held
/// short. With an aircraft lined up on that runway, it holds short there,
/// the early X lapses, and X again crosses.
#[test]
fn cross_given_early_rolls_through_a_clear_runway_and_holds_for_a_busy_one() {
    // Clear.
    let (mut w, i, rwy) = crossing_set_up();
    let name = w.airport.runways[rwy].name.clone();
    assert!(w.ground_verbs(i).contains(&("X", "Cross if clear")));
    w.cross(i).unwrap();
    assert_eq!(w.aircraft[i].cross_ahead.as_ref().map(|c| c.1.clone()), Some(name.clone()));
    assert_eq!(w.cross(i).unwrap_err(), format!("already cleared to cross {name} if clear"));
    let lim = w.aircraft[i].limit.clone().unwrap().0;
    let mut held_ticks = 0;
    while w.aircraft[i].pos_mdm <= lim + TAXI_MDM_PER_TICK {
        w.step_ticks(1);
        if matches!(w.aircraft[i].phase, Phase::HoldShort(_)) {
            held_ticks += 1;
        }
    }
    assert_eq!(held_ticks, 0, "no tick ends held short");
    assert_eq!(w.aircraft[i].phase, Phase::Taxi);
    assert_eq!(w.aircraft[i].cross_ahead, None);
    // The next runway is the one the route ends on: X ahead is refused, L is
    // the verb there, and the aircraft holds short of it as before C3.
    let err = w.cross(i).unwrap_err();
    assert!(err.starts_with("the route ends on ") && err.ends_with(": line up (L) or taxi elsewhere"), "{err}");
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    let Phase::HoldShort(next) = w.aircraft[i].phase.clone() else { panic!("{:?}", w.aircraft[i].phase) };
    assert_ne!(next, name);

    // Busy: an aircraft lined up on that runway.
    let (mut w, i, rwy) = crossing_set_up();
    let on = w.airport.runway_exits().into_iter().find(|&n| w.graph.runway_at[n as usize] == Some(rwy)).unwrap();
    let blk = w.add(Aircraft { phase: Phase::LinedUp("x".into()), ..Aircraft::parked("IGO521", on) });
    assert_eq!(on_runway(&w.aircraft[blk], &w.airport, &w.graph), Some(rwy));
    w.cross(i).unwrap();
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    assert_eq!(w.aircraft[i].phase, Phase::HoldShort(name.clone()));
    assert_eq!(w.aircraft[i].cross_ahead, None, "the early X lapsed");
    w.step_ticks(1_000);
    assert_eq!(w.aircraft[i].phase, Phase::HoldShort(name.clone()), "and needs X again");
    w.cross(i).unwrap();
    assert_eq!(w.aircraft[i].phase, Phase::Taxi);
}

// ---- 7. Backtrack ----------------------------------------------------------------

/// Entry 5 on 09/27 has 2,011 m facing 27, under the 2,030 m a medium
/// needs, so L is refused there. B rolls along the runway the other way to
/// the 27 end, on the runway the whole time, and lines up facing 27 with
/// all 2,810 m ahead; K then goes. At the end, B is refused, and a heavy is
/// refused at the entry: the whole of 09/27 is under 3,100 m.
#[test]
fn backtrack_turns_a_short_entry_into_the_full_length() {
    let mut w = World::empty(1);
    let entry = 5;
    let (ap, g) = (&w.airport, &w.graph);
    assert_eq!(ap.runways[g.runway_at[entry as usize].unwrap()].name, "09/27");
    let full = g.runway_runs(ap, entry).iter().map(|(r, _)| g.run_len_dm(r)).sum::<u64>() / 10;

    // Arrive by taxi and hold short where the route ends on 09/27.
    let from = ap.stands.iter().map(|s| s.node).find(|&s| g.plan(ap, &Request { from: s, to: entry, ..Default::default() }).is_some_and(|r| hold_limits(ap, g, &r, 0, 0).len() == 1)).unwrap();
    let r = g.plan(ap, &Request { from, to: entry, ..Default::default() }).unwrap();
    let i = w.add(Aircraft { phase: Phase::Holding, assigned: Some("27"), ..Aircraft::parked("IGO530", from) });
    w.taxi(i, r).unwrap();
    run_while(&mut w, i, Phase::Taxi, 1_000_000);
    assert_eq!(w.aircraft[i].phase, Phase::HoldShort("09/27".into()));
    assert_eq!(w.line_up(i).unwrap_err(), "only 2011 m of 27 ahead from here; need 2030");
    let mut v = World::empty(1);
    let h = v.add(Aircraft { phase: Phase::Holding, wake: twr_sim::Wake::H, assigned: Some("27"), ..Aircraft::parked("AIC531", entry) });
    assert_eq!(v.backtrack(h).unwrap_err(), format!("27 is {full} m end to end; need 3100"));
    assert!(w.ground_verbs(i).contains(&("B", "Backtrack")));

    w.backtrack(i).unwrap();
    assert_eq!(w.aircraft[i].backtrack.as_deref(), Some("27"));
    let (mut on_rwy, mut ticks) = (0, 0);
    while !matches!(w.aircraft[i].phase, Phase::LinedUp(_)) {
        w.step_ticks(1);
        ticks += 1;
        assert!(ticks < 100_000);
        if w.aircraft[i].on_runway_route(&w.airport) {
            on_rwy += 1;
            assert!(on_runway(&w.aircraft[i], &w.airport, &w.graph).is_some(), "a backtrack is on the runway");
        }
    }
    let a = &w.aircraft[i];
    assert_eq!(a.phase, Phase::LinedUp("27".into()));
    assert_eq!(a.backtrack, None);
    assert_eq!(w.graph.run_len_dm(&a.route) / 10, full, "the whole length ahead");
    assert_eq!(w.graph.runway_runs(&w.airport, a.route.nodes[0]).len(), 1, "lined up at the runway's end");
    assert!(on_rwy * TAXI_MDM_PER_TICK >= 799 * MDM_PER_M, "rolled the 799 m back along 09/27: {on_rwy} ticks");
    assert_eq!(w.backtrack(i).unwrap_err(), "already at the end of 27: line up (L)");
    w.takeoff(i).unwrap();
    println!("backtrack: {full} m of 27 after {on_rwy} ticks on the runway, {ticks} in all");
}
