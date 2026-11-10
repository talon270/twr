//! TWR-SIM · WEATHER
//!   · Wind: one per shift, drawn from the seed on its own stream
//!   · Flow: west (land 29L 28, depart 29R 27) or east (land 11R 10,
//!     depart 11L 09), picked by the wind (PLAN-twr.md § B9)
//!   · windshear: gusty shifts only, a weather go-around on final
//!
//! Every number here is `authored`: the `vidp` research run that would
//! source IGI's wind and flow rules has not produced anything yet. Wind
//! changes no speed; it picks the runways and, when gusting, adds a
//! challenge. The runway does not change mid-shift.

use crate::Rng;

/// 75 in 100 shifts draw a westerly (270°–340°), the rest an easterly
/// (060°–130°). `authored`, from memory: Delhi winds are mostly westerly,
/// easterly in the monsoon.
pub const WESTERLY_PCT: u64 = 75;
pub const WESTERLY_DEG: (u16, u16) = (270, 340);
pub const EASTERLY_DEG: (u16, u16) = (60, 130);
/// Mean wind, knots, uniform. `authored`.
pub const WIND_KT: (u8, u8) = (2, 16);
/// 20 in 100 shifts gust 8–15 kt above the mean. `authored`.
pub const GUSTY_PCT: u64 = 20;
pub const GUST_ABOVE_KT: (u8, u8) = (8, 15);
/// Below this the wind does not pick a direction and IGI keeps its
/// preferred (west) flow. `authored`; runway selection allows about 5 kt
/// of tailwind (PANS-ATM, from memory, unconfirmed).
pub const CALM_KT: u8 = 5;
/// True heading of 29L, measured from the OSM runway way (284°). The
/// other west-flow runways lie within 13° of it, so one heading decides.
pub const WEST_HEADING_DEG: f64 = 284.0;
/// On a gusty shift, an arrival goes around for windshear 3 times in 100,
/// inside the last PILOT_GA_MAX_M. `authored`.
pub const WINDSHEAR_GA_PCT: u64 = 3;

/// C4: tailwind on a runway end, knots, from which the player is warned
/// (`TAILWIND_WARN_KT`) or refused (`TAILWIND_LIMIT_KT`). PANS-ATM and
/// aircraft limits, from memory, unconfirmed; the mean wind is used, gusts
/// are not. `authored`.
pub const TAILWIND_WARN_KT: f64 = 5.0;
pub const TAILWIND_LIMIT_KT: f64 = 10.0;
/// A heavy needs this much runway to depart (`aircraft::MIN_TAKEOFF_RUN_HEAVY_M`).
pub const HEAVY_RUN_M: u64 = 3_100;

/// IGI's eight runway ends, each with its flow and the longest run it
/// offers, metres. Measured from `data/vidp.twr` by the test
/// `end_lengths_match_the_airport`. Order is the canonical one: a plan
/// lists its ends in it, so the draw from a default plan matches B6-B9.
pub const ENDS: [(&str, Flow, u64); 8] = [
    ("29L", Flow::West, 4421),
    ("28", Flow::West, 3808),
    ("29R", Flow::West, 4404),
    ("27", Flow::West, 2810),
    ("11R", Flow::East, 4421),
    ("10", Flow::East, 3808),
    ("11L", Flow::East, 4404),
    ("09", Flow::East, 2810),
];

pub fn end_flow(end: &str) -> Option<Flow> {
    ENDS.iter().find(|e| e.0 == end).map(|e| e.1)
}

pub fn end_len_m(end: &str) -> u64 {
    ENDS.iter().find(|e| e.0 == end).map_or(0, |e| e.2)
}

/// C4: which ends take landings and which take departures. All of one
/// direction: the conflict model does not link opposite directions
/// (`conflict.rs`), so a mix would go unchecked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub flow: Flow,
    pub land: Vec<&'static str>,
    pub depart: Vec<&'static str>,
}

impl Plan {
    /// The flow's own pick: two landing ends, two departure ends.
    pub fn default_for(flow: Flow) -> Plan {
        Plan { flow, land: flow.arr_rwys().to_vec(), depart: flow.dep_rwys().to_vec() }
    }

    /// Ends of `role` after setting `end` on or off, in canonical order.
    pub fn with(&self, end: &'static str, land: bool, on: bool) -> Plan {
        let mut p = self.clone();
        let list = if land { &mut p.land } else { &mut p.depart };
        list.retain(|e| *e != end);
        if on {
            list.push(end);
        }
        let rank = |e: &&str| ENDS.iter().position(|x| x.0 == *e);
        p.land.sort_by_key(rank);
        p.depart.sort_by_key(rank);
        p
    }

    /// The runway for a movement, from its draw `roll`; "" if no end fits
    /// (a heavy with no departure run of `HEAVY_RUN_M`).
    pub fn pick(&self, arrival: bool, heavy: bool, roll: u64) -> &'static str {
        let ends: Vec<&'static str> = if arrival {
            self.land.clone()
        } else if heavy {
            self.depart.iter().copied().filter(|e| end_len_m(e) >= HEAVY_RUN_M).collect()
        } else {
            self.depart.clone()
        };
        if ends.is_empty() { "" } else { ends[(roll % ends.len() as u64) as usize] }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wind {
    /// Direction the wind blows from, degrees true.
    pub dir: u16,
    pub kt: u8,
    /// Peak gust, knots; None when steady.
    pub gust: Option<u8>,
}

