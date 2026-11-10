//! LVP (PLAN-lvp.md): the measurements and rules of low-visibility
//! procedures, run against the real sim.

use twr_sim::aircraft::{point_at, Wake, LAND_MDM_PER_TICK, MDM_PER_M};
use twr_sim::route::Route;
use twr_sim::conflict::{runway_dirs, BODY_M};
use twr_sim::lvp::Lsa;
use twr_sim::{Phase, World};

const ENDS: [&str; 6] = ["29L", "28", "29R", "11R", "10", "11L"];

/// Ticks from the threshold to (a) the vacate point and (b) the first tick
/// the whole aircraft is out of the LSA, for one cleared arrival taking the
/// default exit. Where the vacate point is inside the LSA (10/28: the hold
/// is 90 m off a runway whose LSA reaches 180 m), it is taxied on the tick
/// it stops, to the stand whose route leaves the LSA soonest: the best a
/// controller can do, so (b) is a floor, not a typical time.
fn clear_times(desig: &str, wake: Wake) -> (u64, u64, String) {
    let mut w = World::empty(1);
    let i = w.spawn_arrival("LVP1", wake, desig).expect("a landing end");
    w.clear_to_land(i).unwrap();
    let lsa = Lsa::vidp();
    let d = runway_dirs(&w.airport).into_iter().find(|d| d.desig == desig).unwrap();
    let inside = |_: &World, p| lsa.contains(&d, p, BODY_M as f64);
    let exit = w.aircraft[i].exit.clone().unwrap();
    let (mut thr, mut vac, mut entered) = (None, None, false);
    for _ in 0..40_000 {
        w.step_ticks(1);
        let p = w.aircraft[i].pos_m(&w.airport, &w.graph);
        if thr.is_none() && matches!(w.aircraft[i].phase, Phase::Landing(_)) {
            thr = Some(w.tick);
        }
        let Some(t0) = thr else { continue };
        entered |= inside(&w, p);
        if vac.is_none() && w.aircraft[i].pos_mdm >= exit.vacate_mdm {
            vac = Some(w.tick - t0);
        }
        if entered && !inside(&w, p) {
            return (vac.unwrap_or(w.tick - t0), w.tick - t0, exit.label());
        }
        // An empty airport: a crossing is cleared the tick it is reached.
        if matches!(w.aircraft[i].phase, Phase::HoldShort(_)) {
            w.cross(i).unwrap();
        }
        if w.aircraft[i].phase == Phase::Holding {
            // Metres along each stand route to the first point clear of the LSA.
            let out_at = |r: &Route| (0..).map(|k| k * 5 * MDM_PER_M).take_while(|&m| m <= route_len(&w, r)).find(|&m| !inside(&w, point_at(&w.airport, &w.graph, r, m)));
            let best = w.airport.stands.iter().filter_map(|s| w.plan(i, s.node, &[], &[])).filter_map(|r| out_at(&r).map(|m| (m, r))).min_by_key(|(m, _)| *m);
            let (_, r) = best.expect("some stand route leaves the LSA");
            w.taxi(i, r).unwrap();
        }
    }
    panic!("{desig}: never cleared the LSA");
}

fn route_len(w: &World, r: &Route) -> u64 {
    r.edges.iter().map(|&e| u64::from(w.graph.len_dm[e as usize]) * 1000).sum()
}

/// Step 2: how long a landing keeps the LSA busy, per end and wake. Prints
/// the table the LVP_FINAL_MIN_NM comment quotes.
#[test]
fn measure_lsa_clear_times() {
    let follower_2nm = |nm: f64| ((nm - 2.0) * 1852.0 * MDM_PER_M as f64 / LAND_MDM_PER_TICK as f64).ceil() as u64;
    println!("end  wake  exit       vacate  clear-of-LSA  (game ticks from the threshold, 20 per s)");
    let mut worst = 0;
    for desig in ENDS {
        for wake in [Wake::L, Wake::M, Wake::H, Wake::J] {
            let (vac, out, exit) = clear_times(desig, wake);
            println!("{desig:4} {wake:?}     {exit:10} {vac:6}  {out}");
            worst = worst.max(out);
        }
    }
    let nm = (3..=10).find(|&nm| follower_2nm(nm as f64) > worst).unwrap();
    println!("worst {worst} ticks; a follower at {nm} NM reaches 2 NM after {} ticks", follower_2nm(nm as f64));
}

