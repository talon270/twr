//! PLAN-twr.md § B4, proof 1: every conflict type built on purpose and
//! caught, and the case just inside each rule left alone.

use twr_sim::aircraft::{MDM_PER_M, ROLL_TICKS};
use twr_sim::airport::EdgeKind;
use twr_sim::conflict::{on_runway, GROUND_MIN_M, GROUND_WARN_M};
use twr_sim::route::{Request, Route};
use twr_sim::scenario::{crossing, entry, route_len_m, taxi_route};
use twr_sim::{Aircraft, Phase, Rule, Wake, World};

fn empty() -> World {
    let mut w = World::new(1);
    w.aircraft.clear();
    w
}

fn rules(w: &World) -> Vec<Rule> {
    w.monitor.incidents.iter().map(|i| i.rule).collect()
}

fn until(w: &mut World, max: u32, done: impl Fn(&World) -> bool) {
    for _ in 0..max {
        if done(w) {
            return;
        }
        w.step_ticks(1);
    }
    panic!("not done after {max} ticks; incidents {:?}", w.monitor.incidents);
}

fn idx(w: &World, cs: &str) -> usize {
    w.aircraft.iter().position(|a| a.callsign == cs).unwrap()
}

fn holding_at(cs: &str, node: u32) -> Aircraft {
    Aircraft { phase: Phase::Holding, ..Aircraft::parked(cs, node) }
}

fn cum_m(w: &World, r: &Route, k: usize) -> f64 {
    r.edges[..k].iter().map(|&e| f64::from(w.graph.len_dm[e as usize])).sum::<f64>() / 10.0
}

fn suffix(r: &Route, k: usize) -> Route {
    Route { nodes: r.nodes[k..].to_vec(), edges: r.edges[k..].to_vec() }
}

fn reversed(r: &Route) -> Route {
    Route { nodes: r.nodes.iter().rev().copied().collect(), edges: r.edges.iter().rev().copied().collect() }
}

// ---- Ground (G1, G2) --------------------------------------------------------

/// Nose-to-tail into a stopped aircraft: conflict under 300 m, nothing at a
/// 46 m stop, one incident under 45 m however long the pair stays close.
#[test]
fn ground_nose_to_tail() {
    let mut w = empty();
    w.auto_stop = false;
    let r = taxi_route(&w, 900.0).expect("a long runway-free route");
    let k = (1..r.nodes.len()).find(|&k| cum_m(&w, &r, k) >= 500.0).unwrap();
    let lead_mdm = (cum_m(&w, &r, k) * 10.0) as u64 * 1000;
    w.add(holding_at("LEAD", r.nodes[k]));
    let b = w.add(holding_at("TAIL", r.nodes[0]));
    w.taxi(b, r).unwrap();

    until(&mut w, 100_000, |w| !w.monitor.conflicts.is_empty());
    let d = w.monitor.conflicts[0].d_m;
    // The first reading under 300 m lies within one step of it; 299.0 is
    // that step's lower edge (seen with B10's 270 stands, which split the route).
    assert!(d < GROUND_WARN_M as f64 && d >= GROUND_WARN_M as f64 - 1.0, "first conflict at {d:.4} m");
    assert!(w.monitor.incidents.is_empty());

    until(&mut w, 100_000, |w| lead_mdm - w.aircraft[b].pos_mdm <= 46 * MDM_PER_M);
    w.hold(b).unwrap();
    let gap = (lead_mdm - w.aircraft[b].pos_mdm) as f64 / MDM_PER_M as f64;
    w.step_ticks(200);
    assert!(w.monitor.incidents.is_empty(), "stopped {gap} m behind");
    assert!(w.monitor.conflicts.is_empty(), "both stopped: nothing live");

    w.hold(b).unwrap(); // continue
    until(&mut w, 10_000, |w| !w.monitor.incidents.is_empty());
    let i = &w.monitor.incidents[0];
    assert_eq!(i.rule, Rule::Ground);
    assert!(i.text.contains("min 45"), "{}", i.text);
    w.step_ticks(4_000);
    assert_eq!(rules(&w), [Rule::Ground], "one episode, one incident");
    println!("nose-to-tail: first conflict at {d:.1} m, held at {gap:.1} m with no incident, then one incident");
}

