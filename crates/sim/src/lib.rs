//! twr-sim — the deterministic simulation core.
//!
//! No web dependencies. Everything here runs under `cargo test` with no
//! browser, which is how routing, conflicts and pace get verified by running
//! them rather than by reading them.
//!
//! Determinism is a contract: the same seed and the same command log replay
//! the same shift, tick for tick. Nothing in this crate may read wall-clock
//! time, use floating-point accumulation across frames, or draw randomness
//! from anywhere but `Rng`.
//!
//! B0: clock and RNG. B1: the airport graph (`airport`). B2: the planner
//! (`route`). B3: the departure flow (`aircraft`). B4: the conflict model
//! (`conflict`) and named set-ups (`scenario`). B5: arrivals (`arrival`).
//! B6: traffic (`traffic`). B7: the score and the end of a shift (`score`).

pub mod aircraft;
pub mod airport;
pub mod arrival;
pub mod conflict;
pub mod route;
pub mod scenario;
pub mod score;
pub mod traffic;
pub mod weather;

pub use aircraft::{Aircraft, Phase, Wake};
pub use arrival::GoAround;
pub use conflict::{Incident, Monitor, Rule};
pub use score::Score;
pub use traffic::{Mode, Traffic};
use airport::Airport;
use route::{Graph, Request, Route};

/// Sim ticks per second of game time. At 20 Hz an aircraft taxiing at 30 kt
/// moves 0.77 m per tick, well under any spacing rule we will check.
pub const TICK_HZ: u32 = 20;

/// Game time per real time, in thousandths, applied to movement only.
/// 3000 = 3x. Traffic rates are counted per real hour (PLAN-twr.md § A2), so
/// this shortens taxi time without shortening the gap between decisions.
/// An integer because the clock below must not accumulate float error.
pub const TIME_COMPRESSION_MILLI: u64 = 3000;

/// Real time a single frame may contribute, in microseconds. A tab returning
/// from the background hands us seconds of elapsed time; replaying all of it
/// at once would teleport aircraft through each other and past every conflict
/// check. Anything above this is dropped, which reads as a brief stall.
pub const MAX_FRAME_REAL_US: u64 = 250_000;

/// Length of one tick in game nanoseconds: 50 ms at 20 Hz, exact.
const TICK_GAME_NS: u64 = 1_000_000_000 / TICK_HZ as u64;

/// Fixed-step clock. Real frame time goes in, whole ticks come out, and the
/// remainder carries to the next frame so no time is lost or invented.
///
/// The carry is integer game nanoseconds. The first version carried float
/// milliseconds and turned 1000 ms at 3x into 59 ticks instead of 60, because
/// 1000/60 has no exact float form; integers make that impossible.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    ticks: u64,
    carry_game_ns: u64,
}

impl Clock {
    /// Feed one frame of real elapsed time in milliseconds (what
    /// requestAnimationFrame gives); returns how many ticks to step. The
    /// input is rounded to whole microseconds, the only float step here.
    pub fn advance(&mut self, real_ms: f64) -> u32 {
        let real_us = if real_ms.is_finite() && real_ms > 0.0 {
            ((real_ms * 1000.0).round() as u64).min(MAX_FRAME_REAL_US)
        } else {
            0
        };
        // real_us * (milli-compression) = game time in nanoseconds.
        self.carry_game_ns += real_us * TIME_COMPRESSION_MILLI;
        let steps = self.carry_game_ns / TICK_GAME_NS;
        self.carry_game_ns %= TICK_GAME_NS;
        self.ticks += steps;
        steps as u32
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Game time since the shift started, from the integer tick count, so it
    /// never drifts from the sim.
    pub fn game_seconds(&self) -> f64 {
        self.ticks as f64 / f64::from(TICK_HZ)
    }
}

/// SplitMix64. Small, fast, and identical on every platform, which is all a
/// replayable game needs; nothing here is security-sensitive.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1), from the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The page's one arrival lands here until the `vidp` research says which
/// runway IGI lands on in each flow (PLAN-twr.md § B5). `authored`.
pub const PAGE_ARRIVAL_RWY: &str = "29L";

/// One go-around, for the strip and, in B7, the score: challenges are
/// counted apart from delay you caused.
#[derive(Debug, Clone, PartialEq)]
pub struct GoAroundLog {
    pub tick: u64,
    pub callsign: String,
    pub why: GoAround,
}

/// IGI's route graph, baked by `cargo run -p twr-osm -- VIDP`. BLR and BOM
/// join in B8.
pub const VIDP: &str = include_str!("../../../data/vidp.twr");

/// The whole simulation state. Everything the screen shows is read from here.
pub struct World {
    pub seed: u64,
    pub clock: Clock,
    rng: Rng,
    pub airport: Airport,
    pub graph: Graph,
    pub aircraft: Vec<Aircraft>,
    /// Ticks stepped since the shift began. Wake timers and the incident
    /// log run on this, so tests (which skip the clock) and the page agree.
    pub tick: u64,
    pub monitor: Monitor,
    pub go_arounds: Vec<GoAroundLog>,
    /// B6: the traffic stream; None in Practice and in tests' worlds.
    pub traffic: Option<Traffic>,
    /// B7: movements, delay, and the tick the shift ended.
    pub score: Score,
    /// The world stops on the tick the shift ends (B7). Long-run tests turn
    /// this off to watch the controller finish what the shift gave it.
    pub halt_on_end: bool,
}

impl World {
    /// Practice: IGO512 on a seeded stand and IGO713 on final for 29L, the
    /// B5 world every verify script from B2 on opens.
    pub fn new(seed: u64) -> Self {
        let mut w = World::empty(seed);
        let stand = w.airport.stands[(w.rng.next_u64() % w.airport.stands.len() as u64) as usize].node;
        w.aircraft.push(Aircraft::parked("IGO512", stand));
        // And one arrival on final, drawn after the stand so B0-B4 seeds
        // still pick the same stand.
        w.spawn_arrival("IGO713", Wake::M, PAGE_ARRIVAL_RWY).expect("29L has a default exit; arrival tests guard this");
        w
    }