/// Step 3: fog on 10% +-1 of eligible shifts, never on a gusty or windy
/// one, and the wind is the clear-weather build's wind on every seed.
#[test]
fn fog_draw_rate_and_wind_unchanged() {
    use twr_sim::lvp::{draw, LVP_MAX_WIND_KT};
    use twr_sim::traffic::Mode;
    use twr_sim::weather;
    let (mut eligible, mut fog) = (0, 0);
    for seed in 0..10_000u64 {
        let wind = weather::draw(seed);
        let ok = wind.gust.is_none() && wind.kt <= LVP_MAX_WIND_KT;
        eligible += ok as u32;
        fog += draw(seed, wind) as u32;
        assert!(ok || !draw(seed, wind), "seed {seed}: fog on {}", wind.text());
    }
    let pct = f64::from(fog) * 100.0 / f64::from(eligible);
    println!("fog on {fog} of {eligible} eligible shifts ({pct:.1}%)");
    assert!((9.0..=11.0).contains(&pct));
    for seed in 0..2_000u64 {
        let w = World::shift(seed, Mode::Normal);
        assert_eq!(w.wind(), weather::draw(seed));
        assert_eq!(w.lvp(), draw(seed, w.wind()));
    }
}

/// Distance out, metres, at which an uncleared arrival on 29L goes around.
fn go_around_at(lvp: bool) -> (f64, twr_sim::GoAround) {
    let mut w = World::empty(1);
    w.monitor.lvp = lvp;
    let i = w.spawn_arrival("LVP1", Wake::M, "29L").unwrap();
    let mut last = w.aircraft[i].final_m();
    while matches!(w.aircraft[i].phase, Phase::Final(_)) {
        last = w.aircraft[i].final_m();
        w.step_ticks(1);
    }
    (last, w.aircraft[i].went_around.unwrap())
}

/// Step 4, V1: the deadline is 2 NM under LVP, 1,160 m otherwise, to
/// within one tick of travel (3.5 m).
#[test]
fn landing_clearance_deadline_moves_to_2_nm() {
    use twr_sim::GoAround;
    let (on, why_on) = go_around_at(true);
    let (off, why_off) = go_around_at(false);
    println!("LVP on: around at {on:.1} m ({why_on:?}); off: {off:.1} m ({why_off:?})");
    assert_eq!((why_on, why_off), (GoAround::NoClearance, GoAround::NoClearance));
    assert!(on <= 3704.0 && on > 3704.0 - 3.5, "{on}");
    assert!(off <= 1160.0 && off > 1160.0 - 3.5, "{off}");
}

// ---- Steps 5 and 6: the LSA in the conflict model and the score ---------------

use twr_sim::traffic::Mode;
use twr_sim::weather::{Flow, Plan};
use twr_sim::{GoAround, Rule};

/// A Normal shift that spawns nothing (its runway plan is empty, so every
/// movement it schedules waits without a runway), with LVP set, and A
/// landed on 29L and held at its vacate point: Z2, inside the 29L LSA.
fn a_held_on_z(lvp: bool) -> (World, usize) {
    let mut w = World::shift(3, Mode::Normal);
    w.traffic.as_mut().unwrap().schedule.custom = Some(Plan { flow: Flow::West, land: vec![], depart: vec![] });
    w.monitor.lvp = lvp;
    let a = w.spawn_arrival("AAA1", Wake::M, "29L").unwrap();
    w.clear_to_land(a).unwrap();
    while w.aircraft[a].phase != Phase::Holding {
        w.step_ticks(1);
    }
    (w, a)
}

fn count(w: &World, rule: Rule) -> usize {
    w.monitor.incidents.iter().filter(|x| x.rule == rule).count()
}