#[test]
fn ground_head_on() {
    let mut w = empty();
    w.auto_stop = false;
    let r = taxi_route(&w, 900.0).unwrap();
    let a = w.add(holding_at("WEST", r.nodes[0]));
    let b = w.add(holding_at("EAST", r.end()));
    w.taxi(a, r.clone()).unwrap();
    w.taxi(b, reversed(&r)).unwrap();
    until(&mut w, 100_000, |w| !w.monitor.incidents.is_empty());
    w.step_ticks(2_000);
    assert_eq!(rules(&w), [Rule::Ground]);
}

/// Converging at a junction: two routes into one node, from different
/// edges, at nearly the same distance.
#[test]
fn ground_converging_and_one_already_past() {
    let w0 = empty();
    let r = taxi_route(&w0, 900.0).unwrap();
    let mut found = None;
    'search: for k in (r.nodes.len() / 3)..(r.nodes.len() - 1) {
        let n = r.nodes[k];
        let deg = w0.airport.edges.iter().filter(|e| e.kind == EdgeKind::Taxiway && (e.a == n || e.b == n)).count();
        if deg < 3 || cum_m(&w0, &r, k) < 300.0 {
            continue;
        }
        for s in &w0.airport.stands {
            let Some(e) = w0.airport.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s.node || e.b == s.node)) else { continue };
            let j = if e.a == s.node { e.b } else { e.a };
            let req = Request { from: j, to: n, avoid: vec![r.edges[k - 1], r.edges[k]], ..Default::default() };
            let Some(q) = w0.graph.plan(&w0.airport, &req) else { continue };
            let len = route_len_m(&w0, &q);
            let clean = q.nodes.iter().all(|&x| w0.graph.runway_at[x as usize].is_none()) && !q.nodes[..q.nodes.len() - 1].iter().any(|x| r.nodes.contains(x));
            if clean && (300.0..cum_m(&w0, &r, k)).contains(&len) {
                found = Some((k, q));
                break 'search;
            }
        }
    }
    let (k, q) = found.expect("a junction on the route that another route reaches");
    let lq = route_len_m(&w0, &q);
    // Start A as far from the junction as B is.
    let i0 = (0..k).min_by_key(|&i| ((cum_m(&w0, &r, k) - cum_m(&w0, &r, i)) - lq).abs() as i64).unwrap();
    let ra = Route { nodes: r.nodes[i0..=k].to_vec(), edges: r.edges[i0..k].to_vec() };
    let la = route_len_m(&w0, &ra);
    assert!((la - lq).abs() < 30.0, "{la} vs {lq}");

    let mut w = empty();
    let a = w.add(holding_at("RTE-A", ra.nodes[0]));
    let b = w.add(holding_at("RTE-B", q.nodes[0]));
    w.taxi(a, ra.clone()).unwrap();
    w.taxi(b, q.clone()).unwrap();
    let mut warned_early = false;
    until(&mut w, 100_000, |w| !w.monitor.incidents.is_empty());
    // A converging warning fires while both are still short of the node.
    let mut w2 = empty();
    let a2 = w2.add(holding_at("RTE-A", ra.nodes[0]));
    let b2 = w2.add(holding_at("RTE-B", q.nodes[0]));
    w2.taxi(a2, ra.clone()).unwrap();
    w2.taxi(b2, q.clone()).unwrap();
    for _ in 0..100_000 {
        w2.step_ticks(1);
        let short = |x: &Aircraft| x.on_edge();
        if !w2.monitor.conflicts.is_empty() && short(&w2.aircraft[a2]) && short(&w2.aircraft[b2]) {
            warned_early = true;
            break;
        }
    }
    assert!(warned_early);
    assert_eq!(rules(&w)[0], Rule::Ground);
    let _ = (a, b);

    // Must not flag: A already 30 m past the junction, going on along r,
    // while B comes in from 250 m out on q.
    let mut w = empty();
    let a = w.add(holding_at("PAST", r.nodes[k]));
    w.taxi(a, suffix(&r, k)).unwrap();
    until(&mut w, 10_000, |w| w.aircraft[a].pos_mdm >= 30 * MDM_PER_M);
    let i1 = (0..q.nodes.len()).min_by_key(|&i| ((lq - cum_m(&w, &q, i)) - 250.0).abs() as i64).unwrap();
    let b = w.add(holding_at("LATE", q.nodes[i1]));
    w.taxi(b, suffix(&q, i1)).unwrap();
    until(&mut w, 100_000, |w| w.aircraft[b].phase == Phase::Holding);
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// Pushing back into a taxiing aircraft's path.
#[test]
fn ground_pushback_into_a_passer() {
    let w0 = empty();
    // B10: chart stands link 60-134 m out (median 79), so the junction must
    // lie more than 10 link-lengths along: a 2 km route, not 900 m.
    let r = taxi_route(&w0, 2_000.0).unwrap();
    // A stand whose junction lies on r at least 300 m along.
    let mut pick = None;
    for s in &w0.airport.stands {
        let Some(e) = w0.airport.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s.node || e.b == s.node)) else { continue };
        let j = if e.a == s.node { e.b } else { e.a };
        let link_m = f64::from(w0.graph.len_dm[w0.airport.edges.iter().position(|x| x == e).unwrap()]) / 10.0;
        if let Some(k) = r.nodes.iter().position(|&n| n == j) {
            if k + 1 < r.nodes.len() && cum_m(&w0, &r, k) > 10.0 * link_m {
                pick = Some((s.node, k, link_m));
                break;
            }
        }
    }
    let (stand, k, link_m) = pick.expect("a stand junction along the route");
    // B reaches the junction when the push is 10 m from it.
    let want = 10.0 * (link_m - 10.0).max(0.0);
    let i0 = (0..k).min_by_key(|&i| ((cum_m(&w0, &r, k) - cum_m(&w0, &r, i)) - want).abs() as i64).unwrap();
    let mut w = empty();
    w.auto_stop = false;
    let p = w.add(Aircraft::parked("PUSH", stand));
    let b = w.add(holding_at("PASS", r.nodes[i0]));
    w.push(p).unwrap();
    w.taxi(b, suffix(&r, i0)).unwrap();
    until(&mut w, 100_000, |w| !w.monitor.incidents.is_empty());
    assert_eq!(rules(&w)[0], Rule::Ground, "{:?}", w.monitor.incidents);
}

