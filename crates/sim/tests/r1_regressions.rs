//! PLAN-twr.md § R1.27: one regression test for each of R1.1 to R1.4, each
//! written to fail on the code the review read, plus the clean cases R1.4's
//! fix must leave alone and R1.7's line-up direction.

mod common;

use twr_sim::airport::EdgeKind;
use twr_sim::arrival;
use twr_sim::conflict::on_runway;
use twr_sim::route::{Request, Route};
use twr_sim::scenario::{crossing, entry};
use twr_sim::traffic::{Due, Traffic};
use common::run_shift;
use twr_sim::{Aircraft, Mode, Phase, Rule, Wake, World};

fn empty() -> World {
    World::empty(1)
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

fn rwy_of(w: &World, desig: &str) -> usize {
    w.monitor.dirs.iter().find(|d| d.desig == desig).unwrap().rwy
}

fn holding_at(cs: &str, node: u32) -> Aircraft {
    Aircraft { phase: Phase::Holding, ..Aircraft::parked(cs, node) }
}

/// The taxiway node a stand's link leads to.
fn junction(w: &World, stand: u32) -> u32 {
    let e = w.airport.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == stand || e.b == stand)).unwrap();
    if e.a == stand { e.b } else { e.a }
}

// ---- R1.1 -------------------------------------------------------------------

/// Cross 10/28 with X, then T inside the runway span: the aircraft must
/// carry on off the runway, not stop on it "holding short".
#[test]
fn r1_1_a_new_route_mid_crossing_keeps_the_crossing() {
    let w0 = empty();
    let c = crossing(&w0, "28", 0.0).or_else(|| crossing(&w0, "10", 0.0)).expect("a route crossing 10/28");
    let r = rwy_of(&w0, "28");
    let mut w = empty();
    let x = w.add_taxied("CROSS", c).unwrap();
    assert_eq!(w.aircraft[x].phase, Phase::HoldShort("10/28".into()));
    w.cross(x).unwrap();
    until(&mut w, 10_000, |w| {
        let a = &w.aircraft[x];
        on_runway(a, &w.airport, &w.graph) == Some(r) && a.on_edge() && a.along_mdm > 0
    });
    // T: the rest of the same route, from the node ahead.
    let a = &w.aircraft[x];
    let rest = Route { nodes: a.route.nodes[a.leg + 1..].to_vec(), edges: a.route.edges[a.leg + 1..].to_vec() };
    w.taxi(x, rest).unwrap();
    until(&mut w, 100_000, |w| w.aircraft[x].phase != Phase::Taxi);
    let a = &w.aircraft[x];
    let on = on_runway(a, &w.airport, &w.graph);
    println!("R1.1: after T mid-crossing, {:?}, on runway {on:?}, speed {}", a.phase, a.speed_mdm);
    assert_ne!(on, Some(r), "stopped on 10/28 in {:?}", a.phase);
}

// ---- R1.2 -------------------------------------------------------------------

/// A pushing aircraft's stand is not free: no spawn onto it, no taxi to it.
#[test]
fn r1_2_a_stand_is_taken_while_its_aircraft_pushes_back() {
    let mut w = World::shift(7, Mode::Light);
    w.aircraft.clear();
    let stands: Vec<(u32, char)> = w.airport.stands.iter().map(|s| (s.node, s.code)).collect();
    // Every stand taken, then the one at `s` pushes back.
    let s = stands.iter().find(|&&(_, c)| c == 'C').unwrap().0;
    for (k, &(n, _)) in stands.iter().enumerate() {
        w.add(Aircraft::parked(&format!("P{k}"), n));
    }
    let p = w.aircraft.iter().position(|a| a.route.end() == s).unwrap();
    w.push(p).unwrap();
    let mut t = w.traffic.take().unwrap();
    t.stand_queue.push_back(Due { tick: 0, arrival: false, callsign: "NEW1".into(), type_code: "A20N", wake: Wake::M, rwy: "29R" });
    w.traffic = Some(t);
    w.step_ticks(1);
    assert_eq!(w.aircraft[p].phase, Phase::Pushback);
    let spawned: Vec<String> = w.aircraft.iter().filter(|a| a.callsign == "NEW1").map(|a| format!("NEW1 on node {}", a.route.end())).collect();
    assert!(spawned.is_empty(), "spawned onto the stand being pushed off: {spawned:?}");
    assert!(!w.stand_free(s), "stand reads free while its aircraft pushes back");

    // World::taxi takes the same test.
    let mut w = empty();
    let p = w.add(Aircraft::parked("PUSH", s));
    w.push(p).unwrap();
    w.step_ticks(1);
    let o = w.add(holding_at("TAXI", junction(&w, stands.iter().find(|&&(n, _)| n != s).unwrap().0)));
    let r = w.plan(o, s, &[], &[]).expect("a route to the stand");
    let err = w.taxi(o, r).expect_err("taxi to a stand being pushed off");
    assert!(err.contains("occupied by PUSH"), "{err}");
}