/// B on final for 29L, cleared, flown to the end of its landing roll.
fn land_b(w: &mut World) -> usize {
    let b = w.spawn_arrival("BBB2", Wake::M, "29L").unwrap();
    assert!(w.aircraft[b].pilot_ga.is_none(), "seed draws a pilot go-around for B");
    w.clear_to_land(b).unwrap();
    while !matches!(w.aircraft[b].phase, Phase::Taxi | Phase::Holding) {
        w.step_ticks(1);
    }
    b
}

/// V3: A held on Z as B comes inside 2 NM is one LSA incident and no R5;
/// the same move without LVP is nothing.
#[test]
fn held_on_z_is_one_lsa_incident_and_only_under_lvp() {
    let (mut w, _) = a_held_on_z(true);
    land_b(&mut w);
    println!("{:?}", w.monitor.incidents.iter().map(|x| &x.text).collect::<Vec<_>>());
    assert_eq!((count(&w, Rule::Lsa), count(&w, Rule::Occupied), w.monitor.incidents.len()), (1, 0, 1));
    assert_eq!(w.monitor.incidents[0].text, "LSA · AAA1 in the 29L localizer sensitive area with BBB2 inside 2 NM");
    let (mut w, _) = a_held_on_z(false);
    land_b(&mut w);
    assert_eq!(w.monitor.incidents.len(), 0);
}

/// V3 at the boundary: A taxis out of the LSA on tick T. B first inside
/// 2 NM on T + 1 logs nothing; on T, one incident.
#[test]
fn leaving_the_lsa_one_tick_before_2_nm_is_clear() {
    // Where A leaves, with no B: taxi it to the stand whose route leaves soonest.
    let taxi_out = |w: &mut World, a: usize| {
        let r = w.airport.stands.iter().filter_map(|s| w.plan(a, s.node, &[], &[])).min_by_key(|r| r.edges.len()).unwrap();
        w.taxi(a, r).unwrap();
    };
    let (mut w, a) = a_held_on_z(true);
    let t0 = w.tick;
    taxi_out(&mut w, a);
    while w.monitor.lsa_occupant(&w.aircraft, &w.airport, &w.graph, "29L", None).is_some() {
        w.step_ticks(1);
        if matches!(w.aircraft[a].phase, Phase::HoldShort(_)) {
            w.cross(a).unwrap();
        }
    }
    let out = w.tick - t0;
    // Taxi A at step T. B spawned before step S is first inside 2 NM after
    // step S + 1227; A is first out after step T + out - 1.
    const T: u64 = 1228;
    let run = |s_at: u64| {
        let (mut w, a) = a_held_on_z(true);
        let mut b = None;
        for k in 0..T + out + 10 {
            if k == s_at {
                let i = w.spawn_arrival("BBB2", Wake::M, "29L").unwrap();
                w.clear_to_land(i).unwrap();
                b = Some(i);
            }
            if k == T {
                taxi_out(&mut w, a);
            }
            w.step_ticks(1);
            if matches!(w.aircraft[a].phase, Phase::HoldShort(_)) {
                w.cross(a).unwrap();
            }
        }
        (count(&w, Rule::Lsa), b.unwrap())
    };
    let (late, _) = run(T + out - 1 - 1227);
    let (on_time, _) = run(T + out - 2 - 1227);
    println!("A leaves the LSA {out} ticks into its taxi. B inside 2 NM the tick A is out: {late} incident(s); one tick earlier: {on_time}");
    assert_eq!((late, on_time), (0, 1));
}

/// A7: under LVP, B's strip stops asking for C while A is in the LSA, and
/// the note names A. With A gone it asks again.
#[test]
fn final_strip_waits_while_the_lsa_is_busy() {
    let (mut w, a) = a_held_on_z(true);
    let b = w.spawn_arrival("BBB2", Wake::M, "29L").unwrap();
    w.step_ticks(1);
    assert!(!w.requesting(b));
    assert_eq!(w.lsa_blocker(b).as_deref(), Some("AAA1"));
    w.aircraft[a].phase = Phase::ShutDown;
    assert!(w.requesting(b));
    assert_eq!(w.lsa_blocker(b), None);
}