/// OSM's parallel lines: two aircraft passing on taxiways a few metres
/// apart, far apart along the network, are not a conflict.
#[test]
fn ground_parallel_lines_are_not_conflicts() {
    let w0 = empty();
    let ap = &w0.airport;
    let pt = |n: u32| (f64::from(ap.nodes[n as usize].x) / 10.0, f64::from(ap.nodes[n as usize].y) / 10.0);
    let mid = |e: usize| {
        let (a, b) = (pt(ap.edges[e].a), pt(ap.edges[e].b));
        ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
    };
    // Midpoint of one edge to the nearest point of the other, as the
    // PLAN-twr.md § B4 measurement did.
    let to_seg = |p: (f64, f64), f: usize| {
        let (a, b) = (pt(ap.edges[f].a), pt(ap.edges[f].b));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
    };
    // Same filter as the PLAN-twr.md § B4 measurement: a taxiway edge of
    // 30 m or more, another taxiway edge within 44 m of its midpoint (code C
    // taxiway separation), sharing no node, far apart along the network.
    let taxi: Vec<usize> = (0..ap.edges.len()).filter(|&e| ap.edges[e].kind == EdgeKind::Taxiway && ap.edges[e].a != ap.edges[e].b).collect();
    let near_on_net = |e: usize, f: usize| {
        let (ea, fa) = (&ap.edges[e], &ap.edges[f]);
        let via = w0.graph.plan(ap, &Request { from: ea.a, to: fa.a, ..Default::default() }).map_or(f64::MAX, |r| route_len_m(&w0, &r));
        via - (f64::from(w0.graph.len_dm[e]) + f64::from(w0.graph.len_dm[f])) / 10.0
    };
    let mut pairs = Vec::new();
    for &e in taxi.iter().filter(|&&e| w0.graph.len_dm[e] >= 300) {
        for &f in &taxi {
            let (ea, fa) = (&ap.edges[e], &ap.edges[f]);
            if e == f || [ea.a, ea.b].iter().any(|x| *x == fa.a || *x == fa.b) {
                continue;
            }
            let gap = to_seg(mid(e), f);
            if gap >= 44.0 || w0.graph.runway_at[fa.a as usize].is_some() || w0.graph.runway_at[fa.b as usize].is_some() {
                continue;
            }
            // Lower bound on the network distance between any two points of
            // the edges: beyond G1's reach, so any flag would be a false alarm.
            if near_on_net(e, f) > (GROUND_WARN_M + 20) as f64 {
                pairs.push((e, f, gap));
            }
        }
    }
    let mut closest = f64::MAX;
    for &(e, f, d) in &pairs {
        closest = closest.min(d);
        for flip in [false, true] {
            let mut w = empty();
            let (ea, fa) = (ap.edges[e].clone(), ap.edges[f].clone());
            let len = u64::from(w.graph.len_dm[e]) * 1000;
            let stop = Aircraft {
                phase: Phase::HoldPosition,
                route: Route { nodes: vec![ea.a, ea.b], edges: vec![e as u32] },
                along_mdm: len / 2,
                pos_mdm: len / 2,
                ..Aircraft::parked("STOP", ea.a)
            };
            w.add(stop);
            let (s, t) = if flip { (fa.b, fa.a) } else { (fa.a, fa.b) };
            let m = w.add(holding_at("PASS", s));
            w.taxi(m, Route { nodes: vec![s, t], edges: vec![f as u32] }).unwrap();
            until(&mut w, 100_000, |w| w.aircraft[m].phase != Phase::Taxi);
            assert!(w.monitor.incidents.is_empty() && w.monitor.conflicts.is_empty(), "edges {e}/{f} {d:.1} m apart: {:?}", w.monitor.incidents);
        }
    }
    println!("parallel lines: {} pairs, closest {closest:.1} m apart, none flagged", pairs.len());
    assert!(pairs.len() >= 20 && closest < 20.0, "{} pairs, closest {closest}", pairs.len());
}