    /// IGI with no aircraft and no traffic.
    pub fn empty(seed: u64) -> Self {
        let airport = Airport::parse(VIDP).expect("baked VIDP parses; airport tests guard this");
        let graph = Graph::new(&airport);
        let monitor = Monitor::new(&airport);
        World { seed, clock: Clock::default(), rng: Rng::new(seed), airport, graph, aircraft: Vec::new(), tick: 0, monitor, go_arounds: Vec::new(), traffic: None, score: Score::default(), halt_on_end: true }
    }

    /// Put an arrival on final for `desig`, with its pilot go-around drawn.
    pub fn spawn_arrival(&mut self, callsign: &str, wake: Wake, desig: &str) -> Option<usize> {
        let a = Aircraft::on_final(callsign, wake, desig, &self.airport, &self.graph)?;
        let i = self.add(a);
        self.draw_pilot_ga(i);
        Some(i)
    }

    /// Each time an aircraft comes onto final: does its pilot go around,
    /// where, and why. Three draws from the one stream, every time, so the
    /// stream does not depend on the outcome.
    fn draw_pilot_ga(&mut self, i: usize) {
        let (p, at, why) = (self.rng.next_u64() % 100, self.rng.next_u64(), self.rng.next_u64());
        let span = arrival::PILOT_GA_MAX_M * aircraft::MDM_PER_M - aircraft::LAND_MDM_PER_TICK;
        self.aircraft[i].pilot_ga = (p < arrival::PILOT_GA_PCT).then(|| {
            (aircraft::LAND_MDM_PER_TICK + at % span, arrival::PILOT_REASONS[(why % arrival::PILOT_REASONS.len() as u64) as usize])
        });
        // B9 windshear: drawn on gusty shifts only, so a steady shift's
        // stream is the one B5-B7 tested, draw for draw.
        self.aircraft[i].weather_ga = None;
        if self.wind().gust.is_some() {
            let (p, at) = (self.rng.next_u64() % 100, self.rng.next_u64());
            if self.aircraft[i].pilot_ga.is_none() && p < weather::WINDSHEAR_GA_PCT {
                self.aircraft[i].weather_ga = Some(aircraft::LAND_MDM_PER_TICK + at % span);
            }
        }
    }

    /// Advance by one frame of real time. Returns the ticks stepped.
    pub fn frame(&mut self, real_ms: f64) -> u32 {
        // An ended shift stops its clock too, or the game clock on screen
        // would run on over a frozen map.
        if self.halt_on_end && self.score.over.is_some() {
            return 0;
        }
        let steps = self.clock.advance(real_ms);
        self.step_ticks(steps);
        steps
    }