impl Wind {
    pub const CALM: Wind = Wind { dir: 0, kt: 0, gust: None };

    /// Headwind on a west-flow runway, knots; negative is a tailwind.
    pub fn west_headwind(self) -> f64 {
        f64::from(self.kt) * (f64::from(self.dir) - WEST_HEADING_DEG).to_radians().cos()
    }

    /// Tailwind on a runway end of `flow`, knots, never negative. The
    /// flow's heading stands for all four ends: they lie within 13 degrees.
    pub fn tailwind_kt(self, flow: Flow) -> f64 {
        let head = f64::from(self.kt) * (f64::from(self.dir) - WEST_HEADING_DEG - if flow == Flow::East { 180.0 } else { 0.0 }).to_radians().cos();
        (-head).max(0.0)
    }

    pub fn flow(self) -> Flow {
        if self.kt < CALM_KT || self.west_headwind() >= 0.0 { Flow::West } else { Flow::East }
    }

    /// METAR-style: `290/12`, `100/08G21`, `calm`.
    pub fn text(self) -> String {
        if self.kt == 0 {
            return "calm".into();
        }
        let g = self.gust.map_or(String::new(), |g| format!("G{g:02}"));
        format!("{:03}/{:02}{g}", self.dir, self.kt)
    }
}

/// Draw the shift's wind. Its own stream, so play cannot change it and the
/// schedule's draws do not move.
pub fn draw(seed: u64) -> Wind {
    let mut r = Rng::new(seed ^ 0x3EA7_4E12_D1F7_0001);
    let (lo, hi) = if r.next_u64() % 100 < WESTERLY_PCT { WESTERLY_DEG } else { EASTERLY_DEG };
    let dir = lo + (r.next_u64() % u64::from(hi - lo + 1)) as u16;
    let kt = WIND_KT.0 + (r.next_u64() % u64::from(WIND_KT.1 - WIND_KT.0 + 1)) as u8;
    let (gusty, above) = (r.next_u64() % 100 < GUSTY_PCT, r.next_u64());
    let gust = gusty.then(|| kt + GUST_ABOVE_KT.0 + (above % u64::from(GUST_ABOVE_KT.1 - GUST_ABOVE_KT.0 + 1)) as u8);
    Wind { dir, kt, gust }
}

/// Which way IGI lands and departs. Arrivals split 50/50 between the two
/// landing runways, departures between the two departure runways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    West,
    East,
}

impl Flow {
    /// The split inside a flow is `authored` until the `vidp` run says how
    /// IGI shares it out.
    pub fn arr_rwys(self) -> [&'static str; 2] {
        match self {
            Flow::West => ["29L", "28"],
            Flow::East => ["11R", "10"],
        }
    }

    pub fn dep_rwys(self) -> [&'static str; 2] {
        match self {
            Flow::West => ["29R", "27"],
            Flow::East => ["11L", "09"],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Flow::West => "west",
            Flow::East => "east",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_rule_at_its_edges() {
        let w = |dir, kt| Wind { dir, kt, gust: None };
        assert_eq!(w(290, 12).flow(), Flow::West);
        assert_eq!(w(100, 12).flow(), Flow::East);
        assert_eq!(w(100, 4).flow(), Flow::West, "calm keeps west");
        // Either side of straight across 29L (014°): 1° in, 1° out.
        assert_eq!(w(13, 12).flow(), Flow::West);
        assert_eq!(w(15, 12).flow(), Flow::East);
    }

    #[test]
    fn tailwind_is_the_headwind_of_the_other_flow() {
        let w = Wind { dir: 290, kt: 12, gust: None };
        assert!(w.tailwind_kt(Flow::West) < 0.5, "a 290 wind is a headwind on 29");
        assert!((w.tailwind_kt(Flow::East) - 12.0 * 6.0_f64.to_radians().cos()).abs() < 0.01);
        assert!(Wind::CALM.tailwind_kt(Flow::East) == 0.0);
    }

    #[test]
    fn a_default_plan_draws_what_b6_to_b9_drew() {
        let p = Plan::default_for(Flow::West);
        assert_eq!(p.pick(true, false, 0), "29L");
        assert_eq!(p.pick(true, false, 1), "28");
        assert_eq!((p.pick(false, false, 0), p.pick(false, false, 1)), ("29R", "27"));
        assert_eq!((p.pick(false, true, 0), p.pick(false, true, 1)), ("29R", "29R"), "heavies take the long one whatever the roll");
        assert_eq!(p.with("29R", false, false).with("27", false, true).pick(false, true, 0), "", "27 alone is too short for a heavy");
        assert_eq!(p.with("28", false, true).depart, vec!["28", "29R", "27"], "canonical order");
    }

    #[test]
    fn text_reads_like_a_metar() {
        assert_eq!(Wind { dir: 90, kt: 8, gust: Some(21) }.text(), "090/08G21");
        assert_eq!(Wind::CALM.text(), "calm");
    }
}
