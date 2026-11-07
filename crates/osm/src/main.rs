//! TWR-OSM · AIRPORT EXTRACTOR
//!   · reads data/<icao>.osm.json, the raw Overpass answer (fetch-osm.sh)
//!   · applies data/<icao>.overrides.toml, names typed in by hand
//!   · writes data/<icao>.twr, the route graph the sim bakes in
//!   · prints a data-quality report: what OSM leaves unnamed or unjoined
//!
//! Usage, from anywhere:  cargo run -p twr-osm -- VIDP
//!
//! Runs by hand, once per airport per refresh. The game never fetches. The
//! output is deterministic: ways and nodes are walked in OSM-id order, so the
//! same raw file always writes the same bytes, and the test below can demand
//! that the committed `.twr` matches its sources exactly.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;
use twr_sim::airport::{Airport, Edge, EdgeKind, Hold, Node, Runway, Stand};

/// A stand further than this from every taxiway is refused, not linked. IGI's
/// longest link is about 130 m (2026-11-05 data). Past 200 m the straight link
/// would be invented geometry crossing whatever lies between.
const MAX_STAND_LINK_M: f64 = 200.0;

/// A stand link landing within this distance of an existing node reuses it,
/// rather than splitting a segment into a sliver.
const SNAP_M: f64 = 1.0;

/// A dead-end taxiway this close to another segment is probably a missing
/// junction in OSM. Reported, never auto-joined: joining is a data edit, and
/// it belongs in OSM or the overrides file where a person can see it.
const NEAR_MISS_M: f64 = 5.0;

/// Mean Earth radius (IUGG). An equirectangular projection around the airport
/// centre is off by under 0.1% across a 6 km field, which no rule here feels.
const EARTH_R_M: f64 = 6_371_008.8;

type Tags = BTreeMap<String, String>;

struct Way {
    id: u64,
    nodes: Vec<u64>,
    tags: Tags,
}

struct Osm {
    base: String,
    nodes: HashMap<u64, (f64, f64, Tags)>,
    ways: Vec<Way>,
}

#[derive(Debug, Default)]
struct Overrides {
    taxiway: BTreeMap<u64, String>,
    hold: BTreeMap<u64, String>,
    stand: BTreeMap<u64, String>,
}

fn tags_of(v: &Value) -> Tags {
    v.get("tags")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())
        .unwrap_or_default()
}

fn parse_osm(text: &str) -> Result<Osm, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("osm json: {e}"))?;
    let base = v["osm3s"]["timestamp_osm_base"].as_str().ok_or("osm json: no osm3s.timestamp_osm_base")?;
    let mut osm = Osm { base: base.to_string(), nodes: HashMap::new(), ways: Vec::new() };
    for el in v["elements"].as_array().ok_or("osm json: no elements")? {
        let id = el["id"].as_u64().ok_or("osm json: element without id")?;
        match el["type"].as_str() {
            Some("node") => {
                let (lat, lon) = (el["lat"].as_f64(), el["lon"].as_f64());
                let (Some(lat), Some(lon)) = (lat, lon) else { return Err(format!("node {id}: no lat/lon")) };
                osm.nodes.insert(id, (lat, lon, tags_of(el)));
            }
            Some("way") => {
                let nodes = el["nodes"].as_array().ok_or(format!("way {id}: no nodes"))?;
                let nodes = nodes.iter().map(|n| n.as_u64().ok_or(format!("way {id}: bad node"))).collect::<Result<_, _>>()?;
                osm.ways.push(Way { id, nodes, tags: tags_of(el) });
            }
            _ => {}
        }
    }
    osm.ways.sort_by_key(|w| w.id);
    Ok(osm)
}

fn parse_overrides(text: &str) -> Result<Overrides, String> {
    let t: toml::Table = text.parse().map_err(|e| format!("overrides: {e}"))?;
    let mut ov = Overrides::default();
    for (section, body) in &t {
        let map = match section.as_str() {
            "taxiway" => &mut ov.taxiway,
            "hold" => &mut ov.hold,
            "stand" => &mut ov.stand,
            _ => return Err(format!("overrides: unknown section [{section}]")),
        };
        let body = body.as_table().ok_or(format!("overrides: [{section}] is not a table"))?;
        for (k, v) in body {
            let id = k.parse().map_err(|_| format!("overrides [{section}]: '{k}' is not an OSM id"))?;
            let name = v.as_str().ok_or(format!("overrides [{section}] {k}: name must be a string"))?;
            map.insert(id, name.to_string());
        }
    }
    Ok(ov)
}