    /// Step whole ticks with no clock involved; the frame loop and tests
    /// share this, so a test that fast-forwards runs the same movement code.
    /// After every tick the conflict monitor checks the new positions, and
    /// each lift-off gets its wake check on the tick it happens.
    pub fn step_ticks(&mut self, n: u32) {
        for _ in 0..n {
            // A score that changes after "Shift over" is not a score.
            if self.halt_on_end && self.score.over.is_some() {
                return;
            }
            self.tick += 1;
            let (mut landed, mut on_final) = (Vec::new(), Vec::new());
            // Go-arounds whose circuit is done rejoin final only into a free
            // slot (A12), judged one at a time so two cannot take the same one.
            let mut rejoin = Vec::new();
            for i in 0..self.aircraft.len() {
                let a = &self.aircraft[i];
                let Phase::GoAround(d) = &a.phase else { continue };
                if a.circuit == 0 && !a.visible() && traffic::final_slot_free(&self.aircraft, d, a.wake) {
                    let d = d.clone();
                    self.aircraft[i].back_on_final(&d);
                    rejoin.push(i);
                }
            }
            for (i, a) in self.aircraft.iter_mut().enumerate() {
                let was = matches!(a.phase, Phase::Airborne(_));
                let was_final = matches!(a.phase, Phase::Final(_));
                // A rejoin is back on final already and does not move this tick,
                // as before A12.
                let was_ga = matches!(a.phase, Phase::GoAround(_)) || rejoin.contains(&i);
                if !rejoin.contains(&i) {
                    a.step(&self.airport, &self.graph);
                }
                if !was && matches!(a.phase, Phase::Airborne(_)) {
                    self.score.movements += 1;
                    let at = a.pos_m(&self.airport, &self.graph);
                    self.monitor.on_airborne(&a.callsign, self.tick, at);
                }
                match (&a.phase, was_final, was_ga) {
                    (Phase::Landing(_), true, _) => landed.push(i),
                    (Phase::GoAround(_), true, _) => {
                        let why = a.went_around.expect("a go-around has a cause");
                        self.go_arounds.push(GoAroundLog { tick: self.tick, callsign: a.callsign.clone(), why });
                    }
                    (Phase::Final(_), _, true) => on_final.push(i),
                    _ => {}
                }
            }
            for i in on_final {
                self.draw_pilot_ga(i);
            }
            // R5 before the per-tick check, so the roll that follows is
            // not counted a second time for the same pair.
            self.score.movements += landed.len() as u32;
            for i in landed {
                self.monitor.on_threshold(&self.aircraft, i, &self.airport, &self.graph, self.tick);
            }
            self.monitor.check(&self.aircraft, &self.airport, &self.graph, self.tick);
            self.traffic_tick();
            self.score_tick();
        }
    }

    /// Plan a route for aircraft `i` from where a new clearance must start.
    /// The screen draws the result; `taxi` takes it back unchanged.
    pub fn plan(&self, i: usize, to: u32, via: &[u32], avoid: &[u32]) -> Option<Route> {
        let from = self.start_node(i);
        let req = Request { from, to, via: via.to_vec(), avoid: avoid.to_vec() };
        self.graph.plan(&self.airport, &req)
    }

    /// Where a taxi route for aircraft `i` begins. A lined-up aircraft
    /// leaves from its runway entry, not from the run ahead of it.
    pub fn start_node(&self, i: usize) -> u32 {
        let a = &self.aircraft[i];
        match a.phase {
            Phase::LinedUp(_) => a.route.nodes[0],
            _ => a.next_node(),
        }
    }

    /// Where a planned route would stop short of runways, metres, assuming
    /// no clearance yet: the bars the preview draws.
    pub fn hold_marks(&self, route: &Route) -> Vec<(f64, f64)> {
        aircraft::hold_limits(&self.airport, &self.graph, route, 0, 0)
            .into_iter()
            .map(|(d, _)| aircraft::point_at(&self.airport, &self.graph, route, d))
            .collect()
    }

    pub fn push(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].push(&self.airport, &self.graph)
    }

    /// Clear aircraft `i` along `route`, exactly as given. Nothing re-plans
    /// it. A stand another aircraft is on, or bound for, is refused.
    pub fn taxi(&mut self, i: usize, route: Route) -> Result<(), String> {
        let end = route.end();
        if let Some(st) = self.airport.stands.iter().find(|s| s.node == end) {
            let taken = self.aircraft.iter().enumerate().find(|&(j, a)| j != i && a.visible() && a.route.end() == end);
            if let Some((_, a)) = taken {
                return Err(format!("stand {} occupied by {}", st.name, a.callsign));
            }
        }
        self.aircraft[i].taxi(route, &self.airport, &self.graph)
    }