// ---- Runway (R1-R4) ---------------------------------------------------------

fn rwy_of(w: &World, desig: &str) -> usize {
    w.monitor.dirs.iter().find(|d| d.desig == desig).unwrap().rwy
}

/// R1: K with a crosser on the runway ahead; R3: X ahead of a roll.
#[test]
fn runway_occupied_and_incursion_by_a_crosser() {
    let w0 = empty();
    let e = entry(&w0, "29R", true).unwrap();
    let c = crossing(&w0, "29R", 600.0).expect("a route crossing 29R 600 m along");
    let r = rwy_of(&w0, "29R");

    // R1: crosser first, then K.
    let mut w = empty();
    let x = w.add_taxied("CROSS", c.clone()).unwrap();
    assert!(matches!(w.aircraft[x].phase, Phase::HoldShort(_)));
    assert_eq!(on_runway(&w.aircraft[x], &w.airport, &w.graph), None, "held short is not on the runway");
    let d = w.add_lined_up("DEP", Wake::M, e).unwrap();
    w.cross(x).unwrap();
    until(&mut w, 1_000, |w| on_runway(&w.aircraft[x], &w.airport, &w.graph) == Some(r));
    assert_eq!(w.runway_blocker(d).as_deref(), Some("CROSS"));
    w.takeoff(d).unwrap();
    w.step_ticks(1);
    assert_eq!(rules(&w), [Rule::Occupied], "{:?}", w.monitor.incidents);

    // R3: K first, then X ahead of the roll.
    let mut w = empty();
    let x = w.add_taxied("CROSS", c.clone()).unwrap();
    let d = w.add_lined_up("DEP", Wake::M, e).unwrap();
    w.takeoff(d).unwrap();
    w.step_ticks(5);
    w.cross(x).unwrap();
    until(&mut w, 1_000, |w| !w.monitor.incidents.is_empty());
    assert_eq!(rules(&w), [Rule::Incursion], "{:?}", w.monitor.incidents);

    // Must not flag: X once the roll is 200 m past the crossing.
    let mut w = empty();
    let x = w.add_taxied("CROSS", c).unwrap();
    let d = w.add_lined_up("DEP", Wake::M, e).unwrap();
    w.takeoff(d).unwrap();
    let k = w.aircraft[x].route.nodes.iter().position(|&n| w.graph.runway_at[n as usize] == Some(r)).unwrap();
    let cross_pt = {
        let p = w.airport.nodes[w.aircraft[x].route.nodes[k] as usize];
        (f64::from(p.x) / 10.0, f64::from(p.y) / 10.0)
    };
    let dir = w.monitor.dirs.iter().find(|d| d.desig == "29R").unwrap().clone();
    let along = |p: (f64, f64)| (p.0 - dir.from.0) * dir.u.0 + (p.1 - dir.from.1) * dir.u.1;
    until(&mut w, 10_000, |w| along(w.aircraft[d].pos_m(&w.airport, &w.graph)) > along(cross_pt) + 200.0);
    assert!(!matches!(w.aircraft[d].phase, Phase::Departed(_)), "still on the runway");
    w.cross(x).unwrap();
    until(&mut w, 100_000, |w| matches!(w.aircraft[d].phase, Phase::Departed(_)) && !matches!(w.aircraft[x].phase, Phase::Taxi));
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// R1: an aircraft lined up behind the roll is not in its way.
#[test]
fn runway_aircraft_behind_the_roll_is_fine() {
    let mut w = empty();
    let full = entry(&w, "29R", true).unwrap();
    let mid = entry(&w, "29R", false).expect("an intermediate entry on 29R");
    w.add_lined_up("BEHIND", Wake::M, full).unwrap();
    let d = w.add_lined_up("AHEAD", Wake::M, mid).unwrap();
    assert_eq!(w.runway_blocker(d), None);
    w.takeoff(d).unwrap();
    until(&mut w, 10_000, |w| matches!(w.aircraft[d].phase, Phase::Departed(_)));
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// R1 arrival half, on a synthetic arrival: K with it on the runway is an
/// incident; K on the tick it is removed (vacated) is not.
#[test]
fn runway_departure_after_arrival() {
    for vacate in [false, true] {
        let mut w = empty();
        let e = entry(&w, "29R", true).unwrap();
        w.add(Aircraft::landing("ARR", Wake::M, e, &w.airport, &w.graph).unwrap());
        let d = w.add_lined_up("DEP", Wake::M, e).unwrap();
        w.step_ticks(20);
        assert!(w.monitor.incidents.is_empty(), "lined up behind a landing roll");
        until(&mut w, 10_000, |w| w.aircraft[0].speed_mdm == 0);
        let rolled_m = w.aircraft[0].pos_mdm as f64 / MDM_PER_M as f64;
        if vacate {
            w.aircraft.remove(0);
        }
        let d = if vacate { d - 1 } else { d };
        w.takeoff(d).unwrap();
        until(&mut w, 10_000, |w| matches!(w.aircraft[d].phase, Phase::Departed(_)) || !w.monitor.incidents.is_empty());
        if vacate {
            assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
        } else {
            assert_eq!(rules(&w), [Rule::Occupied]);
            assert!(w.monitor.incidents[0].text.contains("arrival ARR"), "{}", w.monitor.incidents[0].text);
            println!("landing roll stopped after {rolled_m:.0} m");
        }
    }
}

/// R3 on a landing roll: lined up ahead of an arrival.
#[test]
fn runway_lined_up_ahead_of_a_landing_roll() {
    let mut w = empty();
    let full = entry(&w, "29R", true).unwrap();
    let mid = entry(&w, "29R", false).unwrap();
    w.add_lined_up("AHEAD", Wake::M, mid).unwrap();
    w.add(Aircraft::landing("ARR", Wake::M, full, &w.airport, &w.graph).unwrap());
    w.step_ticks(1);
    assert_eq!(rules(&w), [Rule::Incursion], "{:?}", w.monitor.incidents);
}

/// R2: the next departure rolls while the last is still over the runway;
/// one tick after it passes the end is fine.
#[test]
fn runway_departure_after_departure() {
    for wait_for_end in [false, true] {
        let mut w = empty();
        let e = entry(&w, "29R", true).unwrap();
        let a = w.add_lined_up("FIRST", Wake::M, e).unwrap();
        w.takeoff(a).unwrap();
        let b = w.add_lined_up("SECOND", Wake::M, e).unwrap();
        if wait_for_end {
            until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Departed(_)));
        } else {
            until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Airborne(_)));
        }
        w.takeoff(b).unwrap();
        until(&mut w, 10_000, |w| matches!(w.aircraft[b].phase, Phase::Departed(_)));
        if wait_for_end {
            assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
        } else {
            assert_eq!(rules(&w), [Rule::Departure]);
        }
    }
}