fn aeroway(t: &Tags) -> &str {
    t.get("aeroway").map_or("", String::as_str)
}

/// Taxi network ways. Mapped stand lead-in lines (`parking_position` ways)
/// count as taxiway: aircraft roll along them.
fn is_network(w: &Way) -> bool {
    matches!(aeroway(&w.tags), "runway" | "taxiway" | "taxilane" | "parking_position")
}

/// Closest point to `p` on segment a-b: (distance, t along a-b, the point).
fn nearest_on(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64, (f64, f64)) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let q = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - q.0).hypot(p.1 - q.1), t, q)
}

fn extract(icao: &str, osm: &Osm, ov: &Overrides) -> Result<(Airport, Vec<String>), String> {
    let mut errs = Vec::new();

    // Overrides must point at something real. An OSM edit can delete or
    // retag an element; a stale override then names nothing, silently.
    let node_is = |id: &u64, want: &str| osm.nodes.get(id).is_some_and(|n| aeroway(&n.2) == want);
    for id in ov.taxiway.keys() {
        if !osm.ways.iter().any(|w| w.id == *id && is_network(w) && aeroway(&w.tags) != "runway") {
            errs.push(format!("override [taxiway] {id}: no such taxiway way"));
        }
    }
    for (map, want) in [(&ov.hold, "holding_position"), (&ov.stand, "parking_position")] {
        errs.extend(map.keys().filter(|id| !node_is(id, want)).map(|id| format!("override: {id} is not a {want} node")));
    }

    let net: Vec<&Way> = osm.ways.iter().filter(|w| is_network(w)).collect();
    let coord = |id: u64| osm.nodes.get(&id).map(|n| (n.0, n.1)).ok_or(format!("node {id} missing from the osm file"));

    // Origin: centre of the network's bounding box, rounded to 1e-6 degrees
    // (about 0.1 m) so the file shows a readable number.
    let (mut lat_lo, mut lat_hi, mut lon_lo, mut lon_hi) = (90.0f64, -90.0f64, 180.0f64, -180.0f64);
    for w in &net {
        for &n in &w.nodes {
            let (lat, lon) = coord(n)?;
            (lat_lo, lat_hi, lon_lo, lon_hi) = (lat_lo.min(lat), lat_hi.max(lat), lon_lo.min(lon), lon_hi.max(lon));
        }
    }
    let round6 = |v: f64| (v * 1e6).round() / 1e6;
    let origin = (round6((lat_lo + lat_hi) / 2.0), round6((lon_lo + lon_hi) / 2.0));
    let cos0 = origin.0.to_radians().cos();
    let to_dm = |(lat, lon): (f64, f64)| Node {
        x: ((lon - origin.1).to_radians() * cos0 * EARTH_R_M * 10.0).round() as i32,
        y: ((lat - origin.0).to_radians() * EARTH_R_M * 10.0).round() as i32,
    };

    // Runways: OSM splits one runway into the main way plus displaced
    // threshold ways, often with the designator only on the main way. Ways
    // sharing an end node are joined; each group must carry exactly one ref.
    let rwy: Vec<&Way> = net.iter().copied().filter(|w| aeroway(&w.tags) == "runway").collect();
    let mut group: Vec<usize> = (0..rwy.len()).collect();
    fn find(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            i = g[i];
        }
        i
    }
    for i in 0..rwy.len() {
        for j in 0..i {
            let ends = |w: &Way| [w.nodes[0], *w.nodes.last().unwrap()];
            if ends(rwy[i]).iter().any(|n| ends(rwy[j]).contains(n)) {
                let (ri, rj) = (find(&mut group, i), find(&mut group, j));
                group[ri] = rj;
            }
        }
    }
    let mut rwy_name: HashMap<u64, String> = HashMap::new();
    for g in 0..rwy.len() {
        let members: Vec<&Way> = (0..rwy.len()).filter(|&i| find(&mut group, i) == g).map(|i| rwy[i]).collect();
        if members.is_empty() {
            continue;
        }
        let mut refs: Vec<&str> = members.iter().filter_map(|w| w.tags.get("ref").map(String::as_str)).collect();
        refs.sort();
        refs.dedup();
        match refs[..] {
            [r] => rwy_name.extend(members.iter().map(|w| (w.id, r.to_string()))),
            _ => errs.push(format!(
                "runway ways {:?} carry refs {refs:?}; a runway needs exactly one",
                members.iter().map(|w| w.id).collect::<Vec<_>>()
            )),
        }
    }

    // Nodes and edges, in way-id order so the output is stable.
    let mut idx: HashMap<u64, u32> = HashMap::new();
    let mut ap = Airport {
        icao: icao.to_string(),
        osm_base: osm.base.clone(),
        origin,
        nodes: Vec::new(),
        edges: Vec::new(),
        runways: Vec::new(),
        stands: Vec::new(),
        holds: Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    for w in &net {
        let (kind, name) = if aeroway(&w.tags) == "runway" {
            (EdgeKind::Runway, rwy_name.get(&w.id).cloned().unwrap_or_default())
        } else {
            let name = ov.taxiway.get(&w.id).or(w.tags.get("ref"));
            (EdgeKind::Taxiway, name.cloned().unwrap_or_default())
        };
        let mut prev: Option<u32> = None;
        for &n in &w.nodes {
            let i = match idx.get(&n) {
                Some(&i) => i,
                None => {
                    ap.nodes.push(to_dm(coord(n)?));
                    idx.insert(n, ap.nodes.len() as u32 - 1);
                    ap.nodes.len() as u32 - 1
                }
            };
            // Two ways drawn over the same segment would double an edge.
            if let Some(p) = prev.filter(|&p| p != i && seen.insert((p.min(i), p.max(i)))) {
                ap.edges.push(Edge { a: p, b: i, kind, name: name.clone() });
            }
            prev = Some(i);
        }
    }
    let network_edges = ap.edges.len();

    // Runway ends: nodes with exactly one runway edge, two per runway.
    let mut by_name: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    let mut rdeg: HashMap<u32, u32> = HashMap::new();
    for e in ap.edges.iter().filter(|e| e.kind == EdgeKind::Runway) {
        *rdeg.entry(e.a).or_default() += 1;
        *rdeg.entry(e.b).or_default() += 1;
        by_name.entry(&e.name).or_default().extend([e.a, e.b]);
    }
    for (name, mut ns) in by_name {
        ns.sort();
        ns.dedup();
        ns.retain(|n| rdeg[n] == 1);
        match ns[..] {
            [a, b] => ap.runways.push(Runway { name: name.to_string(), ends: [a, b] }),
            _ => errs.push(format!("runway {name}: {} loose ends, need 2", ns.len())),
        }
    }

    let m = |n: Node| (f64::from(n.x) / 10.0, f64::from(n.y) / 10.0);

    // Dead ends close to another segment: likely missing junctions.
    let mut deg = vec![0u32; ap.nodes.len()];
    for e in &ap.edges {
        deg[e.a as usize] += 1;
        deg[e.b as usize] += 1;
    }
    let osm_id: HashMap<u32, u64> = idx.iter().map(|(&o, &i)| (i, o)).collect();
    let mut near_miss = Vec::new();
    for (i, _) in deg.iter().enumerate().filter(|(_, d)| **d == 1) {
        let p = m(ap.nodes[i]);
        let d = ap.edges.iter().filter(|e| e.a as usize != i && e.b as usize != i)
            .map(|e| nearest_on(p, m(ap.nodes[e.a as usize]), m(ap.nodes[e.b as usize])).0)
            .fold(f64::INFINITY, f64::min);
        if d < NEAR_MISS_M {
            near_miss.push(format!("{} ({d:.1} m)", osm_id[&(i as u32)]));
        }
    }

    // Stands. OSM marks the parking spot beside the taxilane, not on it, so
    // each gets a straight link to its nearest taxiway point. Checked against
    // IGI's terminal outlines on 2026-11-05: no link crosses a building.
    let mut stand_ids: Vec<u64> = osm.nodes.iter().filter(|(_, n)| aeroway(&n.2) == "parking_position").map(|(&id, _)| id).collect();
    stand_ids.sort();
    let mut splits: BTreeMap<usize, Vec<(f64, u32)>> = BTreeMap::new();
    let mut links = Vec::new();
    for id in stand_ids {
        let tags = &osm.nodes[&id].2;
        let name = ov.stand.get(&id).or(tags.get("ref")).cloned().unwrap_or_default();
        let at = to_dm(coord(id)?);
        let best = ap.edges[..network_edges].iter().enumerate()
            .filter(|(_, e)| e.kind == EdgeKind::Taxiway)
            .map(|(ei, e)| (ei, nearest_on(m(at), m(ap.nodes[e.a as usize]), m(ap.nodes[e.b as usize]))))
            .min_by(|x, y| x.1.0.total_cmp(&y.1.0));
        let Some((ei, (dist, t, q))) = best else { return Err("no taxiway to link stands to".into()) };
        if dist > MAX_STAND_LINK_M {
            errs.push(format!("stand '{name}' (node {id}) is {dist:.0} m from any taxiway"));
            continue;
        }
        let e = &ap.edges[ei];
        let len = ap.edge_len_m(e);
        let target = if t * len < SNAP_M {
            e.a
        } else if (1.0 - t) * len < SNAP_M {
            e.b
        } else if let Some(&(_, n)) = splits.get(&ei).and_then(|s| s.iter().find(|(st, _)| (st - t).abs() * len < SNAP_M)) {
            n
        } else {
            ap.nodes.push(Node { x: (q.0 * 10.0).round() as i32, y: (q.1 * 10.0).round() as i32 });
            let n = ap.nodes.len() as u32 - 1;
            splits.entry(ei).or_default().push((t, n));
            n
        };
        ap.nodes.push(at);
        let s = ap.nodes.len() as u32 - 1;
        ap.stands.push(Stand { name, node: s });
        links.push(Edge { a: s, b: target, kind: EdgeKind::StandLink, name: String::new() });
    }
    // Split each segment a stand link lands on, in order along it.
    let mut edges = Vec::with_capacity(ap.edges.len() + 2 * links.len());
    for (ei, e) in ap.edges.iter().enumerate() {
        let mut at = e.a;
        if let Some(s) = splits.get_mut(&ei) {
            s.sort_by(|x, y| x.0.total_cmp(&y.0));
            for &(_, n) in s.iter() {
                edges.push(Edge { a: at, b: n, ..e.clone() });
                at = n;
            }
        }
        edges.push(Edge { a: at, b: e.b, ..e.clone() });
    }
    edges.extend(links);
    ap.edges = edges;

    // Holding points must sit on the network, or no route can stop at one.
    let mut hold_ids: Vec<u64> = osm.nodes.iter().filter(|(_, n)| aeroway(&n.2) == "holding_position").map(|(&id, _)| id).collect();
    hold_ids.sort();
    for id in hold_ids {
        let name = ov.hold.get(&id).or(osm.nodes[&id].2.get("ref")).cloned().unwrap_or_default();
        match idx.get(&id) {
            Some(&node) => ap.holds.push(Hold { name, node }),
            None => errs.push(format!("holding point {id} is not on any taxiway")),
        }
    }

    if !errs.is_empty() {
        return Err(errs.join("\n"));
    }
    let pockets = drop_pockets(&mut ap);

    // The report: what the data lacks, in numbers.
    let twy: Vec<&Edge> = ap.edges.iter().filter(|e| e.kind == EdgeKind::Taxiway).collect();
    let named = twy.iter().filter(|e| !e.name.is_empty()).count();
    let mut names: Vec<&str> = twy.iter().map(|e| e.name.as_str()).filter(|n| !n.is_empty()).collect();
    names.sort();
    names.dedup();
    let mut link_m: Vec<(f64, &str)> = ap.edges.iter().filter(|e| e.kind == EdgeKind::StandLink)
        .map(|e| ap.edge_len_m(e))
        .zip(ap.stands.iter().map(|s| s.name.as_str()))
        .collect();
    link_m.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut report = vec![
        format!("OSM base {}", ap.osm_base),
        format!("{} nodes, {} edges", ap.nodes.len(), ap.edges.len()),
        format!(
            "taxiway edges named: {named} of {} ({:.0}%), {} distinct names",
            twy.len(),
            100.0 * named as f64 / twy.len().max(1) as f64,
            names.len()
        ),
    ];
    for r in &ap.runways {
        let len: f64 = ap.edges.iter().filter(|e| e.kind == EdgeKind::Runway && e.name == r.name).map(|e| ap.edge_len_m(e)).sum();
        report.push(format!("runway {}: {len:.0} m end to end, displaced thresholds included", r.name));
    }
    if let (Some(mid), Some(last)) = (link_m.get(link_m.len() / 2), link_m.last()) {
        report.push(format!(
            "stands: {} ({} named), link median {:.0} m, longest {:.0} m (stand '{}')",
            ap.stands.len(),
            ap.stands.iter().filter(|s| !s.name.is_empty()).count(),
            mid.0,
            last.0,
            last.1
        ));
    }
    report.push(format!(
        "holding points: {}, named {}",
        ap.holds.len(),
        ap.holds.iter().filter(|h| !h.name.is_empty()).count()
    ));
    report.push(format!(
        "taxiway pockets dropped, no stand and reachable only along a runway: {}{}",
        pockets.len(),
        if pockets.is_empty() { String::new() } else { format!(" — {}", pockets.join("; ")) }
    ));
    report.push(format!(
        "dead ends within {NEAR_MISS_M} m of another segment (possible missing junctions, not joined): {}{}",
        near_miss.len(),
        if near_miss.is_empty() { String::new() } else { format!(" — OSM nodes {}", near_miss.join(", ")) }
    ));
    Ok((ap, report))
}

