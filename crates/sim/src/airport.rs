//! TWR-SIM · AIRPORT
//!   · Airport, Node, Edge, EdgeKind, Runway, Stand, Hold: the route graph
//!   · Airport::parse / Airport::to_text: the `.twr` text format, both ways
//!   · Airport::components, Airport::runway_exits: what the B1 tests check
//!
//! The graph is the baked output of `twr-osm` (crates/osm). The sim reads it
//! and never sees OpenStreetMap itself.
//!
//! Coordinates are integer decimetres east and north of the file's origin.
//! Integers mean every platform reads the same numbers, so a route replays
//! identically from its seed; decimetres are finer than any OSM survey.
//!
//! Format, one record per line, the name always last so it may hold spaces:
//!
//! ```text
//! twr-airport 2
//! icao VIDP
//! osm_base 2026-11-05T10:33:03Z      OSM snapshot the file was built from
//! origin 28.56 77.10                 lat lon of (0, 0)
//! n <x_dm> <y_dm>                    node; its index is its order among n lines
//! e <a> <b> <t|r|s> [name]           edge: taxiway, runway, stand link
//! r <end_a> <end_b> <name>           runway, by its two end nodes
//! s <node> <A-F|-> [name]           stand; the code is its largest ICAO aircraft code, - if unknown
//! h <node> [name]                    holding point
//! p <a|t> <n> x1 y1 .. xn yn [name]  apron (a) or terminal (t) outline, n >= 3 points
//! ```

/// First line of every file. Bump the number when the format changes, so an
/// old file fails loudly instead of parsing into the wrong fields.
pub const FORMAT: &str = "twr-airport 2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// Taxiways, taxilanes and mapped stand lead-in lines.
    Taxiway,
    Runway,
    /// Synthetic: OSM puts stands beside the taxilane, not on it, so the
    /// extractor joins each stand to its nearest taxiway point.
    StandLink,
}

impl EdgeKind {
    fn code(self) -> char {
        match self {
            EdgeKind::Taxiway => 't',
            EdgeKind::Runway => 'r',
            EdgeKind::StandLink => 's',
        }
    }

    fn from_code(c: &str) -> Option<Self> {
        match c {
            "t" => Some(EdgeKind::Taxiway),
            "r" => Some(EdgeKind::Runway),
            "s" => Some(EdgeKind::StandLink),
            _ => None,
        }
    }
}

/// Undirected. `name` is the taxiway letter or runway designator; empty when
/// OSM and the overrides file both leave the segment unnamed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub kind: EdgeKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runway {
    pub name: String,
    /// Outermost centreline nodes, displaced thresholds included.
    pub ends: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stand {
    pub name: String,
    pub node: u32,
    /// B10: the largest ICAO code letter the stand takes (A-F), from the
    /// eAIP chart; '-' when the source gives none (OSM stands).
    pub code: char,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hold {
    pub name: String,
    pub node: u32,
}

/// B10: an apron or terminal outline, drawn under the network and never
/// routed over. Points are decimetres from the origin, like nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    /// 'a' apron, 't' terminal.
    pub kind: char,
    pub ring: Vec<(i32, i32)>,
    pub name: String,
}