// ---- R1.3 -------------------------------------------------------------------

/// A go-around back from its circuit gets the next free slot on final even
/// with an arrival always queued for the same runway.
#[test]
fn r1_3_a_go_around_rejoins_ahead_of_a_standing_queue() {
    let mut w = World::empty(1);
    w.traffic = Some(Traffic::new(1, Mode::Practice));
    let ga = w.spawn_arrival("GA1", Wake::M, "29L").unwrap();
    w.aircraft[ga].pilot_ga = None;
    // No C: it goes around at the decision point.
    until(&mut w, 100_000, |w| matches!(w.aircraft[ga].phase, Phase::GoAround(_)) && !w.aircraft[ga].visible());
    let (mut n, mut ready) = (0, 0u32);
    while !matches!(w.aircraft[ga].phase, Phase::Final(_)) {
        let t = w.traffic.as_mut().unwrap();
        if t.final_queue.is_empty() {
            n += 1;
            t.final_queue.push_back(Due { tick: w.tick, arrival: true, callsign: format!("Q{n}"), type_code: "A20N", wake: Wake::M, rwy: "29L" });
        }
        for i in 0..w.aircraft.len() {
            if i != ga && matches!(w.aircraft[i].phase, Phase::Final(_)) && !w.aircraft[i].cleared_to_land {
                w.aircraft[i].pilot_ga = None;
                w.clear_to_land(i).unwrap();
            }
        }
        if w.aircraft[ga].circuit_left() == 0 {
            ready += 1;
            assert!(ready < 30_000, "GA1 held {ready} ticks with its circuit done; {n} queued arrivals took the slot");
        }
        w.step_ticks(1);
    }
    println!("R1.3: GA1 rejoined after {ready} ticks ready; {n} queued arrivals spawned meanwhile");
    // Was one 3 NM gap, 1,588 ticks. Since 2026-11-11 a rejoin waits for the
    // leader to clear the runway (spacing_clear.rs): measured 1,841 ticks
    // here. The bound is one leader's roll-out plus the 15 s buffer, 2,400:
    // what this test guards is that the standing queue never takes the slot
    // (the 30,000 assertion above), not the old distance.
    assert!(ready <= 2_400, "GA1 waited {ready} ticks");
}

// ---- R1.4 -------------------------------------------------------------------

/// A planned route from a stand junction to entry `e`.
fn route_to_entry(w: &World, e: u32) -> Route {
    w.airport.stands.iter()
        .find_map(|s| w.graph.plan(&w.airport, &Request { from: junction(w, s.node), to: e, ..Default::default() }))
        .expect("a route to the entry")
}

/// Add `cs` at the route's start, taxi it, and X every crossing on the way,
/// so it ends held short of the runway the route ends on.
fn held_short_at_end(w: &mut World, cs: &str, r: Route) -> usize {
    let i = w.add_taxied(cs, r).unwrap();
    while matches!(w.aircraft[i].phase, Phase::HoldShort(_)) && w.cross(i).is_ok() {
        until(w, 100_000, |w| w.aircraft[i].phase != Phase::Taxi);
    }
    assert!(matches!(w.aircraft[i].phase, Phase::HoldShort(_)), "{:?}", w.aircraft[i].phase);
    i
}

/// Two aircraft lined up on one spot: the second's line-up roll into the
/// first is a ground incident.
#[test]
fn r1_4_lining_up_into_an_occupied_spot_is_a_ground_incident() {
    let w0 = empty();
    let e = entry(&w0, "29L", true).unwrap();
    let r = route_to_entry(&w0, e);
    let mut w = empty();
    let two = held_short_at_end(&mut w, "TWO", r);
    w.add_lined_up("ONE", Wake::M, e).unwrap();
    w.line_up(two).unwrap();
    until(&mut w, 100_000, |w| matches!(w.aircraft[two].phase, Phase::LinedUp(_)));
    w.step_ticks(100);
    println!("R1.4: incidents {:?}", w.monitor.incidents.iter().map(|i| &i.text).collect::<Vec<_>>());
    assert_eq!(rules(&w), [Rule::Ground], "two aircraft on one line-up spot");
}

