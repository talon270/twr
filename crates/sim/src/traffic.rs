//! TWR-SIM · TRAFFIC
//!   · Mode: Practice (no stream), Light / Normal / Peak shifts, Surge
//!   · the schedule: two Poisson streams, arrivals and departures, at half
//!     the mode's rate each, counted per real hour (PLAN-twr.md § A2, § B6)
//!   · OPERATORS, TYPES: who flies at IGI and in what, each row graded
//!   · final_slot_free: approach spacing on final, shared by new arrivals
//!     and go-arounds coming back (A12)
//!   · Traffic: the queues, stand pick, callsigns, on-stand shut-down
//!
//! The schedule draws from its own `Rng`, seeded from the world's seed, so
//! the stream of movements does not depend on what happens in play (a
//! pilot go-around draw, a command). Same seed and mode, same schedule.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::aircraft::{Aircraft, Phase, Wake, MDM_PER_M};
use crate::arrival::FINAL_SPAWN_M;
use crate::weather::{self, Wind};
use crate::{Rng, World, TICK_HZ, TIME_COMPRESSION_MILLI};

/// Sim ticks in one real second: 20 Hz on a 3x clock, 60.
pub const TICKS_PER_REAL_S: u64 = TICK_HZ as u64 * TIME_COMPRESSION_MILLI / 1000;
/// Sim ticks in one real hour: 216,000.
pub const TICKS_PER_REAL_H: u64 = 3600 * TICKS_PER_REAL_S;
/// A Shift lasts 30 real minutes; after that nothing new spawns.
pub const SHIFT_REAL_S: u64 = 30 * 60;
/// Surge starts at Normal and climbs 10% every 2 real minutes (§ A2).
pub const SURGE_START_PER_H: u64 = 53;
pub const SURGE_STEP_REAL_S: u64 = 120;
/// ponytail: rate cap so the per-tick draw stays a probability; B7 ends
/// Surge at 3 incidents long before this (it is passed at about 44 min).
pub const SURGE_CAP_PER_H: u64 = 600;
/// An arrival on stand shuts down and leaves after 2 real minutes.
/// There is no turnaround into a departure: IGI's are 35-60 min, longer
/// than a shift. `authored`.
pub const ON_STAND_REAL_S: u64 = 120;

const NM_M: u64 = 1_852;
/// Least spacing on final, any pair: 3 NM, the radar minimum. `memory`:
/// Doc 4444 8.7.3.2, not checked against the text.
pub const FINAL_MIN_NM: u64 = 3;

/// Distance-based wake minimum on final, NM, leader then follower; 0 where
/// only the radar minimum applies. `memory`: Doc 4444 8.7.3.4, unchecked.
pub fn final_wake_nm(lead: Wake, follow: Wake) -> u64 {
    use Wake::*;
    match (lead, follow) {
        (H, H) => 4,
        (H, M) => 5,
        (H, L) => 6,
        (J, H) => 6,
        (J, M) => 7,
        (J, L) => 8,
        _ => 0,
    }
}

/// One aircraft type: ICAO designator and wake category, from
/// `research/out/aircraft/types.csv` (`icao_wake`; FAA database, with the
/// manufacturer's MTOW where read): solid. Comments give MTOW.
pub struct Type {
    pub icao: &'static str,
    pub wake: Wake,
}