    /// C. Never refused for separation; R5 logs a threshold crossed with
    /// the runway occupied.
    pub fn clear_to_land(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].clear_to_land()
    }

    /// G, logged as instructed: delay you caused, not a challenge.
    pub fn go_around(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].go_around()?;
        self.go_arounds.push(GoAroundLog { tick: self.tick, callsign: self.aircraft[i].callsign.clone(), why: GoAround::Instructed });
        Ok(())
    }

    /// E: exit at the one leaving runway node `node`.
    pub fn set_exit(&mut self, i: usize, node: u32) -> Result<(), String> {
        self.aircraft[i].set_exit(node, &self.airport, &self.graph)
    }

    pub fn hold(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].hold()
    }

    pub fn cross(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].cross(&self.airport, &self.graph)
    }

    pub fn line_up(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].line_up(&self.airport, &self.graph)
    }

    /// K. Never refused on separation grounds: the monitor logs what it
    /// breaks (PLAN-twr.md § B4).
    pub fn takeoff(&mut self, i: usize) -> Result<(), String> {
        self.aircraft[i].takeoff()?;
        self.monitor.on_takeoff(&self.aircraft, i, &self.airport, &self.graph, self.tick);
        Ok(())
    }

    /// Ticks until K would lift off clear of every wake, and who binds.
    pub fn wake_wait(&self, i: usize) -> Option<(u64, String)> {
        self.monitor.wake_wait(&self.aircraft[i], &self.airport, &self.graph, self.tick)
    }

    /// Who would make K break R1 or R2 now, for the strip's neutral note.
    /// On final: anyone on the runway it will land on.
    pub fn runway_blocker(&self, i: usize) -> Option<String> {
        match self.aircraft[i].phase {
            Phase::LinedUp(_) => self.monitor.runway_blocker(&self.aircraft, i, &self.airport, &self.graph),
            Phase::Final(_) => conflict::occupants(&self.aircraft, i, &self.airport, &self.graph).first().map(|&j| self.aircraft[j].callsign.clone()),
            _ => None,
        }
    }

    pub fn verbs(&self, i: usize) -> Vec<char> {
        self.aircraft[i].verbs(&self.airport, &self.graph)
    }

    /// Next random draw; exposed so later slices share the one stream.
    pub fn rand(&mut self) -> f64 {
        self.rng.next_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_real_second_is_three_game_seconds() {
        let mut c = Clock::default();
        // 1000 ms in 4 frames of 250 ms each, the frame cap.
        let steps: u32 = (0..4).map(|_| c.advance(250.0)).sum();
        assert_eq!(steps, TICK_HZ * 3);
        assert_eq!(c.game_seconds(), 3.0);
    }

    #[test]
    fn uneven_frames_lose_no_time() {
        let mut c = Clock::default();
        // 10 s of real time in ragged frames: 16.6, 17.1, 15.9 ms repeating.
        // Summed in integer microseconds so the expectation is exact too.
        let pattern_us: [u64; 3] = [16_600, 17_100, 15_900];
        let mut real_us = 0;
        let mut i = 0;
        while real_us < 10_000_000 {
            let dt = pattern_us[i % 3];
            c.advance(dt as f64 / 1000.0);
            real_us += dt;
            i += 1;
        }
        let expected = real_us * TIME_COMPRESSION_MILLI / TICK_GAME_NS;
        assert_eq!(c.ticks(), expected);
    }

    #[test]
    fn backgrounded_tab_is_capped_not_replayed() {
        let mut c = Clock::default();
        let steps = c.advance(30_000.0);
        // 250 ms real at 3x is 750 ms game: exactly 15 ticks, not 1800.
        assert_eq!(steps, 15);
    }

    #[test]
    fn junk_frame_times_step_nothing() {
        let mut c = Clock::default();
        assert_eq!(c.advance(f64::NAN), 0);
        assert_eq!(c.advance(-50.0), 0);
        assert_eq!(c.advance(f64::INFINITY), 0);
    }

    #[test]
    fn same_seed_same_world() {
        let mut a = World::new(42);
        let mut b = World::new(42);
        assert_eq!(a.aircraft[0].route, b.aircraft[0].route);
        assert_eq!(a.rand(), b.rand());
        let stands = |s| World::new(s).aircraft[0].route.end();
        assert!((0..8).map(stands).collect::<std::collections::HashSet<_>>().len() > 1, "seed picks the stand");
    }

    #[test]
    fn rng_matches_reference_splitmix64() {
        // Reference outputs of SplitMix64 for seed 0.
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }
}