/// Clean: lining up behind a departure already rolling.
#[test]
fn r1_4_clean_line_up_behind_a_rolling_departure() {
    let w0 = empty();
    let e = entry(&w0, "29L", true).unwrap();
    let r = route_to_entry(&w0, e);
    let mut w = empty();
    let two = held_short_at_end(&mut w, "TWO", r);
    let one = w.add_lined_up("ONE", Wake::M, e).unwrap();
    w.takeoff(one).unwrap();
    w.line_up(two).unwrap();
    until(&mut w, 100_000, |w| matches!(w.aircraft[two].phase, Phase::LinedUp(_)) && matches!(w.aircraft[one].phase, Phase::Departed(_)));
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
}

/// Clean: two aircraft on adjacent T1 stands (101-178, the nearest pair),
/// then one pushes and taxis out past the other.
#[test]
fn r1_4_clean_adjacent_t1_stands() {
    let mut w = empty();
    let t1: Vec<(u32, String)> = w.airport.stands.iter()
        .filter(|s| s.name.get(..3).and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| (101..=178).contains(&n)))
        .map(|s| (s.node, s.name.clone()))
        .collect();
    assert!(t1.len() > 50, "{} T1 stands", t1.len());
    let at = |n: u32| {
        let p = w.airport.nodes[n as usize];
        (f64::from(p.x) / 10.0, f64::from(p.y) / 10.0)
    };
    let mut best: Option<(f64, usize, usize)> = None;
    for i in 0..t1.len() {
        for j in i + 1..t1.len() {
            let (a, b) = (at(t1[i].0), at(t1[j].0));
            let d = (a.0 - b.0).hypot(a.1 - b.1);
            if best.is_none_or(|(x, _, _)| d < x) {
                best = Some((d, i, j));
            }
        }
    }
    let (d, i, j) = best.unwrap();
    println!("R1.4: nearest T1 pair {} and {}, {d:.1} m apart", t1[i].1, t1[j].1);
    let a = w.add(Aircraft::parked("STAND1", t1[i].0));
    w.add(Aircraft::parked("STAND2", t1[j].0));
    w.step_ticks(100);
    assert!(w.monitor.incidents.is_empty() && w.monitor.conflicts.is_empty(), "parked: {:?}", w.monitor.incidents);
    w.push(a).unwrap();
    let mut seen = 0;
    until(&mut w, 100_000, |w| w.aircraft[a].phase == Phase::Holding);
    let to = junction(&w, w.airport.stands.iter().rev().find(|s| !t1.iter().any(|t| t.0 == s.node)).unwrap().node);
    let r = w.plan(a, to, &[], &[]).expect("a route away from T1");
    w.taxi(a, r).unwrap();
    while w.aircraft[a].phase == Phase::Taxi {
        w.step_ticks(1);
        seen += w.monitor.conflicts.len();
    }
    assert!(w.monitor.incidents.is_empty(), "{:?}", w.monitor.incidents);
    assert_eq!(seen, 0, "conflict ticks while STAND1 left past STAND2");
}

// ---- R1.7 -------------------------------------------------------------------

/// A departure assigned 29L lines up facing 29L from every 11R/29L entry
/// that has a 29L run ahead, never 11R into the arrivals.
#[test]
fn r1_7_line_up_faces_the_assigned_direction() {
    let w = empty();
    let r = rwy_of(&w, "29L");
    let (mut faced, mut wrong) = (0, Vec::new());
    for e in w.airport.runway_exits() {
        if w.graph.runway_at[e as usize] != Some(r) {
            continue;
        }
        let mut a = Aircraft { assigned: Some("29L"), ..holding_at("DEP", e) };
        if a.line_up(&w.airport, &w.graph).is_err() {
            continue;
        }
        match &a.phase {
            Phase::LinedUp(d) if d == "29L" => faced += 1,
            p => wrong.push(format!("node {e}: {p:?}")),
        }
    }
    println!("R1.7: {faced} entries face 29L, {} face the other way", wrong.len());
    assert!(faced > 0 && wrong.is_empty(), "{wrong:?}");
}

// ---- R1.10 ------------------------------------------------------------------

