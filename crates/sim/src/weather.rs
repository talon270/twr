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
    fn text_reads_like_a_metar() {
        assert_eq!(Wind { dir: 90, kt: 8, gust: Some(21) }.text(), "090/08G21");
        assert_eq!(Wind::CALM.text(), "calm");
    }
}