// ---- Wake (W1-W3) -----------------------------------------------------------

/// Leader takes off from `lead`; once it has passed the runway end, the
/// follower lines up at `fol` and presses K `offset` ticks from the moment
/// that lifts it off exactly on the minimum. Returns the incidents and the
/// minimum in ticks (0 when no wake applies).
fn wake_case(lead: (&str, bool, Wake), fol: (&str, bool, Wake), offset: i64) -> (Vec<Rule>, u64) {
    let mut w = empty();
    let le = entry(&w, lead.0, lead.1).unwrap();
    let fe = entry(&w, fol.0, fol.1).unwrap();
    let a = w.add_lined_up("LEAD", lead.2, le).unwrap();
    w.takeoff(a).unwrap();
    let k_tick = w.tick;
    until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Airborne(_)));
    let t_air = w.tick;
    assert_eq!(t_air - k_tick, ROLL_TICKS, "every roll lifts off ROLL_TICKS after K");
    until(&mut w, 10_000, |w| matches!(w.aircraft[a].phase, Phase::Departed(_)));
    let b = w.add_lined_up("FOLLOW", fol.2, fe).unwrap();
    let wait = w.wake_wait(b);
    let req = wait.as_ref().map_or(0, |(t, _)| w.tick + t + ROLL_TICKS - t_air);
    let target = if req == 0 { w.tick } else { (t_air + req - ROLL_TICKS).checked_add_signed(offset).unwrap() };
    assert!(target >= w.tick, "follower lined up too late for the case");
    until(&mut w, 10_000, |w| w.tick == target);
    w.takeoff(b).unwrap();
    until(&mut w, 10_000, |w| matches!(w.aircraft[b].phase, Phase::Airborne(_)));
    if req > 0 {
        assert_eq!(w.tick as i64 - t_air as i64, req as i64 + offset, "lift-off gap is the minimum plus offset");
    }
    (rules(&w), req)
}