pub const TYPES: [Type; 24] = [
    Type { icao: "A20N", wake: Wake::M }, // 79,000 kg
    Type { icao: "A21N", wake: Wake::M }, // 97,000 kg
    Type { icao: "B38M", wake: Wake::M }, // 82,644 kg
    Type { icao: "AT76", wake: Wake::M }, // 22,800 kg
    Type { icao: "A320", wake: Wake::M }, // 78,000 kg
    Type { icao: "B738", wake: Wake::M }, // 79,016 kg
    Type { icao: "B788", wake: Wake::H }, // 227,930 kg
    Type { icao: "B77W", wake: Wake::H }, // 351,534 kg
    Type { icao: "B789", wake: Wake::H }, // 254,692 kg
    Type { icao: "A321", wake: Wake::M }, // 91,000 kg
    Type { icao: "A333", wake: Wake::H }, // 242,000 kg
    Type { icao: "A359", wake: Wake::H }, // 280,000 kg
    Type { icao: "A319", wake: Wake::M }, // 76,500 kg
    Type { icao: "A343", wake: Wake::H }, // 276,500 kg
    Type { icao: "DH8D", wake: Wake::M }, // 29,257 kg
    Type { icao: "B737", wake: Wake::M }, // 70,080 kg
    Type { icao: "A388", wake: Wake::J }, // 575,000 kg
    Type { icao: "B78X", wake: Wake::H }, // 254,012 kg
    Type { icao: "A339", wake: Wake::H }, // 251,018 kg
    Type { icao: "E190", wake: Wake::M }, // 50,300 kg
    Type { icao: "B748", wake: Wake::H }, // 447,696 kg
    Type { icao: "A332", wake: Wake::H }, // 242,000 kg
    Type { icao: "AT46", wake: Wake::M }, // 18,600 kg
    Type { icao: "B739", wake: Wake::M }, // 85,139 kg
];

pub fn type_wake(icao: &str) -> Wake {
    TYPES.iter().find(|t| t.icao == icao).map_or(Wake::M, |t| t.wake)
}

/// One operator: ICAO prefix, radio telephony, and the types it flies here,
/// each weighted in thousandths of a percent of all IGI movements; `share`
/// is their sum.
pub struct Operator {
    pub icao: &'static str,
    /// FAA JO 7340.2P, from `operators.csv`: solid.
    pub telephony: Option<&'static str>,
    pub share: u64,
    pub fleet: &'static [(&'static str, u64)],
}