/// Same seed, same movements, whatever the stands hold: a full apron changes
/// how many stand picks a tick makes, and must not shift the schedule.
#[test]
fn r1_10_stand_picks_do_not_shift_the_schedule() {
    let run = |fill: usize| {
        let mut w = World::shift(11, Mode::Peak);
        let nodes: Vec<u32> = w.airport.stands.iter().take(fill).map(|s| s.node).collect();
        for (k, n) in nodes.into_iter().enumerate() {
            w.add(Aircraft::parked(&format!("F{k}"), n));
        }
        w.step_ticks(30 * 3600);
        let t = w.traffic.as_ref().unwrap();
        let mut all: Vec<String> = t.log.iter().chain(&t.final_queue).chain(&t.stand_queue).map(|d| format!("{} {} {} {}", d.tick, d.callsign, d.type_code, d.rwy)).collect();
        all.sort();
        all
    };
    // All but 5 stands taken: the queue holds departures the open apron places.
    let n = World::empty(1).airport.stands.len();
    let (open, full) = (run(0), run(n - 5));
    let moved = open.iter().filter(|d| !full.contains(d)).count();
    println!("R1.10: {} movements fell due on an open apron, {moved} differ with {} of {n} stands filled", open.len(), n - 5);
    assert_eq!(moved, 0);
}

// ---- R1.20, R1.24 -----------------------------------------------------------

/// R1.20 guard: verbs() on its one reused copy answers as asking each verb
/// on a fresh clone does, every 50 ticks of a controlled Peak shift.
#[test]
fn r1_20_verbs_match_a_fresh_clone_per_verb() {
    let (mut checked, mut offered) = (0, 0);
    run_shift(1, Mode::Peak, None, twr_sim::traffic::TICKS_PER_REAL_H / 2, |w: &World| {
        if !w.tick.is_multiple_of(50) {
            return;
        }
        let (ap, g) = (&w.airport, &w.graph);
        for (i, a) in w.aircraft.iter().enumerate().filter(|(_, a)| a.visible()) {
            let ok = |f: &dyn Fn(&mut Aircraft) -> Result<(), String>| f(&mut a.clone()).is_ok();
            let want: String = [
                ('P', ok(&|a| a.push(ap, g))),
                ('H', ok(&|a| a.hold())),
                ('X', ok(&|a| a.cross(ap, g))),
                ('L', ok(&|a| a.line_up(ap, g))),
                ('K', ok(&|a| a.takeoff())),
                ('C', ok(&|a| a.clear_to_land())),
                ('G', ok(&|a| a.go_around())),
            ].into_iter().filter(|x| x.1).map(|x| x.0).collect();
            let got: String = w.verbs(i).into_iter().filter(|c| "PHXLKCG".contains(*c)).collect();
            assert_eq!(got, want, "{} in {:?} at tick {}", a.callsign, a.phase, w.tick);
            checked += 1;
            offered += got.len();
        }
    });
    println!("R1.20: {checked} verb lists checked, {offered} verbs offered");
    assert!(checked > 1_000);
}

/// R1.24: E is offered exactly while some other exit would be accepted.
#[test]
fn r1_24_e_only_when_another_exit_can_be_set() {
    let mut w = World::empty(1);
    let i = w.spawn_arrival("ARR", Wake::M, "29L").unwrap();
    w.aircraft[i].pilot_ga = None;
    w.clear_to_land(i).unwrap();
    // The farthest exit: earlier ones are offered on final and drop out once
    // the roll passes their braking points. From the default exit a later
    // one always has room, so E was never withheld.
    let last = arrival::exits(&w.airport, &w.graph, "29L").into_iter().max_by_key(|(x, _)| x.at_mdm).unwrap().0.node;
    w.set_exit(i, last).unwrap();
    let (mut with_e, mut without) = (0, 0);
    while matches!(w.aircraft[i].phase, Phase::Final(_) | Phase::Landing(_)) {
        let a = &w.aircraft[i];
        let d = match &a.phase {
            Phase::Final(d) | Phase::Landing(d) => d.clone(),
            _ => unreachable!(),
        };
        let cur = a.exit.as_ref().unwrap().node;
        let can = arrival::exits(&w.airport, &w.graph, &d).iter().any(|(x, _)| x.node != cur && a.clone().set_exit(x.node, &w.airport, &w.graph).is_ok());
        let e = w.verbs(i).contains(&'E');
        assert_eq!(e, can, "tick {} {:?} pos {} m: E offered {e}, another exit settable {can}", w.tick, a.phase, a.pos_mdm / 10_000);
        if e { with_e += 1 } else { without += 1 }
        w.step_ticks(1);
    }
    println!("R1.24: E offered {with_e} ticks, withheld {without} ticks");
    assert!(with_e > 0 && without > 0);
}