#[test]
fn wake_same_runway_each_pair_at_its_minimum() {
    use Wake::*;
    let cases = [(H, M, 120), (H, L, 120), (M, L, 120), (J, H, 120), (J, M, 180), (J, L, 180)];
    for (l, f, s) in cases {
        let (inside, req) = wake_case(("29R", true, l), ("29R", true, f), -1);
        assert_eq!(req, s * 20, "{l:?}>{f:?}");
        assert_eq!(inside, [Rule::Wake], "{l:?}>{f:?} one tick inside");
        let (on, _) = wake_case(("29R", true, l), ("29R", true, f), 0);
        assert!(on.is_empty(), "{l:?}>{f:?} on the minimum: {on:?}");
    }
    for (l, f) in [(M, M), (H, H), (L, M), (M, H), (H, J)] {
        let (none, req) = wake_case(("29R", true, l), ("29R", true, f), 0);
        assert_eq!((none.as_slice(), req), (&[][..], 0), "{l:?}>{f:?} needs no wake gap");
    }
}

#[test]
fn wake_intermediate_adds_a_minute() {
    let (inside, req) = wake_case(("29R", true, Wake::H), ("29R", false, Wake::M), -1);
    assert_eq!(req, 180 * 20);
    assert_eq!(inside, [Rule::Wake]);
    let (on, _) = wake_case(("29R", true, Wake::H), ("29R", false, Wake::M), 0);
    assert!(on.is_empty(), "{on:?}");
}