/// Step 6, A3: a correctly withheld C at 2 NM goes around as "LSA
/// occupied": no delay for B, no challenge, one movement short. A
/// forgotten C with a clear LSA is still "no landing clearance" and delay.
#[test]
fn withheld_clearance_is_not_your_delay() {
    let (mut w, _) = a_held_on_z(true);
    let b = w.spawn_arrival("BBB2", Wake::M, "29L").unwrap();
    for _ in 0..3_000 {
        w.step_ticks(1);
    }
    assert_eq!(w.aircraft[b].went_around, Some(GoAround::LsaOccupied));
    assert_eq!(w.go_arounds.last().unwrap().why.text(), "LSA occupied");
    assert_eq!(w.score.waits.get("BBB2"), None);
    assert_eq!((w.challenges(), w.score.movements), (0, 1));
    assert_eq!(w.monitor.incidents.len(), 0, "a withheld C is no incident");

    // LVP on, LSA clear, C forgotten.
    let (mut w, a) = a_held_on_z(true);
    w.aircraft[a].phase = Phase::ShutDown;
    let b = w.spawn_arrival("BBB2", Wake::M, "29L").unwrap();
    for _ in 0..3_000 {
        w.step_ticks(1);
    }
    assert_eq!(w.aircraft[b].went_around, Some(GoAround::NoClearance));
    assert!(w.score.waits.get("BBB2").copied().unwrap_or(0) > 0);
}

// ---- Step 7: the LVP taxi routes in the planner ---------------------------------

/// Taxiway names along a route, consecutive repeats and unnamed lanes dropped.
fn names(w: &World, r: &Route) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for &e in &r.edges {
        let n = &w.airport.edges[e as usize].name;
        if !n.is_empty() && out.last() != Some(n) {
            out.push(n.clone());
        }
    }
    out
}

/// V4: from Z2 on 29L to every stand, LVP never takes more off-route
/// taxiway than the clear-weather planner, and takes less on some. A `via`
/// off the set still wins. The cost works on names, not on the table's
/// order: Z2 to stand 150 already runs Z, R5, Q, A, N7, K6, D, all
/// published names, with LVP on or off.
#[test]
fn lvp_prefers_the_published_names_and_via_still_wins() {
    let (w, a) = a_held_on_z(true);
    let (w_off, a_off) = a_held_on_z(false);
    let mask = twr_sim::lvp::off_route_mask(&w.airport, Flow::West);
    let off_m = |r: &Route| r.edges.iter().filter(|&&e| mask[e as usize]).map(|&e| u64::from(w.graph.len_dm[e as usize])).sum::<u64>() / 10;
    let (mut better, mut same, mut total_on, mut total_off) = (0, 0, 0, 0);
    for st in &w.airport.stands {
        let (Some(on), Some(off)) = (w.plan(a, st.node, &[], &[]), w_off.plan(a_off, st.node, &[], &[])) else { continue };
        let (m_on, m_off) = (off_m(&on), off_m(&off));
        assert!(m_on <= m_off, "stand {}: {m_on} m off-route under LVP, {m_off} m without", st.name);
        if m_on < m_off { better += 1 } else { same += 1 }
        total_on += m_on;
        total_off += m_off;
    }
    println!("stands from Z2: {better} routes with less off-route taxiway under LVP, {same} the same; off-route total {total_off} m clear, {total_on} m LVP");
    assert!(better > 0);
    let stand = w.airport.stands.iter().find(|s| s.name == "150").expect("stand 150").node;
    println!("Z2 to 150 under LVP: {:?}", names(&w, &w.plan(a, stand, &[], &[]).unwrap()));
    // R1 is not in the west set; a via on it is honoured.
    let r1 = w.airport.edges.iter().position(|e| e.name == "R1").unwrap() as u32;
    assert!(w.plan(a, stand, &[r1], &[]).unwrap().edges.contains(&r1));
}