impl Area {
    /// Where to print the name, in metres: the inside point farthest from
    /// the outline, on a 24 x 24 grid over the bounding box. The vertex mean
    /// falls outside a V-shaped pier (T3 International's lands on the apron).
    /// ponytail: grid search, about 1/24 of the box off the true pole;
    /// polylabel if labels ever need to sit exactly.
    pub fn label_point(&self) -> (f64, f64) {
        const GRID: usize = 24;
        let r: Vec<(f64, f64)> = self.ring.iter().map(|&(x, y)| (f64::from(x) / 10.0, f64::from(y) / 10.0)).collect();
        let edges = || r.iter().zip(r.iter().cycle().skip(1));
        let inside = |p: (f64, f64)| {
            edges().filter(|(a, b)| (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0)).count() % 2 == 1
        };
        let to_edge = |p: (f64, f64)| {
            edges().map(|(a, b)| {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
            }).fold(f64::INFINITY, f64::min)
        };
        let (x0, x1) = r.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.0), hi.max(p.0)));
        let (y0, y1) = r.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
        let n = r.len() as f64;
        let mut best = (r.iter().map(|p| p.0).sum::<f64>() / n, r.iter().map(|p| p.1).sum::<f64>() / n);
        let mut far = -1.0;
        for i in 0..GRID {
            for j in 0..GRID {
                let p = (x0 + (x1 - x0) * (i as f64 + 0.5) / GRID as f64, y0 + (y1 - y0) * (j as f64 + 0.5) / GRID as f64);
                if inside(p) && to_edge(p) > far {
                    (best, far) = (p, to_edge(p));
                }
            }
        }
        best
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Airport {
    pub icao: String,
    pub osm_base: String,
    pub origin: (f64, f64),
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub runways: Vec<Runway>,
    pub stands: Vec<Stand>,
    pub holds: Vec<Hold>,
    pub areas: Vec<Area>,
}

impl Airport {
    pub fn parse(text: &str) -> Result<Airport, String> {
        let mut lines = text.lines().enumerate();
        match lines.next() {
            Some((_, l)) if l == FORMAT => {}
            other => return Err(format!("line 1: expected '{FORMAT}', got {:?}", other.map(|o| o.1))),
        }
        let mut ap = Airport {
            icao: String::new(),
            osm_base: String::new(),
            origin: (0.0, 0.0),
            nodes: Vec::new(),
            edges: Vec::new(),
            runways: Vec::new(),
            stands: Vec::new(),
            holds: Vec::new(),
            areas: Vec::new(),
        };
        for (i, line) in lines {
            let at = |m: &str| format!("line {}: {m}: {line:?}", i + 1);
            let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
            let num = |s: Option<&str>| -> Result<i64, String> {
                s.and_then(|s| s.parse().ok()).ok_or_else(|| at("bad number"))
            };
            match tag {
                "icao" => ap.icao = rest.to_string(),
                "osm_base" => ap.osm_base = rest.to_string(),
                "origin" => {
                    let mut f = rest.split(' ').map(str::parse::<f64>);
                    match (f.next(), f.next()) {
                        (Some(Ok(lat)), Some(Ok(lon))) => ap.origin = (lat, lon),
                        _ => return Err(at("bad origin")),
                    }
                }
                "n" => {
                    let mut f = rest.split(' ');
                    let x = num(f.next())?;
                    let y = num(f.next())?;
                    let x = i32::try_from(x).map_err(|_| at("x out of range"))?;
                    let y = i32::try_from(y).map_err(|_| at("y out of range"))?;
                    ap.nodes.push(Node { x, y });
                }
                "e" => {
                    let mut f = rest.splitn(4, ' ');
                    let a = ap.node_ref(num(f.next())?).ok_or_else(|| at("node index out of range"))?;
                    let b = ap.node_ref(num(f.next())?).ok_or_else(|| at("node index out of range"))?;
                    let kind = f.next().and_then(EdgeKind::from_code).ok_or_else(|| at("bad edge kind"))?;
                    ap.edges.push(Edge { a, b, kind, name: f.next().unwrap_or("").to_string() });
                }
                "r" => {
                    let mut f = rest.splitn(3, ' ');
                    let a = ap.node_ref(num(f.next())?).ok_or_else(|| at("node index out of range"))?;
                    let b = ap.node_ref(num(f.next())?).ok_or_else(|| at("node index out of range"))?;
                    ap.runways.push(Runway { name: f.next().unwrap_or("").to_string(), ends: [a, b] });
                }
                "s" | "h" => {
                    // s <node> <code A-F or -> [name]; h <node> [name]
                    let mut f = rest.splitn(if tag == "s" { 3 } else { 2 }, ' ');
                    let node = ap.node_ref(num(f.next())?).ok_or_else(|| at("node index out of range"))?;
                    let code = if tag == "s" {
                        let c = f.next().unwrap_or("");
                        match c.as_bytes() {
                            [b'A'..=b'F' | b'-'] => c.as_bytes()[0] as char,
                            _ => return Err(at("stand code must be A-F or -")),
                        }
                    } else {
                        '-'
                    };
                    let name = f.next().unwrap_or("").to_string();
                    if tag == "s" {
                        ap.stands.push(Stand { name, node, code });
                    } else {
                        ap.holds.push(Hold { name, node });
                    }
                }
                // p <a|t> <n> x1 y1 ... xn yn [name]
                "p" => {
                    let mut f = rest.split(' ');
                    let kind = match f.next() {
                        Some("a") => 'a',
                        Some("t") => 't',
                        _ => return Err(at("area kind must be a or t")),
                    };
                    let n = usize::try_from(num(f.next())?).map_err(|_| at("bad point count"))?;
                    // Check the count against the fields on the line before
                    // allocating: a corrupt count must be an error, not a
                    // multi-gigabyte Vec::with_capacity.
                    let have = f.clone().count() / 2;
                    if n > have {
                        return Err(at(&format!("point count {n}, but the line holds only {have} points")));
                    }
                    let mut ring = Vec::with_capacity(n);
                    for _ in 0..n {
                        let x = i32::try_from(num(f.next())?).map_err(|_| at("x out of range"))?;
                        let y = i32::try_from(num(f.next())?).map_err(|_| at("y out of range"))?;
                        ring.push((x, y));
                    }
                    if n < 3 {
                        return Err(at("an area needs 3 points"));
                    }
                    ap.areas.push(Area { kind, ring, name: f.collect::<Vec<_>>().join(" ") });
                }
                "" => {}
                _ => return Err(at("unknown record")),
            }
        }
        Ok(ap)
    }

