//! TWR-SIM · SCORE
//!   · Score: movements, delay you caused, the aircraft that waited; never
//!     blended into one number (PLAN-twr.md Phase 0, § B7)
//!   · the delay rules: which waits count against you, tick by tick
//!   · the end of a shift: 30:00 real for Light / Normal / Peak, the third
//!     incident for Surge; Practice never ends
//!
//! Incidents and challenges are not kept here: the monitor's incident log
//! and the go-around log already hold them, and a second count could only
//! disagree with the first.

use std::collections::BTreeMap;

use crate::aircraft::{Aircraft, Phase};
use crate::arrival::GoAround;
use crate::conflict::on_runway;
use crate::traffic::{Mode, SHIFT_REAL_S, TICKS_PER_REAL_S};
use crate::{World, TICK_HZ};

/// A departure is ready to push this long after it fell due, in game
/// seconds (20 real s): real departures have a slot time, and one is not
/// late the instant it appears. `authored`.
pub const STAND_GRACE_S: u64 = 60;
/// An aircraft counts as delayed once its waits add up past this, in game
/// seconds. Without it every aircraft counts: the controller in the B7 tests
/// "delayed" 22 of 22, each for the tick between vacating and T. `authored`.
pub const DELAYED_OVER_S: u64 = 60;
/// Surge ends on the tick of this incident (§ A2; confirmed 2026-11-07).
pub const SURGE_INCIDENTS: usize = 3;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Score {
    /// Takeoffs (the lift-off tick) plus landings (the threshold tick).
    pub movements: u32,
    /// Ticks, summed over aircraft, spent waiting on a clearance you could
    /// have given.
    pub delay_ticks: u64,
    /// Ticks waited, per aircraft that waited at all.
    pub waits: BTreeMap<String, u64>,
    /// The tick the shift ended on; nothing moves after it.
    pub over: Option<u64>,
}

impl Score {
    /// Aircraft whose waits add up past DELAYED_OVER_S.
    pub fn delayed(&self) -> usize {
        self.waits.values().filter(|&&t| t > DELAYED_OVER_S * u64::from(TICK_HZ)).count()
    }

    /// Delay in game minutes, the time the aircraft live through.
    pub fn delay_min(&self) -> f64 {
        self.delay_ticks as f64 / f64::from(TICK_HZ) / 60.0
    }
}

/// Does aircraft `i` wait on you this tick? `occupied[r]`: someone is on
/// runway `r`.
fn waiting(w: &World, i: usize, occupied: &[bool]) -> bool {
    let a: &Aircraft = &w.aircraft[i];
    match &a.phase {
        // Counts from the first tick after the grace: 70 s on stand is 10 s, 200 ticks.
        Phase::OnStand => a.ready.is_some_and(|r| w.tick > r + STAND_GRACE_S * u64::from(TICK_HZ)),
        Phase::Holding | Phase::HoldPosition => true,
        // Waiting while traffic uses the runway is the rule, not delay.
        Phase::HoldShort(name) => w.airport.runways.iter().position(|r| &r.name == name).is_some_and(|r| !occupied[r]),
        // Waiting out a wake minimum or a busy runway is the rule too.
        Phase::LinedUp(_) => w.wake_wait(i).is_none() && w.runway_blocker(i).is_none(),
        Phase::GoAround(_) => matches!(a.went_around, Some(GoAround::Instructed | GoAround::NoClearance)),
        _ => false,
    }
}

impl World {
    /// Challenges: go-arounds the pilot or the weather caused.
    pub fn challenges(&self) -> usize {
        self.go_arounds.iter().filter(|g| g.why.challenge()).count()
    }

    /// Movements the shift has offered so far: everything that fell due,
    /// spawned or still queued.
    pub fn offered(&self) -> usize {
        self.traffic.as_ref().map_or(0, |t| t.log.len() + t.final_queue.len() + t.stand_queue.len())
    }

    /// One tick of scoring, after movement, the monitor and traffic.
    pub(crate) fn score_tick(&mut self) {
        let Some(t) = self.traffic.as_ref() else { return };
        let mode = t.schedule.mode;
        let mut occupied = vec![false; self.airport.runways.len()];
        if self.aircraft.iter().any(|a| matches!(a.phase, Phase::HoldShort(_))) {
            for a in &self.aircraft {
                if let Some(r) = on_runway(a, &self.airport, &self.graph) {
                    occupied[r] = true;
                }
            }
        }
        let mut waited: Vec<String> = (0..self.aircraft.len())
            .filter(|&i| waiting(self, i, &occupied))
            .map(|i| self.aircraft[i].callsign.clone())
            .collect();
        let grace = STAND_GRACE_S * u64::from(TICK_HZ);
        waited.extend(t.stand_queue.iter().filter(|d| self.tick > d.tick + grace).map(|d| d.callsign.clone()));
        self.score.delay_ticks += waited.len() as u64;
        for cs in waited {
            *self.score.waits.entry(cs).or_default() += 1;
        }

        let end = match mode {
            Mode::Practice => false,
            Mode::Surge => self.monitor.incidents.len() >= SURGE_INCIDENTS,
            _ => self.tick >= SHIFT_REAL_S * TICKS_PER_REAL_S,
        };
        if end && self.score.over.is_none() {
            self.score.over = Some(self.tick);
        }
    }
}