/// Remove taxiway pockets that hold no stand: groups of taxiway that reach
/// the rest of the field only by rolling along a runway. IGI has one, taxiway
/// A3 north of 09/27 (35 nodes, 2026-11-05), probably the Air Force side;
/// that guess is unconfirmed. An arrival cleared off the runway there could
/// only taxi into a dead end, so the game never offers it. Returns one line
/// per pocket for the report: its names and size.
fn drop_pockets(ap: &mut Airport) -> Vec<String> {
    let comp = ap.components(|e| e.kind != EdgeKind::Runway);
    let has_stand: std::collections::HashSet<u32> = ap.stands.iter().map(|s| comp[s.node as usize]).collect();
    let dead = |n: u32| !has_stand.contains(&comp[n as usize]);

    let mut pockets: BTreeMap<u32, (Vec<String>, usize)> = BTreeMap::new();
    for e in ap.edges.iter().filter(|e| e.kind != EdgeKind::Runway && dead(e.a)) {
        let p = pockets.entry(comp[e.a as usize]).or_default();
        p.1 += 1;
        if !e.name.is_empty() && !p.0.contains(&e.name) {
            p.0.push(e.name.clone());
        }
    }
    ap.edges.retain(|e| e.kind == EdgeKind::Runway || !dead(e.a));
    ap.holds.retain(|h| !dead(h.node));

    // Renumber: drop nodes no edge uses any more, keep the order of the rest.
    let mut used = vec![false; ap.nodes.len()];
    for e in &ap.edges {
        used[e.a as usize] = true;
        used[e.b as usize] = true;
    }
    let mut new_id = vec![u32::MAX; ap.nodes.len()];
    let mut kept = Vec::new();
    for (i, n) in ap.nodes.iter().enumerate().filter(|(i, _)| used[*i]) {
        new_id[i] = kept.len() as u32;
        kept.push(*n);
    }
    ap.nodes = kept;
    let re = |n: &mut u32| *n = new_id[*n as usize];
    for e in &mut ap.edges {
        re(&mut e.a);
        re(&mut e.b);
    }
    ap.runways.iter_mut().for_each(|r| r.ends.iter_mut().for_each(re));
    ap.stands.iter_mut().for_each(|s| re(&mut s.node));
    ap.holds.iter_mut().for_each(|h| re(&mut h.node));

    pockets
        .into_values()
        .map(|(names, edges)| {
            let names = if names.is_empty() { "unnamed".to_string() } else { names.join(" ") };
            format!("{names} ({edges} edges)")
        })
        .collect()
}

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// Read the raw file and overrides for `icao` and extract. A missing
/// overrides file counts as empty: a fresh airport has nothing typed in yet.
fn run(icao: &str) -> Result<(Airport, Vec<String>), String> {
    let lower = icao.to_lowercase();
    let dir = data_dir();
    let raw_path = dir.join(format!("{lower}.osm.json"));
    let raw = std::fs::read_to_string(&raw_path).map_err(|e| format!("{}: {e}", raw_path.display()))?;
    let ov = match std::fs::read_to_string(dir.join(format!("{lower}.overrides.toml"))) {
        Ok(t) => parse_overrides(&t)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Overrides::default(),
        Err(e) => return Err(e.to_string()),
    };
    extract(&icao.to_uppercase(), &parse_osm(&raw)?, &ov)
}