    /// Node index check for the parser. Nodes come first in the file, so a
    /// forward reference is an error rather than something to resolve later.
    fn node_ref(&self, i: i64) -> Option<u32> {
        (0..self.nodes.len() as i64).contains(&i).then_some(i as u32)
    }

    pub fn to_text(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "{FORMAT}\nicao {}\nosm_base {}", self.icao, self.osm_base);
        let _ = writeln!(s, "origin {} {}", self.origin.0, self.origin.1);
        for n in &self.nodes {
            let _ = writeln!(s, "n {} {}", n.x, n.y);
        }
        // Trailing spaces would be harmless to the parser but noisy in a diff.
        let named = |line: String, name: &str| if name.is_empty() { line } else { format!("{line} {name}") };
        for e in &self.edges {
            let _ = writeln!(s, "{}", named(format!("e {} {} {}", e.a, e.b, e.kind.code()), &e.name));
        }
        for r in &self.runways {
            let _ = writeln!(s, "r {} {} {}", r.ends[0], r.ends[1], r.name);
        }
        for st in &self.stands {
            let _ = writeln!(s, "{}", named(format!("s {} {}", st.node, st.code), &st.name));
        }
        for a in &self.areas {
            let pts: Vec<String> = a.ring.iter().map(|(x, y)| format!("{x} {y}")).collect();
            let _ = writeln!(s, "{}", named(format!("p {} {} {}", a.kind, a.ring.len(), pts.join(" ")), &a.name));
        }
        for h in &self.holds {
            let _ = writeln!(s, "{}", named(format!("h {}", h.node), &h.name));
        }
        s
    }

    pub fn edge_len_m(&self, e: &Edge) -> f64 {
        let (a, b) = (self.nodes[e.a as usize], self.nodes[e.b as usize]);
        f64::from(a.x - b.x).hypot(f64::from(a.y - b.y)) / 10.0
    }

    /// Connected-component id per node, counting only edges `use_edge`
    /// accepts. Union-find; ids are the root node index.
    pub fn components(&self, use_edge: impl Fn(&Edge) -> bool) -> Vec<u32> {
        let mut parent: Vec<u32> = (0..self.nodes.len() as u32).collect();
        fn root(p: &mut [u32], mut i: u32) -> u32 {
            while p[i as usize] != i {
                p[i as usize] = p[p[i as usize] as usize];
                i = p[i as usize];
            }
            i
        }
        for e in self.edges.iter().filter(|e| use_edge(e)) {
            let (ra, rb) = (root(&mut parent, e.a), root(&mut parent, e.b));
            parent[ra as usize] = rb;
        }
        (0..self.nodes.len() as u32).map(|i| root(&mut parent, i)).collect()
    }

    /// Every edge of the taxiway that `edge` belongs to: all taxiway edges
    /// sharing its name, or just `edge` when it is unnamed. "Avoid B" means
    /// all of B, not the one segment under the pointer.
    pub fn taxiway_of(&self, edge: u32) -> Vec<u32> {
        let e = &self.edges[edge as usize];
        if e.name.is_empty() || e.kind != EdgeKind::Taxiway {
            return vec![edge];
        }
        (0..self.edges.len() as u32)
            .filter(|&i| self.edges[i as usize].kind == EdgeKind::Taxiway && self.edges[i as usize].name == e.name)
            .collect()
    }

    /// Nodes where a taxiway meets a runway: every place an aircraft can
    /// enter or leave one. Sorted, no repeats.
    pub fn runway_exits(&self) -> Vec<u32> {
        let mut on_rwy = vec![false; self.nodes.len()];
        let mut on_twy = vec![false; self.nodes.len()];
        for e in &self.edges {
            let side = if e.kind == EdgeKind::Runway { &mut on_rwy } else { &mut on_twy };
            side[e.a as usize] = true;
            side[e.b as usize] = true;
        }
        (0..self.nodes.len() as u32).filter(|&i| on_rwy[i as usize] && on_twy[i as usize]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baked by `cargo run -p twr-osm -- VIDP`. twr-osm's own test fails if
    /// this file is stale against the raw data and the overrides.
    const VIDP: &str = include_str!("../../../data/vidp.twr");

    #[test]
    fn vidp_is_one_network() {
        let ap = Airport::parse(VIDP).unwrap();
        let comp = ap.components(|_| true);
        let mut sizes = std::collections::BTreeMap::<u32, usize>::new();
        for c in &comp {
            *sizes.entry(*c).or_default() += 1;
        }
        assert_eq!(sizes.len(), 1, "disconnected components (root node: size): {sizes:?}");
    }

    /// Stronger than one network: every runway exit, stand and holding point
    /// connect with no runway edge used, so a taxi route never needs to roll
    /// along a runway to get somewhere. Crossing one is still allowed: a
    /// crossing passes through a runway node, not along a runway edge.
    #[test]
    fn vidp_every_stand_reachable_from_every_exit_without_using_a_runway() {
        let ap = Airport::parse(VIDP).unwrap();
        let comp = ap.components(|e| e.kind != EdgeKind::Runway);
        let exits = ap.runway_exits();
        assert!(exits.len() > 20, "only {} runway exits", exits.len());
        let want = comp[ap.stands[0].node as usize];
        let stray: Vec<String> = exits
            .iter()
            .map(|&n| format!("exit node {n}"))
            .zip(exits.iter().map(|&n| comp[n as usize]))
            .chain(ap.stands.iter().map(|s| (format!("stand {}", s.name), comp[s.node as usize])))
            .chain(ap.holds.iter().map(|h| (format!("hold node {}", h.node), comp[h.node as usize])))
            .filter(|(_, c)| *c != want)
            .map(|(what, _)| what)
            .collect();
        assert!(stray.is_empty(), "cut off from the stands without a runway edge: {stray:?}");
    }

    #[test]
    fn vidp_has_its_four_runways() {
        let ap = Airport::parse(VIDP).unwrap();
        let mut names: Vec<&str> = ap.runways.iter().map(|r| r.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["09/27", "10/28", "11L/29R", "11R/29L"]);
    }

    #[test]
    fn text_round_trips() {
        let ap = Airport::parse(VIDP).unwrap();
        assert_eq!(Airport::parse(&ap.to_text()).unwrap(), ap);
        assert_eq!(ap.to_text(), VIDP);
    }

    #[test]
    fn bad_files_fail_with_a_line_number() {
        let head = "twr-airport 2\nn 0 0\nn 10 0\n";
        assert!(Airport::parse("twr-airport 0\n").unwrap_err().contains("line 1"));
        let e = Airport::parse(&format!("{head}e 0 2 t A\n")).unwrap_err();
        assert!(e.starts_with("line 4: node index out of range"), "{e}");
        assert!(Airport::parse(&format!("{head}e 0 1 q A\n")).unwrap_err().contains("bad edge kind"));
        assert!(Airport::parse(&format!("{head}z\n")).unwrap_err().contains("unknown record"));
        let ok = Airport::parse(&format!("{head}e 0 1 t Bay 7\ns 1 E Stand 12A\n")).unwrap();
        assert_eq!(ok.edges[0].name, "Bay 7");
        assert_eq!((ok.stands[0].name.as_str(), ok.stands[0].code), ("Stand 12A", 'E'));
        // B10: a stand line carries its code letter, A-F or '-'.
        assert!(Airport::parse(&format!("{head}s 1 Stand 12A\n")).unwrap_err().contains("stand code"));
        assert_eq!(ok.edge_len_m(&ok.edges[0]), 1.0);
    }

    #[test]
    fn area_label_sits_inside_a_v_shape() {
        // A V, 100 m arms 20 m thick: its vertex mean lies in the notch.
        let v = Area { kind: 't', name: String::new(), ring: vec![(0, 0), (1000, 0), (1000, 1000), (800, 1000), (800, 200), (0, 200)] };
        let n = v.ring.len() as f64;
        let mean = (v.ring.iter().map(|p| f64::from(p.0)).sum::<f64>() / n / 10.0, v.ring.iter().map(|p| f64::from(p.1)).sum::<f64>() / n / 10.0);
        let (x, y) = v.label_point();
        assert!(mean.0 < 80.0 && mean.1 > 20.0, "the test shape's mean {mean:?} must be in the notch");
        assert!((x >= 80.0 && y <= 100.0) || y <= 20.0, "label at ({x:.1}, {y:.1}) is outside the V");
        assert!(x <= 100.0 && y >= 0.0);
    }

    /// R1.23: a corrupt point count must fail with a message, not allocate.
    #[test]
    fn area_point_count_is_checked_before_allocating() {
        let head = format!("{FORMAT}\nicao TEST\nosm_base x\norigin 0 0\n");
        let err = Airport::parse(&format!("{head}p a 4000000000000 0 0 10 0 10 10\n")).unwrap_err();
        assert!(err.contains("point count 4000000000000") && err.contains("only 3 points"), "{err}");
        // Too few points on the line, the count itself small.
        let err = Airport::parse(&format!("{head}p t 4 0 0 10 0 10 10\n")).unwrap_err();
        assert!(err.contains("only 3 points"), "{err}");
        assert!(Airport::parse(&format!("{head}p a 3 0 0 10 0 10 10 Apron 1\n")).is_ok());
    }

    /// R1.9: a version-1 file fails on the version line, not on a field.
    #[test]
    fn old_format_fails_on_the_version() {
        let err = Airport::parse("twr-airport 1\nicao X\n").unwrap_err();
        assert!(err.contains("expected 'twr-airport 2'"), "{err}");
    }
}