/// Shares out of 100,000, generated from `research/work/aircraft/
/// estimate_mix.js` (B9). Domestic 77% (GMR, H1 FY27) split by DGCA's
/// September 2026 shares and each carrier's fleet: secondary, national,
/// not Delhi. International 23%: half Indian carriers, half foreign, one
/// rotation a day per foreign carrier unless sourced (Emirates 4, one of
/// them the A380), and each foreign carrier's type: `authored` in the
/// estimator. Heavies come to 11.1%. Dropped from the estimate: Centrum
/// Air and Aero Nomad (no ICAO code found, 0.15% each) and Alliance Air's
/// Do 228 (no wake row, 0.015%).
pub const OPERATORS: [Operator; 68] = [
    Operator { icao: "IGO", telephony: Some("IFLY"), share: 54939, fleet: &[("A21N", 24331), ("A20N", 22182), ("AT76", 5296), ("A320", 3130)] },
    Operator { icao: "AIC", telephony: Some("AIRINDIA"), share: 17433, fleet: &[("A20N", 9565), ("B788", 2081), ("A21N", 1705), ("A321", 1176), ("B77W", 1041), ("A319", 543), ("B789", 480), ("A359", 480), ("A320", 362)] },
    Operator { icao: "AXB", telephony: Some("EXPRESS INDIA"), share: 11043, fleet: &[("B38M", 5622), ("B738", 2707), ("A20N", 1538), ("A320", 814), ("A21N", 362)] },
    Operator { icao: "AKJ", telephony: Some("AKASA AIR"), share: 4349, fleet: &[("B38M", 4349)] },
    Operator { icao: "UAE", telephony: Some("EMIRATES"), share: 597, fleet: &[("B77W", 448), ("A388", 149)] },
    Operator { icao: "SEJ", telephony: Some("SPICEJET"), share: 466, fleet: &[("DH8D", 233), ("B738", 117), ("B38M", 68), ("B739", 29), ("B737", 19)] },
    Operator { icao: "QTR", telephony: Some("QATARI"), share: 447, fleet: &[("B77W", 149), ("B788", 149), ("A359", 149)] },
    Operator { icao: "BAW", telephony: Some("SPEEDBIRD"), share: 447, fleet: &[("B788", 149), ("B789", 149), ("B77W", 149)] },
    Operator { icao: "SIA", telephony: Some("SINGAPORE"), share: 447, fleet: &[("A359", 149), ("B78X", 149), ("B77W", 149)] },
    Operator { icao: "ETD", telephony: Some("ETIHAD"), share: 298, fleet: &[("B789", 149), ("A21N", 149)] },
    Operator { icao: "FDB", telephony: Some("SKY DUBAI"), share: 298, fleet: &[("B38M", 298)] },
    Operator { icao: "DLH", telephony: Some("LUFTHANSA"), share: 298, fleet: &[("A359", 149), ("B748", 149)] },
    Operator { icao: "ACA", telephony: Some("AIR CANADA"), share: 298, fleet: &[("B789", 149), ("B77W", 149)] },
    Operator { icao: "THA", telephony: Some("THAI"), share: 298, fleet: &[("A333", 149), ("B789", 149)] },
    Operator { icao: "ALK", telephony: Some("SRILANKAN"), share: 298, fleet: &[("A320", 149), ("A333", 149)] },
    Operator { icao: "LLR", telephony: Some("ALLIED"), share: 296, fleet: &[("AT76", 266), ("AT46", 30)] },
    Operator { icao: "ABY", telephony: Some("ARABIA"), share: 149, fleet: &[("A320", 149)] },
    Operator { icao: "GFA", telephony: Some("GULF AIR"), share: 149, fleet: &[("A21N", 149)] },
    Operator { icao: "OMA", telephony: Some("OMAN AIR"), share: 149, fleet: &[("B738", 149)] },
    Operator { icao: "KAC", telephony: Some("KUWAITI"), share: 149, fleet: &[("A20N", 149)] },
    Operator { icao: "JZR", telephony: Some("JAZEERA"), share: 149, fleet: &[("A20N", 149)] },
    Operator { icao: "SVA", telephony: Some("SAUDIA"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "KNE", telephony: Some("FLYNAS"), share: 149, fleet: &[("A20N", 149)] },
    Operator { icao: "RJA", telephony: Some("JORDANIAN"), share: 149, fleet: &[("A321", 149)] },
    Operator { icao: "IAW", telephony: Some("IRAQI"), share: 149, fleet: &[("B38M", 149)] },
    Operator { icao: "MSR", telephony: Some("EGYPTAIR"), share: 149, fleet: &[("B789", 149)] },
    Operator { icao: "ETH", telephony: Some("ETHIOPIAN"), share: 149, fleet: &[("B788", 149)] },
    Operator { icao: "VIR", telephony: Some("VIRGIN"), share: 149, fleet: &[("B789", 149)] },
    Operator { icao: "AFR", telephony: Some("AIRFRANS"), share: 149, fleet: &[("B77W", 149)] },
    Operator { icao: "KLM", telephony: Some("KLM"), share: 149, fleet: &[("B789", 149)] },
    Operator { icao: "SWR", telephony: Some("SWISS"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "FIN", telephony: Some("FINNAIR"), share: 149, fleet: &[("A359", 149)] },
    Operator { icao: "LOT", telephony: Some("LOT"), share: 149, fleet: &[("B788", 149)] },
    Operator { icao: "ITY", telephony: Some("ITARROW"), share: 149, fleet: &[("A332", 149)] },
    Operator { icao: "UAL", telephony: Some("UNITED"), share: 149, fleet: &[("B789", 149)] },
    Operator { icao: "AAL", telephony: Some("AMERICAN"), share: 149, fleet: &[("B789", 149)] },
    Operator { icao: "AFL", telephony: Some("AEROFLOT"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "CPA", telephony: Some("CATHAY"), share: 149, fleet: &[("A359", 149)] },
    Operator { icao: "MAS", telephony: Some("MALAYSIAN"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "MXD", telephony: Some("MALINDO"), share: 149, fleet: &[("B38M", 149)] },
    Operator { icao: "XAX", telephony: Some("XANADU"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "TAX", telephony: Some("EXPRESS WING"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "TLM", telephony: Some("MENTARI"), share: 149, fleet: &[("B38M", 149)] },
    Operator { icao: "VJC", telephony: Some("VIETJETAIR"), share: 149, fleet: &[("A321", 149)] },
    Operator { icao: "HVN", telephony: Some("VIET NAM AIRLINES"), share: 149, fleet: &[("A21N", 149)] },
    Operator { icao: "ANA", telephony: Some("ALL NIPPON"), share: 149, fleet: &[("B788", 149)] },
    Operator { icao: "JAL", telephony: Some("JAPANAIR"), share: 149, fleet: &[("B788", 149)] },
    Operator { icao: "KAL", telephony: Some("KOREANAIR"), share: 149, fleet: &[("B77W", 149)] },
    Operator { icao: "CCA", telephony: Some("AIR CHINA"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "CES", telephony: Some("CHINA EASTERN"), share: 149, fleet: &[("A333", 149)] },
    Operator { icao: "CSN", telephony: Some("CHINA SOUTHERN"), share: 149, fleet: &[("A21N", 149)] },
    Operator { icao: "QFA", telephony: Some("QANTAS"), share: 149, fleet: &[("B789", 149)] },
    Operator { icao: "MAU", telephony: Some("AIRMAURITIUS"), share: 149, fleet: &[("A339", 149)] },
    Operator { icao: "RNA", telephony: Some("ROYAL NEPAL"), share: 149, fleet: &[("A320", 149)] },
    Operator { icao: "DRK", telephony: Some("ROYAL BHUTAN"), share: 149, fleet: &[("A20N", 149)] },
    Operator { icao: "BTN", telephony: Some("BHUTAN AIR"), share: 149, fleet: &[("A319", 149)] },
    Operator { icao: "MMA", telephony: Some("MYANMAR"), share: 149, fleet: &[("A319", 149)] },
    Operator { icao: "KZR", telephony: Some("ASTANALINE"), share: 149, fleet: &[("A21N", 149)] },
    Operator { icao: "UZB", telephony: Some("UZBEK"), share: 149, fleet: &[("A20N", 149)] },
    Operator { icao: "AHY", telephony: Some("AZAL"), share: 149, fleet: &[("A20N", 149)] },
    Operator { icao: "TUA", telephony: Some("TURKMENISTAN"), share: 149, fleet: &[("B738", 149)] },
    Operator { icao: "SMR", telephony: Some("SOMON AIR"), share: 149, fleet: &[("B738", 149)] },
    Operator { icao: "AFG", telephony: Some("ARIANA"), share: 149, fleet: &[("B737", 149)] },
    Operator { icao: "KMF", telephony: Some("KAMGAR"), share: 149, fleet: &[("A343", 149)] },
    Operator { icao: "IRM", telephony: Some("MAHAN AIR"), share: 149, fleet: &[("A343", 149)] },
    Operator { icao: "BRU", telephony: Some("BELARUS AVIA"), share: 149, fleet: &[("B38M", 149)] },
    Operator { icao: "MML", telephony: Some("TRANS MONGOLIA"), share: 149, fleet: &[("E190", 149)] },
    Operator { icao: "KHV", telephony: Some("CAMBODIA AIR"), share: 149, fleet: &[("A321", 149)] },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Practice,
    Light,
    Normal,
    Peak,
    Surge,
}

impl Mode {
    pub const ALL: [Mode; 5] = [Mode::Practice, Mode::Light, Mode::Normal, Mode::Peak, Mode::Surge];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Practice => "practice",
            Mode::Light => "light",
            Mode::Normal => "normal",
            Mode::Peak => "peak",
            Mode::Surge => "surge",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.name() == s)
    }

    /// Movements per real hour offered at `tick`, in thousandths: integer,
    /// so Surge's compounding is the same on every platform.
    pub fn rate_milli(self, tick: u64) -> u64 {
        match self {
            Mode::Practice => 0,
            Mode::Light => 27_000,
            Mode::Normal => 53_000,
            Mode::Peak => 84_000,
            Mode::Surge => {
                let mut r = SURGE_START_PER_H * 1000;
                for _ in 0..tick / (SURGE_STEP_REAL_S * TICKS_PER_REAL_S) {
                    r = (r * 11 / 10).min(SURGE_CAP_PER_H * 1000);
                }
                r
            }
        }
    }

    /// Does anything new spawn at `tick`?
    pub fn open(self, tick: u64) -> bool {
        match self {
            Mode::Practice => false,
            Mode::Surge => true,
            _ => tick < SHIFT_REAL_S * TICKS_PER_REAL_S,
        }
    }
}

/// A movement that has fallen due: who, in what, to or from where.
#[derive(Debug, Clone, PartialEq)]
pub struct Due {
    pub tick: u64,
    pub arrival: bool,
    pub callsign: String,
    pub type_code: &'static str,
    pub wake: Wake,
    pub rwy: &'static str,
}

/// The pure schedule: no world, so tests can run hours of it in a blink.
pub struct Schedule {
    pub mode: Mode,
    /// B9: the shift's wind, and so its flow. Set it before the first tick
    /// to force one; the runways are drawn from it as each movement falls due.
    pub wind: Wind,
    rng: Rng,
    used: BTreeSet<String>,
}

impl Schedule {
    pub fn new(seed: u64, mode: Mode) -> Self {
        // A distinct stream from the world's: same seed, different draws.
        Schedule { mode, wind: weather::draw(seed), rng: Rng::new(seed ^ 0x7AF1_C0DE_5EED_0B6), used: BTreeSet::new() }
    }

    /// What falls due on `tick`: each stream is a Bernoulli trial per tick
    /// at rate / 2 per real hour, which is Poisson to within a tick. Two
    /// draws every tick whatever happens, so the stream never shifts.
    pub fn tick(&mut self, tick: u64) -> Vec<Due> {
        let rate = self.mode.rate_milli(tick);
        // P(per tick) = rate/1000 / 2 / TICKS_PER_REAL_H, as a u64 fraction.
        let p = ((u128::from(rate) << 64) / (2_000 * u128::from(TICKS_PER_REAL_H))) as u64;
        let (a, d) = (self.rng.next_u64(), self.rng.next_u64());
        let mut out = Vec::new();
        if !self.mode.open(tick) {
            return out;
        }
        for (hit, arrival) in [(a < p, true), (d < p, false)] {
            if hit {
                out.push(self.draw(tick, arrival));
            }
        }
        out
    }

    fn pick<'a, T>(&mut self, rows: &'a [T], weight: impl Fn(&T) -> u64) -> &'a T {
        let total: u64 = rows.iter().map(&weight).sum();
        let mut x = self.rng.next_u64() % total;
        for r in rows {
            if x < weight(r) {
                return r;
            }
            x -= weight(r);
        }
        unreachable!("x < total")
    }

    fn draw(&mut self, tick: u64, arrival: bool) -> Due {
        let op = self.pick(&OPERATORS, |o| o.share);
        let (icao, fleet) = (op.icao, op.fleet);
        let &(type_code, _) = self.pick(fleet, |t| t.1);
        let callsign = loop {
            let cs = format!("{icao}{}", 100 + self.rng.next_u64() % 9_900);
            if self.used.insert(cs.clone()) {
                break cs;
            }
        };
        let flow = self.wind.flow();
        let rwys = if arrival { flow.arr_rwys() } else { flow.dep_rwys() };
        let wake = type_wake(type_code);
        // A heavy departs the flow's long runway (29R, 11L: 4.4 km); 27 and
        // 09 (2.7 km) are short of MIN_TAKEOFF_RUN_HEAVY_M. The draw is made
        // either way, so the stream does not depend on the type.
        let pick = (self.rng.next_u64() % 2) as usize;
        let rwy = if !arrival && matches!(wake, Wake::H | Wake::J) { rwys[0] } else { rwys[pick] };
        Due { tick, arrival, callsign, type_code, wake, rwy }
    }
}

/// A12: may an arrival of `wake` appear at FINAL_SPAWN_M on `desig` now?
/// The leader is whoever is ahead on that final, landing roll or
/// go-around still over the runway; the gap is measured along the
/// approach. An aircraft that has turned off is not a leader: IGI's
/// first usable exits are 1.5 km or more down, past the 1.3 km a 5 NM
/// wake gap asks of the leader. A 6 NM (L behind H) gap would not be, but
/// no L flies here.
pub fn final_slot_free(ac: &[Aircraft], desig: &str, wake: Wake) -> bool {
    let spawn = (FINAL_SPAWN_M * MDM_PER_M) as i64;
    ac.iter().all(|a| {
        let ahead = match &a.phase {
            Phase::Final(d) if d == desig => spawn + a.air_mdm,
            Phase::Landing(d) if d == desig => spawn + a.pos_mdm as i64,
            Phase::GoAround(d) if d == desig && a.visible() => spawn + a.air_mdm,
            _ => return true,
        };
        let need = FINAL_MIN_NM.max(final_wake_nm(a.wake, wake)) * NM_M * MDM_PER_M;
        ahead >= need as i64
    })
}

/// The live traffic in a shift: the schedule, what is waiting to appear,
/// and every arrival that has reached its stand.
pub struct Traffic {
    pub schedule: Schedule,
    /// Arrivals due but held by approach until their final is free.
    pub final_queue: VecDeque<Due>,
    /// Departures due with no free stand to appear on.
    pub stand_queue: VecDeque<Due>,
    /// Everything spawned, in order: the replay check and the HUD read it.
    pub log: Vec<Due>,
    /// Arrivals spawned and not yet shut down; the tick each reached its stand.
    inbound: BTreeMap<String, Option<u64>>,
}

impl Traffic {
    pub fn new(seed: u64, mode: Mode) -> Self {
        Traffic { schedule: Schedule::new(seed, mode), final_queue: VecDeque::new(), stand_queue: VecDeque::new(), log: Vec::new(), inbound: BTreeMap::new() }
    }
}

impl World {
    /// A shift: IGI with nobody on it and the mode's traffic stream.
    /// Practice gives the B5 world instead (`World::new`).
    pub fn shift(seed: u64, mode: Mode) -> World {
        if mode == Mode::Practice {
            return World::new(seed);
        }
        let mut w = World::empty(seed);
        w.traffic = Some(Traffic::new(seed, mode));
        w
    }

    /// The shift's wind; Practice is calm, so it stays west flow.
    pub fn wind(&self) -> Wind {
        self.traffic.as_ref().map_or(Wind::CALM, |t| t.schedule.wind)
    }

    pub fn mode(&self) -> Mode {
        self.traffic.as_ref().map_or(Mode::Practice, |t| t.schedule.mode)
    }

    /// A stand nobody is on or bound for: the same test `taxi` refuses on.
    pub fn stand_free(&self, node: u32) -> bool {
        !self.aircraft.iter().any(|a| a.visible() && a.route.end() == node)
    }

    /// One tick of traffic, after movement and the conflict check.
    pub(crate) fn traffic_tick(&mut self) {
        let Some(mut t) = self.traffic.take() else { return };
        let now = self.tick;
        for d in t.schedule.tick(now) {
            if d.arrival { t.final_queue.push_back(d) } else { t.stand_queue.push_back(d) }
        }
        // Arrivals, in order, each onto its own final when that is free. A
        // spawn makes its final busy, so one per runway per tick at most.
        let mut k = 0;
        while k < t.final_queue.len() {
            let d = &t.final_queue[k];
            if final_slot_free(&self.aircraft, d.rwy, d.wake) {
                let d = t.final_queue.remove(k).expect("k in range");
                let i = self.spawn_arrival(&d.callsign, d.wake, d.rwy).expect("every landing runway has a default exit; traffic tests guard this");
                self.aircraft[i].type_code = d.type_code;
                t.inbound.insert(d.callsign.clone(), None);
                t.log.push(d);
            } else {
                k += 1;
            }
        }
        // Departures onto a free stand, drawn from the schedule's stream.
        while !t.stand_queue.is_empty() {
            let free: Vec<u32> = self.airport.stands.iter().map(|s| s.node).filter(|&n| self.stand_free(n)).collect();
            if free.is_empty() {
                break;
            }
            let node = free[(t.schedule.rng.next_u64() % free.len() as u64) as usize];
            let d = t.stand_queue.pop_front().expect("non-empty");
            let a = Aircraft { wake: d.wake, type_code: d.type_code, assigned: Some(d.rwy), ready: Some(d.tick), ..Aircraft::parked(&d.callsign, node) };
            self.add(a);
            t.log.push(d);
        }
        // Arrivals on stand shut down after ON_STAND_REAL_S.
        for a in &mut self.aircraft {
            let Some(since) = t.inbound.get_mut(&a.callsign) else { continue };
            if a.phase != Phase::OnStand {
                continue;
            }
            let at = *since.get_or_insert(now);
            if now - at >= ON_STAND_REAL_S * TICKS_PER_REAL_S {
                a.phase = Phase::ShutDown;
                t.inbound.remove(&a.callsign);
            }
        }
        self.traffic = Some(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_add_up() {
        assert_eq!(OPERATORS.iter().map(|o| o.share).sum::<u64>(), 100_000);
        for o in &OPERATORS {
            assert_eq!(o.fleet.iter().map(|t| t.1).sum::<u64>(), o.share, "{}", o.icao);
            assert_eq!(o.icao.len(), 3, "callsigns take a 3-letter prefix");
            for (t, _) in o.fleet {
                assert!(TYPES.iter().any(|x| x.icao == *t), "{t} has no wake row");
            }
        }
    }

    #[test]
    fn surge_compounds_ten_percent_every_two_minutes() {
        let step = SURGE_STEP_REAL_S * TICKS_PER_REAL_S;
        assert_eq!(Mode::Surge.rate_milli(0), 53_000);
        assert_eq!(Mode::Surge.rate_milli(step - 1), 53_000);
        assert_eq!(Mode::Surge.rate_milli(step), 58_300);
        assert_eq!(Mode::Surge.rate_milli(5 * step) / 1000, 85);
    }
}

#[cfg(test)]
mod runway_length {
    use crate::aircraft::{min_takeoff_run_m, Wake};
    use crate::weather::Flow;
    use crate::World;

    /// The longest takeoff run each departure runway offers, from any entry.
    fn longest(w: &World, desig: &str) -> u64 {
        w.airport.runway_exits().into_iter()
            .filter_map(|n| w.graph.runway_run(&w.airport, n))
            .filter(|(_, d)| d == desig)
            .map(|(r, _)| w.graph.run_len_dm(&r) / 10)
            .max()
            .unwrap_or(0)
    }

    /// Heavies depart the flow's first departure runway (B9, R3): it must
    /// be long enough for one, and every departure runway for the rest.
    #[test]
    fn heavies_get_a_runway_long_enough() {
        let w = World::empty(1);
        for flow in [Flow::West, Flow::East] {
            let [long, other] = flow.dep_rwys();
            let (l, o) = (longest(&w, long), longest(&w, other));
            println!("{flow:?}: {long} {l} m, {other} {o} m");
            assert!(l >= min_takeoff_run_m(Wake::H), "{long}: {l} m");
            assert!(o >= min_takeoff_run_m(Wake::M), "{other}: {o} m");
        }
    }
}