fn main() {
    let Some(icao) = std::env::args().nth(1) else {
        eprintln!("usage: twr-osm <ICAO>   e.g. twr-osm VIDP");
        std::process::exit(2);
    };
    match run(&icao) {
        Ok((ap, report)) => {
            let out = data_dir().join(format!("{}.twr", icao.to_lowercase()));
            if let Err(e) = std::fs::write(&out, ap.to_text()) {
                eprintln!("{}: {e}", out.display());
                std::process::exit(1);
            }
            println!("wrote {}", out.display());
            for line in report {
                println!("  {line}");
            }
        }
        Err(e) => {
            eprintln!("{icao}: refused\n{e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed graph must be exactly what the raw data and overrides
    /// produce. Fails when someone edits the overrides and forgets to re-run.
    #[test]
    fn vidp_twr_is_current() {
        let (ap, _) = run("VIDP").unwrap();
        let on_disk = std::fs::read_to_string(data_dir().join("vidp.twr")).unwrap();
        assert!(ap.to_text() == on_disk, "data/vidp.twr is stale: run `cargo run -p twr-osm -- VIDP`");
    }

    #[test]
    fn vidp_every_named_taxiway_present() {
        let raw = std::fs::read_to_string(data_dir().join("vidp.osm.json")).unwrap();
        let osm = parse_osm(&raw).unwrap();
        let (ap, _) = run("VIDP").unwrap();
        let mut want: Vec<&str> = osm.ways.iter()
            .filter(|w| matches!(aeroway(&w.tags), "taxiway" | "taxilane" | "parking_position"))
            .filter_map(|w| w.tags.get("ref").map(String::as_str))
            .collect();
        let ov = parse_overrides(&std::fs::read_to_string(data_dir().join("vidp.overrides.toml")).unwrap()).unwrap();
        want.extend(ov.taxiway.values().map(String::as_str));
        want.sort();
        want.dedup();
        // A3 lies only in the pocket north of 09/27 that drop_pockets removes.
        want.retain(|n| *n != "A3");
        let mut got: Vec<&str> = ap.edges.iter().filter(|e| e.kind == EdgeKind::Taxiway && !e.name.is_empty()).map(|e| e.name.as_str()).collect();
        got.sort();
        got.dedup();
        assert_eq!(got, want);
        assert!(got.len() >= 100, "IGI had 109 usable names on 2026-11-05; got {}", got.len());
    }

    /// A small hand-built field: one runway, one 100 m taxiway meeting it,
    /// two stands beside the taxiway and one exactly beside its end node.
    fn toy() -> String {
        // 0.0009 degrees of latitude is about 100 m.
        r#"{"osm3s":{"timestamp_osm_base":"2026-01-01T00:00:00Z"},"elements":[
          {"type":"node","id":1,"lat":0.0,"lon":0.0},
          {"type":"node","id":2,"lat":0.0009,"lon":0.0},
          {"type":"node","id":3,"lat":0.0,"lon":0.0018},
          {"type":"node","id":4,"lat":0.0,"lon":-0.0018},
          {"type":"node","id":10,"lat":0.0003,"lon":0.0004,"tags":{"aeroway":"parking_position","ref":"1"}},
          {"type":"node","id":11,"lat":0.0006,"lon":0.0004,"tags":{"aeroway":"parking_position","ref":"2"}},
          {"type":"node","id":12,"lat":0.0009,"lon":0.0004,"tags":{"aeroway":"parking_position","ref":"3"}},
          {"type":"node","id":13,"lat":0.0009,"lon":0.0,"tags":{"aeroway":"holding_position"}},
          {"type":"way","id":100,"nodes":[4,1,3],"tags":{"aeroway":"runway","ref":"09/27"}},
          {"type":"way","id":101,"nodes":[1,2],"tags":{"aeroway":"taxiway","ref":"A"}}
        ]}"#
        .to_string()
    }

    #[test]
    fn stands_split_their_segment_once_each_in_order() {
        let osm = parse_osm(&toy()).unwrap();
        // Node 13 is a hold but not on the network: refused, by name.
        let err = extract("TOY", &osm, &Overrides::default()).unwrap_err();
        assert!(err.contains("holding point 13 is not on any taxiway"), "{err}");

        // Node 13 sits where node 2 does; route the taxiway through it instead.
        let fixed = toy().replace(r#""nodes":[1,2]"#, r#""nodes":[1,13]"#);
        let (ap, _) = extract("TOY", &parse_osm(&fixed).unwrap(), &Overrides::default()).unwrap();
        // Taxiway A: 1 -> s1 -> s2 -> 13. Stand 3 lands on node 13 itself.
        let a: Vec<&Edge> = ap.edges.iter().filter(|e| e.name == "A").collect();
        assert_eq!(a.len(), 3, "{a:?}");
        assert_eq!(a[0].b, a[1].a);
        assert_eq!(a[1].b, a[2].a);
        let links: Vec<&Edge> = ap.edges.iter().filter(|e| e.kind == EdgeKind::StandLink).collect();
        assert_eq!(links.len(), 3);
        assert_eq!(links[2].b, ap.holds[0].node, "stand 3 snaps to the hold node, no sliver");
        for l in &links {
            assert!((ap.edge_len_m(l) - 44.5).abs() < 1.0, "link {:.1} m", ap.edge_len_m(l));
        }
        assert_eq!(ap.runways.len(), 1);
        assert_eq!(ap.runways[0].name, "09/27");
        assert_eq!(ap.runway_exits().len(), 1, "taxiway A meets the runway once");
    }

    /// Taxiway B leaves the runway's far side and goes nowhere with a stand:
    /// it must be dropped, along with its nodes, and everything renumbered.
    #[test]
    fn pocket_with_no_stand_is_dropped() {
        let base = toy().replace(r#""nodes":[1,2]"#, r#""nodes":[1,13]"#);
        let (before, _) = extract("TOY", &parse_osm(&base).unwrap(), &Overrides::default()).unwrap();
        let pocket = base.replace(
            r#"{"type":"way","id":101"#,
            r#"{"type":"node","id":20,"lat":-0.0009,"lon":0.0018},
               {"type":"way","id":102,"nodes":[3,20],"tags":{"aeroway":"taxiway","ref":"B"}},
               {"type":"way","id":101"#,
        );
        let (ap, report) = extract("TOY", &parse_osm(&pocket).unwrap(), &Overrides::default()).unwrap();
        assert!(report.iter().any(|l| l.ends_with(": 1 — B (1 edges)")), "{report:?}");
        assert_eq!(ap.nodes.len(), before.nodes.len());
        assert!(ap.edges.iter().all(|e| e.name != "B"));
        assert_eq!(ap.runways[0].ends, before.runways[0].ends, "runway end kept");
        assert_eq!(ap.to_text().lines().count(), before.to_text().lines().count());
    }

    #[test]
    fn stale_or_misplaced_overrides_are_refused() {
        let osm = parse_osm(&toy()).unwrap();
        let ov = parse_overrides("[taxiway]\n999 = \"B\"\n[stand]\n13 = \"X\"\n").unwrap();
        let err = extract("TOY", &osm, &ov).unwrap_err();
        assert!(err.contains("[taxiway] 999: no such taxiway way"), "{err}");
        assert!(err.contains("13 is not a parking_position node"), "{err}");
        assert!(parse_overrides("[gates]\n1 = \"x\"\n").unwrap_err().contains("unknown section"));
        assert!(parse_overrides("[taxiway]\nA = \"x\"\n").unwrap_err().contains("not an OSM id"));
    }

    #[test]
    fn far_stand_is_refused_not_linked() {
        let far = toy().replace(r#""id":10,"lat":0.0003,"lon":0.0004"#, r#""id":10,"lat":0.0003,"lon":0.004"#);
        let fixed = far.replace(r#""nodes":[1,2]"#, r#""nodes":[1,13]"#);
        let err = extract("TOY", &parse_osm(&fixed).unwrap(), &Overrides::default()).unwrap_err();
        assert!(err.contains("stand '1' (node 10) is"), "{err}");
    }
}