#[test]
fn wake_across_linked_runways_only() {
    // B9: no heavy can lead from 27 or 09 (2.7 km, short of the 3,100 m a
    // widebody needs), so "27 then 28" and "09 then 10" cannot happen in
    // play. The 27/28 link both ways stays in conflict.rs's geometry test.
    for (l, f) in [("29R", "29L"), ("29L", "29R"), ("28", "27")] {
        let (inside, req) = wake_case((l, true, Wake::H), (f, true, Wake::M), -1);
        assert_eq!((inside.as_slice(), req), (&[Rule::Wake][..], 2400), "{l} then {f}");
        let (on, _) = wake_case((l, true, Wake::H), (f, true, Wake::M), 0);
        assert!(on.is_empty(), "{l} then {f}: {on:?}");
    }
    for (l, f) in [("10", "09"), ("29R", "28"), ("28", "29L")] {
        let (none, req) = wake_case((l, true, Wake::H), (f, true, Wake::M), 0);
        assert_eq!((none.as_slice(), req), (&[][..], 0), "{l} then {f} are not linked");
    }
}

/// The page's scenarios each build, and each is one key away from its
/// conflict.
#[test]
fn scenarios_build_and_lead_to_their_conflict() {
    let mut w = World::scenario(7, "ground").expect("ground");
    until(&mut w, 100_000, |w| !w.monitor.incidents.is_empty());
    assert_eq!(rules(&w), [Rule::Ground]);

    let mut w = World::scenario(7, "wake").expect("wake");
    let i = idx(&w, "IGO512");
    let (left, by) = w.wake_wait(i).expect("a wake timer running");
    assert_eq!(by, "AIC101");
    assert!(left > 0);
    w.takeoff(i).unwrap();
    until(&mut w, 10_000, |w| !w.monitor.incidents.is_empty());
    assert_eq!(rules(&w), [Rule::Wake]);

    let mut w = World::scenario(7, "runway").expect("runway");
    let (x, d) = (idx(&w, "IGO701"), idx(&w, "IGO512"));
    w.takeoff(d).unwrap();
    w.cross(x).unwrap();
    until(&mut w, 10_000, |w| !w.monitor.incidents.is_empty());
    assert_eq!(rules(&w), [Rule::Incursion]);
    let _ = GROUND_MIN_M;
}

// ---- Found by the 10-hour run --------------------------------------------

/// A route that crosses a runway and later meets the same runway again must
/// stop short the second time: X clears one crossing, not the runway by name.
#[test]
fn crossing_clearance_does_not_carry_to_a_later_meeting_of_the_same_runway() {
    let w0 = empty();
    let js: Vec<u32> = w0.airport.stands.iter().filter_map(|s| {
        w0.airport.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s.node || e.b == s.node)).map(|e| if e.a == s.node { e.b } else { e.a })
    }).collect();
    let mut found = None;
    'search: for &a in &js {
        for to in w0.airport.runway_exits() {
            let Some(r) = w0.graph.plan(&w0.airport, &Request { from: a, to, ..Default::default() }) else { continue };
            let lims = twr_sim::aircraft::hold_limits(&w0.airport, &w0.graph, &r, 0, 0);
            if lims.len() >= 2 && lims[0].1 == lims[1].1 && lims[0].0 > 0 {
                found = Some((r, lims[0].1.clone()));
                break 'search;
            }
        }
    }
    let (r, name) = found.expect("IGI has a route that meets one runway twice");
    let mut w = empty();
    let i = w.add_taxied("TWICE", r).unwrap();
    assert_eq!(w.aircraft[i].phase, Phase::HoldShort(name.clone()));
    w.cross(i).unwrap();
    until(&mut w, 100_000, |w| w.aircraft[i].phase != Phase::Taxi);
    assert_eq!(w.aircraft[i].phase, Phase::HoldShort(name.clone()), "stopped short of {name} the second time");
    assert_eq!(on_runway(&w.aircraft[i], &w.airport, &w.graph), None, "and not on it");
}
