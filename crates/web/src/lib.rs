//! TWR-WEB · BROWSER SHELL
//!   · App: the wasm object the page drives, one per canvas
//!   · Camera: pan 1:1 with the pointer, zoom on a spring about the cursor
//!   · input: select; verbs P push, T taxi (click destination, click via,
//!     right-click avoid, Enter sends, Esc cancels), H hold, X cross,
//!     L line up, K takeoff; arrivals C clear to land, G go around, E
//!     exit (click the exit node). Clicking a strip selects its aircraft.
//!     The sim decides which verbs are legal.
//!   · draw: runways, taxiways, stands, holds, the route preview, aircraft,
//!     and in red only: live conflicts (ring + joining line) and incidents
//!     of the last INCIDENT_MARK_S game seconds (filled square)
//!   · strips: per-aircraft state, conflict line, wake timer, incident log,
//!     go-around log (plain ink: a go-around is never an incident)
//!   · shifts (B6): `restart` with a mode and seed; HUD reads the rate,
//!     the shift clock in real seconds and the two queues
//!   · test hooks: screen positions and route lists, read-only, for
//!     Playwright; one write hook, `load_scenario`, for the B4 set-ups
//!
//! Thin on purpose: real frame time and input come in from the page's
//! JavaScript, draw calls go out to a Canvas 2D context. No game rule lives
//! here; if it would matter in a headless test, it belongs in `twr-sim`.
//!
//! The preview is the route: `preview` holds the one `Route` the planner
//! returned, it is what `draw` strokes, and `Enter` hands that same value to
//! `World::taxi`. Nothing between the screen and the sim plans again.

use twr_sim::airport::EdgeKind;
use twr_sim::conflict::INCIDENT_MARK_S;
use twr_sim::route::Route;
use twr_sim::traffic::{SHIFT_REAL_S, TICKS_PER_REAL_S};
use twr_sim::{Phase, World, TICK_HZ};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Click targets reach this far, in CSS px, from what they hit: a 44 px
/// target (UI tip `hit-slop`) around an aircraft drawn 10 px wide.
const HIT_PX: f64 = 22.0;
/// Zoom limits in CSS px per metre. 0.05 shows a 30 km field on a laptop;
/// 8 puts a stand at the width of a thumb.
const ZOOM_MIN: f64 = 0.05;
const ZOOM_MAX: f64 = 8.0;
/// Log-zoom per wheel pixel. One mouse notch (100 px) zooms by e^0.2 = 1.22x.
const ZOOM_PER_WHEEL_PX: f64 = 0.002;
/// Zoom spring, on log scale. Critically damped: settles in about 250 ms
/// without overshoot, and a new wheel notch retargets it mid-flight.
const ZOOM_STIFFNESS: f64 = 300.0;
const ZOOM_DAMPING: f64 = 34.64; // 2 * sqrt(stiffness): critical
/// Aircraft glyph half-length in CSS px. Drawn at a fixed size so it stays
/// clickable at full zoom-out, where a 40 m airliner is two pixels long.
const AIRCRAFT_PX: f64 = 7.0;

struct Camera {
    /// World point at the canvas centre, metres.
    cx: f64,
    cy: f64,
    /// ln(CSS px per metre), its spring target and velocity.
    z: f64,
    zt: f64,
    vz: f64,
    /// Zoom pivot: a screen point and the world point that must stay under
    /// it while the spring runs.
    pivot: Option<(f64, f64, f64, f64)>,
    fitted: bool,
}

impl Camera {
    fn scale(&self) -> f64 {
        self.z.exp()
    }
}

enum Mode {
    Idle,
    /// Building a taxi clearance for the selected aircraft.
    Taxi { dest: Option<u32>, via: Vec<u32>, avoid: Vec<u32>, preview: Option<Route>, note: String },
    /// After E: the next click picks the selected arrival's exit.
    Exit,
}

/// Route Rust panics to the console with their message. Without this a
/// panic surfaces as an opaque "unreachable executed" and the cause is lost.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!("twr panic: {info}")));
    }));
}

#[wasm_bindgen]
pub struct App {
    world: World,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    cam: Camera,
    /// Canvas size in CSS px, and the device pixel ratio, from the last frame.
    w: f64,
    h: f64,
    dpr: f64,
    selected: Option<usize>,
    mode: Mode,
    hover_edge: Option<u32>,
    /// Why the last verb was refused, shown on the strip until the next key.
    notice: String,
}

#[wasm_bindgen]
impl App {
    /// Bind to the canvas with the given id. The seed crosses as u32 because
    /// u64 would arrive in JS as a BigInt; 4 billion seeds is plenty.
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str, seed: u32) -> Result<App, JsValue> {
        install_panic_hook();
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: HtmlCanvasElement = doc
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("no element #{canvas_id}")))?
            .dyn_into()
            .map_err(|_| JsValue::from_str(&format!("#{canvas_id} is not a canvas")))?;
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("2d context unavailable"))?
            .dyn_into()?;
        let cam = Camera { cx: 0.0, cy: 0.0, z: 0.0, zt: 0.0, vz: 0.0, pivot: None, fitted: false };
        Ok(App {
            world: World::new(u64::from(seed)),
            canvas,
            ctx,
            cam,
            w: 1.0,
            h: 1.0,
            dpr: 1.0,
            selected: None,
            mode: Mode::Idle,
            hover_edge: None,
            notice: String::new(),
        })
    }

    /// One animation frame: advance the sim by `real_ms` (0 while paused),
    /// run the camera spring, draw. Colours come from the page's CSS custom
    /// properties so both themes are drawn by the same code.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, real_ms: f64, dpr: f64, reduce: bool, bg: &str, fg: &str, dim: &str, line: &str, red: &str) -> u32 {
        self.dpr = dpr;
        self.w = f64::from(self.canvas.width()) / dpr;
        self.h = f64::from(self.canvas.height()) / dpr;
        if !self.cam.fitted {
            self.fit();
        }
        let steps = self.world.frame(real_ms);
        self.spring(real_ms, reduce);
        self.draw(bg, fg, dim, line, red);
        steps
    }

    pub fn ticks(&self) -> f64 {
        self.world.clock.ticks() as f64
    }

    pub fn game_seconds(&self) -> f64 {
        self.world.clock.game_seconds()
    }

    pub fn seed(&self) -> u32 {
        self.world.seed as u32
    }

    /// Start over: a fresh world for `mode` ("practice", "light",
    /// "normal", "peak", "surge") from `seed`. False for an unknown mode.
    pub fn restart(&mut self, mode: &str, seed: u32) -> bool {
        let Some(m) = twr_sim::Mode::parse(mode) else { return false };
        self.world = World::shift(u64::from(seed), m);
        self.cam.fitted = false;
        self.selected = None;
        self.mode = Mode::Idle;
        self.notice.clear();
        true
    }

    pub fn shift_mode(&self) -> String {
        self.world.mode().name().into()
    }

    /// Movements per real hour offered now; 0 in Practice.
    pub fn rate_per_h(&self) -> f64 {
        self.world.mode().rate_milli(self.world.tick) as f64 / 1000.0
    }

    /// Real seconds since the shift began, from the tick count.
    pub fn real_s(&self) -> f64 {
        (self.world.tick / TICKS_PER_REAL_S) as f64
    }

    /// Real seconds a Shift lasts; 0 for Practice and Surge, which are open.
    pub fn shift_len_s(&self) -> f64 {
        match self.world.mode() {
            twr_sim::Mode::Practice | twr_sim::Mode::Surge => 0.0,
            _ => SHIFT_REAL_S as f64,
        }
    }

    /// Arrivals held by approach until their final is free.
    pub fn final_queue(&self) -> usize {
        self.world.traffic.as_ref().map_or(0, |t| t.final_queue.len())
    }

    /// Departures waiting for a free stand to appear on.
    pub fn stand_queue(&self) -> usize {
        self.world.traffic.as_ref().map_or(0, |t| t.stand_queue.len())
    }

    // ---- Score (B7), read by the page each frame ----------------------------

    /// Takeoffs plus landings.
    pub fn movements(&self) -> u32 {
        self.world.score.movements
    }

    /// Movements the shift has offered so far.
    pub fn offered(&self) -> usize {
        self.world.offered()
    }

    /// Delay you caused, in game minutes.
    pub fn delay_min(&self) -> f64 {
        self.world.score.delay_min()
    }

    /// Aircraft whose waits add up past a game minute.
    pub fn delayed(&self) -> usize {
        self.world.score.delayed()
    }

    /// The shift has ended; the world no longer moves.
    pub fn over(&self) -> bool {
        self.world.score.over.is_some()
    }

    /// Real seconds from the start to the end of the shift; 0 until it ends.
    pub fn held_s(&self) -> f64 {
        self.world.score.over.map_or(0.0, |t| (t / TICKS_PER_REAL_S) as f64)
    }

    /// Departed or shut down: done, its strip can go.
    pub fn gone(&self, i: usize) -> bool {
        matches!(self.world.aircraft[i].phase, Phase::Departed(_) | Phase::ShutDown)
    }

    /// Callsigns of every movement the traffic has spawned, in order.
    pub fn spawn_log(&self) -> Vec<String> {
        self.world.traffic.as_ref().map_or(Vec::new(), |t| t.log.iter().map(|d| d.callsign.clone()).collect())
    }

    /// B9: the shift's wind, METAR-style (`290/12`, `100/08G21`, `calm`).
    pub fn wind_text(&self) -> String {
        self.world.wind().text()
    }

    /// B9: "west" or "east".
    pub fn flow(&self) -> String {
        self.world.wind().flow().name().into()
    }

    /// B9: the runways in use, for the status bar's tooltip and the tests.
    pub fn runways_text(&self) -> String {
        let f = self.world.wind().flow();
        format!("land {} · depart {}", f.arr_rwys().join(" "), f.dep_rwys().join(" "))
    }

    /// B9: radio telephony and type for a strip's tooltip: `IFLY 512 · A20N`.
    /// Callsign alone when the operator has none on file.
    pub fn telephony(&self, i: usize) -> String {
        let a = &self.world.aircraft[i];
        let (pre, num) = a.callsign.split_at(a.callsign.len().min(3));
        let said = twr_sim::traffic::OPERATORS.iter().find(|o| o.icao == pre).and_then(|o| o.telephony)
            .map_or(a.callsign.clone(), |t| format!("{t} {num}"));
        format!("{said} · {}", a.type_code)
    }

    pub fn osm_base(&self) -> String {
        self.world.airport.osm_base.clone()
    }

    // ---- Input -------------------------------------------------------------

    /// Drag: move the map 1:1 with the pointer, CSS px.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let s = self.cam.scale();
        self.cam.cx -= dx / s;
        self.cam.cy += dy / s;
        // Keep a running zoom pivoting on the same screen point.
        if let Some((sx, sy, _, _)) = self.cam.pivot {
            let (wx, wy) = self.to_world(sx, sy);
            self.cam.pivot = Some((sx, sy, wx, wy));
        }
    }

    /// Wheel at (x, y) CSS px. Retargets the spring; never queues.
    pub fn wheel(&mut self, x: f64, y: f64, delta_y: f64) {
        let dz = (-delta_y.clamp(-200.0, 200.0)) * ZOOM_PER_WHEEL_PX;
        self.cam.zt = (self.cam.zt + dz).clamp(ZOOM_MIN.ln(), ZOOM_MAX.ln());
        let (wx, wy) = self.to_world(x, y);
        self.cam.pivot = Some((x, y, wx, wy));
    }

    pub fn hover(&mut self, x: f64, y: f64) {
        self.hover_edge = self.edge_at(x, y);
    }

    /// A click that was not a drag. `button`: 0 left, 2 right.
    pub fn click(&mut self, x: f64, y: f64, button: i16) {
        let dest_set = match &self.mode {
            Mode::Idle => {
                if button == 0 {
                    self.selected = self.aircraft_at(x, y);
                }
                return;
            }
            Mode::Taxi { dest, .. } => dest.is_some(),
            Mode::Exit => {
                if button != 0 {
                    return;
                }
                let Some(n) = self.node_at(x, y) else { return };
                let i = self.selected.expect("exit mode needs a selection");
                self.mode = Mode::Idle;
                self.notice = self.world.set_exit(i, n).err().unwrap_or_default();
                return;
            }
        };
        if !dest_set {
            // First click in taxi mode picks where the route ends.
            let Some(n) = self.node_at(x, y).filter(|_| button == 0) else { return };
            if let Mode::Taxi { dest, .. } = &mut self.mode {
                *dest = Some(n);
            }
        } else {
            // Later clicks steer it: left adds the clicked segment as a via
            // (or removes it), right avoids that whole taxiway (or lifts the
            // avoid). Both toggle, so every choice can be undone in place.
            let Some(e) = self.edge_at(x, y) else { return };
            let group = self.world.airport.taxiway_of(e);
            let Mode::Taxi { via, avoid, .. } = &mut self.mode else { return };
            let was_avoided = avoid.contains(&e);
            avoid.retain(|a| !group.contains(a));
            if button == 0 {
                if let Some(i) = via.iter().position(|v| *v == e) {
                    via.remove(i);
                } else {
                    via.push(e);
                }
            } else if !was_avoided {
                via.retain(|v| !group.contains(v));
                avoid.extend(group);
            }
        }
        self.replan();
    }

    /// Keyboard verbs. Returns true when the key was used, so the page can
    /// stop the browser acting on it too. A refused verb says why on the
    /// strip; nothing pops up.
    pub fn key(&mut self, key: &str) -> bool {
        if matches!(self.mode, Mode::Exit) {
            if key != "Escape" {
                return false;
            }
            self.mode = Mode::Idle;
            return true;
        }
        if matches!(self.mode, Mode::Taxi { .. }) {
            match key {
                "Escape" => self.mode = Mode::Idle,
                "Enter" => {
                    let Mode::Taxi { preview: Some(route), .. } = std::mem::replace(&mut self.mode, Mode::Idle) else {
                        return false;
                    };
                    let i = self.selected.expect("taxi mode needs a selection");
                    self.notice = self.world.taxi(i, route).err().unwrap_or_default();
                }
                _ => return false,
            }
            return true;
        }
        let Some(i) = self.selected else { return false };
        let verb = key.to_ascii_uppercase();
        let result = match verb.as_str() {
            "ESCAPE" => {
                self.selected = None;
                Ok(())
            }
            "T" if self.world.verbs(i).contains(&'T') => {
                self.mode = Mode::Taxi { dest: None, via: Vec::new(), avoid: Vec::new(), preview: None, note: String::new() };
                Ok(())
            }
            "T" => Err("can't taxi now".to_string()),
            "P" => self.world.push(i),
            "H" => self.world.hold(i),
            "X" => self.world.cross(i),
            "L" => self.world.line_up(i),
            "K" => self.world.takeoff(i),
            "C" => self.world.clear_to_land(i),
            "G" => self.world.go_around(i),
            "E" if self.world.verbs(i).contains(&'E') => {
                self.mode = Mode::Exit;
                Ok(())
            }
            "E" => Err("only a landing aircraft can be given an exit".to_string()),
            _ => return false,
        };
        self.notice = result.err().unwrap_or_default();
        true
    }

    // ---- Strip text, read by the page each frame ---------------------------

    /// One line for the strip: what aircraft `i` is doing.
    pub fn strip_state(&self, i: usize) -> String {
        let a = &self.world.aircraft[i];
        let (ap, g) = (&self.world.airport, &self.world.graph);
        let m = |v: f64| v.round() as i64;
        let kt = |mdm: u64| (mdm as f64 * 20.0 / 10_000.0 * 1.943_844).round() as i64;
        match &a.phase {
            Phase::OnStand => {
                let stand = ap.stands.iter().find(|s| s.node == a.route.end()).map_or("", |s| s.name.as_str());
                match a.assigned {
                    Some(r) => format!("On stand {stand} · dep {r}"),
                    None => format!("On stand {stand}"),
                }
            }
            Phase::ShutDown => "Shut down".into(),
            Phase::Pushback => format!("Pushing back, {} m", m(a.metres_left(g))),
            Phase::Holding => "Holding".into(),
            Phase::Taxi => format!("Taxi, {} m to go", m(a.metres_left(g))),
            Phase::HoldPosition => "Holding position".into(),
            Phase::HoldShort(r) => format!("Holding short {r}"),
            Phase::LineUp(r) => format!("Lining up {r}"),
            Phase::LinedUp(d) => format!("Lined up {d}, {} m ahead", m(a.metres_left(g))),
            Phase::TakeoffRoll(d) => format!("Takeoff roll {d}, {} kt", kt(a.speed_mdm)),
            Phase::Airborne(d) => format!("Airborne {d}, {} kt", kt(a.speed_mdm)),
            Phase::Departed(d) => format!("Departed {d}"),
            Phase::Landing(d) => match &a.exit {
                Some(x) => format!("Landing roll {d}, {} kt · exit {}", kt(a.speed_mdm), x.label()),
                None => format!("Landing roll {d}, {} kt", kt(a.speed_mdm)),
            },
            Phase::Final(d) => {
                let clr = if a.cleared_to_land { "cleared to land" } else { "not cleared" };
                let exit = a.exit.as_ref().map_or(String::new(), |x| format!(" · exit {}", x.label()));
                format!("Final {d} · {:.1} km · {clr}{exit}", a.final_m() / 1000.0)
            }
            Phase::GoAround(d) if a.visible() => format!("Going around {d}, {} kt", kt(a.speed_mdm)),
            Phase::GoAround(d) => format!("Going around {d} · final again in {}", mmss(a.circuit_left())),
        }
    }

    pub fn count(&self) -> usize {
        self.world.aircraft.len()
    }

    /// Type and wake category, for the strip's corner: "A21N M".
    pub fn wake(&self, i: usize) -> String {
        let a = &self.world.aircraft[i];
        format!("{} {:?}", a.type_code, a.wake)
    }

    /// The live conflict aircraft `i` is in, as the strip's red line; empty
    /// when none.
    pub fn conflict_text(&self, i: usize) -> String {
        let c = self.world.monitor.conflicts.iter().find(|c| c.a == i || c.b == i);
        c.map(|c| {
            let other = if c.a == i { c.b } else { c.a };
            format!("CONFLICT {} · {:.0} m", self.world.aircraft[other].callsign, c.d_m)
        })
        .unwrap_or_default()
    }

    /// True while an incident involving aircraft `i` is still on the map.
    pub fn incident_tag(&self, i: usize) -> bool {
        let cs = &self.world.aircraft[i].callsign;
        self.recent_incidents().any(|x| &x.a == cs || &x.b == cs)
    }

    /// Neutral note: the wake timer, or who is in the way of a K now.
    pub fn strip_note(&self, i: usize) -> String {
        let a = &self.world.aircraft[i];
        match a.phase {
            Phase::LinedUp(_) => {}
            Phase::Final(_) => return self.world.runway_blocker(i).map(|by| format!("rwy busy · {by}")).unwrap_or_default(),
            Phase::GoAround(_) => {
                return a.went_around.map_or(String::new(), |w| {
                    if w.challenge() { format!("went around: {} · challenge", w.text()) } else { format!("went around: {}", w.text()) }
                })
            }
            _ => return String::new(),
        }
        let mut parts = Vec::new();
        if let Some((t, by)) = self.world.wake_wait(i) {
            parts.push(format!("wake {} · {by}", mmss(t)));
        }
        if let Some(by) = self.world.runway_blocker(i) {
            parts.push(format!("rwy busy · {by}"));
        }
        parts.join(" · ")
    }

    pub fn incident_count(&self) -> usize {
        self.world.monitor.incidents.len()
    }

    /// Incident log, newest first: "hh:mm:ss text", game time.
    pub fn incident_rows(&self) -> Vec<String> {
        self.world.monitor.incidents.iter().rev().map(|x| format!("{} {}", hhmmss(x.tick), x.text)).collect()
    }

    /// Rule names of every incident, oldest first, e.g. "Ground,Wake".
    pub fn incident_rules(&self) -> String {
        self.world.monitor.incidents.iter().map(|x| format!("{:?}", x.rule)).collect::<Vec<_>>().join(",")
    }

    pub fn go_around_count(&self) -> usize {
        self.world.go_arounds.len()
    }

    /// Go-arounds the pilot or the weather caused: challenges, not counted
    /// against you.
    pub fn challenge_count(&self) -> usize {
        self.world.go_arounds.iter().filter(|g| g.why.challenge()).count()
    }

    /// Go-around log, newest first: "hh:mm:ss GO AROUND · IGO713 (pilot:
    /// unstable approach) · challenge".
    pub fn go_around_rows(&self) -> Vec<String> {
        self.world
            .go_arounds
            .iter()
            .rev()
            .map(|g| {
                let tag = if g.why.challenge() { " · challenge" } else { "" };
                format!("{} GO AROUND · {} ({}){tag}", hhmmss(g.tick), g.callsign, g.why.text())
            })
            .collect()
    }

    /// Select aircraft `i`, as clicking its strip does.
    pub fn select(&mut self, i: usize) {
        if i < self.world.aircraft.len() {
            self.selected = Some(i);
            self.mode = Mode::Idle;
            self.notice.clear();
        }
    }

    /// Runway node of aircraft `i`'s exit, or -1.
    pub fn exit_node(&self, i: usize) -> i64 {
        self.world.aircraft[i].exit.as_ref().map_or(-1, |x| i64::from(x.node))
    }

    /// Runway nodes of every usable exit off aircraft `i`'s landing
    /// direction, in order down the runway.
    pub fn exit_nodes(&self, i: usize) -> Vec<u32> {
        let a = &self.world.aircraft[i];
        let d = match &a.phase {
            Phase::Final(d) | Phase::Landing(d) => d,
            _ => return Vec::new(),
        };
        twr_sim::arrival::exits(&self.world.airport, &self.world.graph, d).into_iter().map(|(x, _)| x.node).collect()
    }

    pub fn conflict_count(&self) -> usize {
        self.world.monitor.conflicts.len()
    }

    /// Index of the aircraft with this callsign, or -1.
    pub fn index_of(&self, callsign: &str) -> i32 {
        self.world.aircraft.iter().position(|a| a.callsign == callsign).map_or(-1, |i| i as i32)
    }

    /// Test only: replace the world with a named B4 set-up from
    /// `World::scenario`, keeping the seed. The page never calls it.
    pub fn load_scenario(&mut self, name: &str) -> bool {
        match World::scenario(self.world.seed, name) {
            Some(w) => {
                self.world = w;
                // A new airport picture: frame it again, or the aircraft
                // can sit off-canvas under the last scenario's zoom.
                self.cam.fitted = false;
                self.selected = None;
                self.mode = Mode::Idle;
                self.notice.clear();
                true
            }
            None => false,
        }
    }

    /// Which keys and clicks do something now, for the strip's hint line.
    pub fn hint(&self) -> String {
        let Some(i) = self.selected else { return "Click an aircraft to select it".into() };
        match &self.mode {
            Mode::Idle => {
                let words = |c: char| match c {
                    'P' => "P push".to_string(),
                    'T' => "T taxi".into(),
                    'H' => "H hold".into(),
                    'X' => "X cross".into(),
                    'L' => "L line up".into(),
                    'C' => match self.strip_note(i) {
                        n if n.is_empty() => "C clear to land".into(),
                        n => format!("C clear to land ({n})"),
                    },
                    'G' => "G go around".into(),
                    'E' => "E exit".into(),
                    // K is never refused on separation; the hint says what it
                    // would break, in plain ink (PLAN-twr.md § B4).
                    'K' => match self.strip_note(i) {
                        n if n.is_empty() => "K takeoff".into(),
                        n => format!("K takeoff ({n})"),
                    },
                    _ => String::new(),
                };
                let mut parts: Vec<String> = self.world.verbs(i).into_iter().map(words).collect();
                parts.push("Esc deselect".into());
                parts.join(" · ")
            }
            Mode::Taxi { dest: None, .. } => "Click a stand, holding point or runway entry · Esc cancel".into(),
            Mode::Taxi { preview: None, note, .. } => format!("{note} · click it again to undo · Esc cancel"),
            Mode::Taxi { .. } => "Enter send · click taxiway: via · right-click: avoid, again to undo · Esc cancel".into(),
            Mode::Exit => "Click an exit along the runway · Esc cancel".into(),
        }
    }

    /// Why the last verb was refused; empty when it was not.
    pub fn notice(&self) -> String {
        self.notice.clone()
    }

    /// Callsign of the selected aircraft, empty when none.
    pub fn selected_callsign(&self) -> String {
        self.selected.map(|i| self.world.aircraft[i].callsign.clone()).unwrap_or_default()
    }

    pub fn callsign(&self, i: usize) -> String {
        self.world.aircraft[i].callsign.clone()
    }

    /// The taxiway under the pointer, for the status bar.
    pub fn hover_name(&self) -> String {
        self.hover_edge.map(|e| self.world.airport.edges[e as usize].name.clone()).unwrap_or_default()
    }

    /// Via list as taxiway names, in order, for the strip.
    pub fn via_names(&self) -> String {
        match &self.mode {
            Mode::Taxi { via, .. } => {
                via.iter().map(|&e| self.name_or_dash(e)).collect::<Vec<_>>().join(" ")
            }
            Mode::Idle | Mode::Exit => String::new(),
        }
    }

    // ---- Test hooks: read-only views for Playwright ------------------------

    /// Edges of the route preview on screen; empty when there is none.
    pub fn preview_edges(&self) -> Vec<u32> {
        match &self.mode {
            Mode::Taxi { preview: Some(r), .. } => r.edges.clone(),
            _ => Vec::new(),
        }
    }

    /// Destination node picked in taxi mode, or -1.
    pub fn taxi_dest(&self) -> i64 {
        match &self.mode {
            Mode::Taxi { dest: Some(n), .. } => i64::from(*n),
            _ => -1,
        }
    }

    /// Edges aircraft `i` has entered, in order.
    pub fn trail(&self, i: usize) -> Vec<u32> {
        self.world.aircraft[i].trail.clone()
    }

    pub fn moving(&self, i: usize) -> bool {
        self.world.aircraft[i].on_edge()
    }

    /// Phase of aircraft `i` as the strip shows it.
    pub fn phase(&self, i: usize) -> String {
        format!("{:?}", self.world.aircraft[i].phase)
    }

    /// Verb keys aircraft `i` accepts now, e.g. "TX".
    pub fn verbs(&self, i: usize) -> String {
        self.world.verbs(i).into_iter().collect()
    }

    /// Runway entries with at least `min_m` metres of runway ahead.
    pub fn departure_entries(&self, min_m: f64) -> Vec<u32> {
        let (ap, g) = (&self.world.airport, &self.world.graph);
        ap.runway_exits()
            .into_iter()
            .filter(|&n| g.runway_run(ap, n).is_some_and(|(r, _)| g.run_len_dm(&r) as f64 / 10.0 >= min_m))
            .collect()
    }

    /// B9: entries onto `desig` with at least `min_m` metres ahead, so a
    /// script can taxi a departure to its assigned runway.
    pub fn entries_for(&self, desig: &str, min_m: f64) -> Vec<u32> {
        let (ap, g) = (&self.world.airport, &self.world.graph);
        ap.runway_exits()
            .into_iter()
            .filter(|&n| g.runway_run(ap, n).is_some_and(|(r, d)| d == desig && g.run_len_dm(&r) as f64 / 10.0 >= min_m))
            .collect()
    }

    /// Final node of aircraft `i`'s current route.
    pub fn route_end(&self, i: usize) -> u32 {
        self.world.aircraft[i].route.end()
    }

    /// Screen position, CSS px, of aircraft `i`: [x, y].
    pub fn aircraft_screen(&self, i: usize) -> Vec<f64> {
        let (x, y) = self.world.aircraft[i].pos_m(&self.world.airport, &self.world.graph);
        let (sx, sy) = self.to_screen(x, y);
        vec![sx, sy]
    }

    /// Screen position, CSS px, of node `n`: [x, y].
    pub fn node_screen(&self, n: u32) -> Vec<f64> {
        let p = self.world.airport.nodes[n as usize];
        let (sx, sy) = self.to_screen(f64::from(p.x) / 10.0, f64::from(p.y) / 10.0);
        vec![sx, sy]
    }

    /// Screen position, CSS px, of the middle of edge `e`: [x, y].
    pub fn edge_screen(&self, e: u32) -> Vec<f64> {
        let ed = &self.world.airport.edges[e as usize];
        let (a, b) = (self.node_screen(ed.a), self.node_screen(ed.b));
        vec![(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
    }

    /// Taxiway edge a click at (x, y) CSS px would hit, or -1.
    pub fn edge_under(&self, x: f64, y: f64) -> i32 {
        self.edge_at(x, y).map_or(-1, |e| e as i32)
    }

    pub fn edge_count(&self) -> u32 {
        self.world.airport.edges.len() as u32
    }

    pub fn hold_nodes(&self) -> Vec<u32> {
        self.world.airport.holds.iter().map(|h| h.node).collect()
    }

    pub fn stand_nodes(&self) -> Vec<u32> {
        self.world.airport.stands.iter().map(|s| s.node).collect()
    }

    pub fn zoom(&self) -> f64 {
        self.cam.scale()
    }

    /// Run `n` sim ticks at once: the same movement code as a frame, without
    /// waiting real time. For tests only; the page never calls it.
    pub fn fast_forward(&mut self, n: u32) {
        self.world.step_ticks(n);
    }
}

impl App {
    fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        let s = self.cam.scale();
        // Sim y grows north; canvas y grows down.
        (self.w / 2.0 + (x - self.cam.cx) * s, self.h / 2.0 - (y - self.cam.cy) * s)
    }

    fn to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        let s = self.cam.scale();
        (self.cam.cx + (sx - self.w / 2.0) / s, self.cam.cy - (sy - self.h / 2.0) / s)
    }

    /// Frame the whole field, once, when the canvas size is first known.
    fn fit(&mut self) {
        let ns = &self.world.airport.nodes;
        let fold = |f: fn(&twr_sim::airport::Node) -> i32| {
            ns.iter().map(f).fold((i32::MAX, i32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        };
        let ((x0, x1), (y0, y1)) = (fold(|n| n.x), fold(|n| n.y));
        let (wm, hm) = (f64::from(x1 - x0) / 10.0, f64::from(y1 - y0) / 10.0);
        self.cam.cx = f64::from(x0 + x1) / 20.0;
        self.cam.cy = f64::from(y0 + y1) / 20.0;
        let s = ((self.w / wm).min(self.h / hm) * 0.92).clamp(ZOOM_MIN, ZOOM_MAX);
        self.cam.z = s.ln();
        self.cam.zt = self.cam.z;
        self.cam.fitted = self.w > 1.0 && self.h > 1.0;
    }

    /// Advance the zoom spring by one frame and hold the pivot still.
    fn spring(&mut self, real_ms: f64, reduce: bool) {
        let c = &mut self.cam;
        if reduce {
            c.z = c.zt;
            c.vz = 0.0;
        } else {
            // Fixed 4 ms substeps keep semi-implicit Euler stable at any frame rate.
            let mut left = (real_ms / 1000.0).clamp(0.0, 0.1);
            while left > 0.0 {
                let h = left.min(0.004);
                c.vz += (ZOOM_STIFFNESS * (c.zt - c.z) - ZOOM_DAMPING * c.vz) * h;
                c.z += c.vz * h;
                left -= h;
            }
            if (c.zt - c.z).abs() < 1e-4 && c.vz.abs() < 1e-3 {
                c.z = c.zt;
                c.vz = 0.0;
            }
        }
        if let Some((sx, sy, wx, wy)) = c.pivot {
            let s = c.z.exp();
            c.cx = wx - (sx - self.w / 2.0) / s;
            c.cy = wy + (sy - self.h / 2.0) / s;
            if c.z == c.zt {
                c.pivot = None;
            }
        }
    }

    fn aircraft_at(&self, x: f64, y: f64) -> Option<usize> {
        (0..self.world.aircraft.len())
            .filter(|&i| self.world.aircraft[i].visible())
            .map(|i| {
                let p = self.aircraft_screen(i);
                (i, (p[0] - x).hypot(p[1] - y))
            })
            .filter(|(_, d)| *d <= HIT_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Nearest node a taxi route can end at, within reach of the click.
    fn node_at(&self, x: f64, y: f64) -> Option<u32> {
        (0..self.world.airport.nodes.len() as u32)
            .filter(|&n| self.world.graph.is_taxi_node(n))
            .map(|n| {
                let p = self.node_screen(n);
                (n, (p[0] - x).hypot(p[1] - y))
            })
            .filter(|(_, d)| *d <= HIT_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(n, _)| n)
    }

    /// Nearest taxiway edge within reach of the pointer. Runways and stand
    /// links are not taxiways a route can be steered through.
    fn edge_at(&self, x: f64, y: f64) -> Option<u32> {
        let ap = &self.world.airport;
        ap.edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EdgeKind::Taxiway)
            .map(|(i, e)| {
                let (a, b) = (self.node_screen(e.a), self.node_screen(e.b));
                (i as u32, seg_dist((x, y), (a[0], a[1]), (b[0], b[1])))
            })
            .filter(|(_, d)| *d <= HIT_PX / 2.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn replan(&mut self) {
        let Some(i) = self.selected else { return };
        let Mode::Taxi { dest: Some(to), via, avoid, preview, note } = &mut self.mode else { return };
        *preview = self.world.plan(i, *to, via, avoid);
        *note = if preview.is_some() { String::new() } else { "No route: an avoid cuts it off, or a via needs a U-turn".into() };
    }

    fn name_or_dash(&self, e: u32) -> String {
        let n = &self.world.airport.edges[e as usize].name;
        if n.is_empty() { "(unnamed)".into() } else { n.clone() }
    }

    // ---- Drawing -----------------------------------------------------------

    /// Stroke a set of edges as one path: one draw call per style, not per edge.
    fn stroke_edges(&self, edges: impl Iterator<Item = u32>, colour: &str, width_px: f64, dash: &[f64]) {
        let (ctx, d) = (&self.ctx, self.dpr);
        ctx.begin_path();
        for e in edges {
            let ed = &self.world.airport.edges[e as usize];
            let (a, b) = (self.node_screen(ed.a), self.node_screen(ed.b));
            ctx.move_to(a[0] * d, a[1] * d);
            ctx.line_to(b[0] * d, b[1] * d);
        }
        let pattern = js_sys::Array::new();
        for v in dash {
            pattern.push(&JsValue::from_f64(v * d));
        }
        let _ = ctx.set_line_dash(&pattern);
        ctx.set_stroke_style_str(colour);
        ctx.set_line_width(width_px * d);
        ctx.stroke();
        let _ = ctx.set_line_dash(&js_sys::Array::new());
    }

    fn recent_incidents(&self) -> impl Iterator<Item = &twr_sim::Incident> {
        let since = self.world.tick.saturating_sub(INCIDENT_MARK_S * u64::from(TICK_HZ));
        self.world.monitor.incidents.iter().filter(move |x| x.tick >= since)
    }

    fn draw(&self, bg: &str, fg: &str, dim: &str, line: &str, red: &str) {
        let (ctx, d) = (&self.ctx, self.dpr);
        ctx.set_fill_style_str(bg);
        ctx.fill_rect(0.0, 0.0, self.w * d, self.h * d);
        ctx.set_line_cap("round");
        let ap = &self.world.airport;
        let s = self.cam.scale();
        let of = |k: EdgeKind| (0..ap.edges.len() as u32).filter(move |&i| ap.edges[i as usize].kind == k);

        // Runways at their real 45-60 m width, never thinner than 3 px.
        self.stroke_edges(of(EdgeKind::Runway), line, (45.0 * s).max(3.0), &[]);
        self.stroke_edges(of(EdgeKind::Runway), dim, 1.0, &[6.0, 6.0]);
        self.stroke_edges(of(EdgeKind::Taxiway), dim, 1.0, &[]);
        self.stroke_edges(of(EdgeKind::StandLink), dim, 1.0, &[2.0, 3.0]);

        // Stands: small squares. Holding points: short bars across the line.
        ctx.set_fill_style_str(dim);
        for st in &ap.stands {
            let p = self.node_screen(st.node);
            ctx.fill_rect((p[0] - 2.0) * d, (p[1] - 2.0) * d, 4.0 * d, 4.0 * d);
        }
        ctx.set_fill_style_str(fg);
        for h in &ap.holds {
            let p = self.node_screen(h.node);
            ctx.fill_rect((p[0] - 3.0) * d, (p[1] - 1.0) * d, 6.0 * d, 2.0 * d);
        }

        // Taxi routes still to roll: dashed, so they read as planned. The
        // runway ahead of a lined-up or departing aircraft is not drawn: it
        // is the runway itself.
        let taxiing = |a: &&twr_sim::Aircraft| {
            a.on_edge() && matches!(a.phase, Phase::Pushback | Phase::Taxi | Phase::HoldPosition | Phase::HoldShort(_) | Phase::LineUp(_))
        };
        for a in self.world.aircraft.iter().filter(taxiing) {
            self.stroke_edges(a.route.edges[a.leg..].iter().copied(), fg, 1.5, &[4.0, 4.0]);
            if let Some((lim, _)) = &a.limit {
                let p = twr_sim::aircraft::point_at(ap, &self.world.graph, &a.route, *lim);
                self.hold_bar(p, &a.route, *lim, fg);
            }
        }

        if let Mode::Taxi { dest, via, avoid, preview, .. } = &self.mode {
            // Avoided taxiways: dotted, so the choice stays visible without
            // reading as a route. Red stays reserved for conflicts.
            self.stroke_edges(avoid.iter().copied(), dim, 4.0, &[0.5, 6.0]);
            if let Some(h) = self.hover_edge {
                self.stroke_edges(ap.taxiway_of(h).into_iter(), fg, 1.0, &[]);
            }
            if let Some(r) = preview {
                self.stroke_edges(r.edges.iter().copied(), fg, 3.0, &[]);
                // Where the route will stop short of each runway it meets.
                let lims = twr_sim::aircraft::hold_limits(ap, &self.world.graph, r, 0, 0);
                for (lim, _) in lims {
                    let p = twr_sim::aircraft::point_at(ap, &self.world.graph, r, lim);
                    self.hold_bar(p, r, lim, fg);
                }
            }
            self.stroke_edges(via.iter().copied(), fg, 6.0, &[]);
            if let Some(n) = dest {
                let p = self.node_screen(*n);
                ctx.set_stroke_style_str(fg);
                ctx.set_line_width(2.0 * d);
                ctx.begin_path();
                let _ = ctx.arc(p[0] * d, p[1] * d, 8.0 * d, 0.0, std::f64::consts::TAU);
                ctx.stroke();
            }
        }

        // On final: a dim line to the threshold, so the screen shows where
        // it will land.
        ctx.set_stroke_style_str(dim);
        ctx.set_line_width(1.0 * d);
        for a in self.world.aircraft.iter().filter(|a| matches!(a.phase, Phase::Final(_))) {
            let (x, y) = a.pos_m(ap, &self.world.graph);
            let ((sx, sy), t) = (self.to_screen(x, y), self.node_screen(a.route.nodes[0]));
            ctx.begin_path();
            ctx.move_to(sx * d, sy * d);
            ctx.line_to(t[0] * d, t[1] * d);
            ctx.stroke();
        }
        // The selected arrival's exit: a ring. In exit mode, every usable
        // exit gets a smaller one, so the click has visible targets.
        if let Some(i) = self.selected {
            if matches!(self.mode, Mode::Exit) {
                ctx.set_stroke_style_str(fg);
                for n in self.exit_nodes(i) {
                    let p = self.node_screen(n);
                    ctx.begin_path();
                    let _ = ctx.arc(p[0] * d, p[1] * d, 4.0 * d, 0.0, std::f64::consts::TAU);
                    ctx.stroke();
                }
            }
            if let Some(x) = self.world.aircraft[i].exit.as_ref().filter(|_| matches!(self.world.aircraft[i].phase, Phase::Final(_) | Phase::Landing(_))) {
                let p = self.node_screen(x.node);
                ctx.set_stroke_style_str(fg);
                ctx.set_line_width(2.0 * d);
                ctx.begin_path();
                let _ = ctx.arc(p[0] * d, p[1] * d, 8.0 * d, 0.0, std::f64::consts::TAU);
                ctx.stroke();
            }
        }

        for (i, a) in self.world.aircraft.iter().enumerate().filter(|(_, a)| a.visible()) {
            let (x, y) = a.pos_m(ap, &self.world.graph);
            let (sx, sy) = self.to_screen(x, y);
            let hd = self.heading(a);
            let (c, sn) = (hd.cos(), hd.sin());
            // A triangle pointing along the direction of travel.
            let pt = |fwd: f64, side: f64| ((sx + fwd * c - side * sn) * d, (sy + fwd * sn + side * c) * d);
            ctx.set_fill_style_str(fg);
            ctx.begin_path();
            let (p0, p1, p2) = (pt(AIRCRAFT_PX, 0.0), pt(-AIRCRAFT_PX, AIRCRAFT_PX * 0.7), pt(-AIRCRAFT_PX, -AIRCRAFT_PX * 0.7));
            ctx.move_to(p0.0, p0.1);
            ctx.line_to(p1.0, p1.1);
            ctx.line_to(p2.0, p2.1);
            ctx.close_path();
            // In the air (climbing out, on final, going around): outlined.
            if matches!(a.phase, Phase::Airborne(_) | Phase::Final(_) | Phase::GoAround(_)) {
                ctx.set_stroke_style_str(fg);
                ctx.set_line_width(1.5 * d);
                ctx.stroke();
            } else {
                ctx.fill();
            }
            if self.selected == Some(i) {
                ctx.set_stroke_style_str(fg);
                ctx.set_line_width(1.0 * d);
                ctx.begin_path();
                let _ = ctx.arc(sx * d, sy * d, (AIRCRAFT_PX + 7.0) * d, 0.0, std::f64::consts::TAU);
                ctx.stroke();
            }
            ctx.set_font(&format!("{}px ui-monospace, \"DejaVu Sans Mono\", monospace", 11.0 * d));
            let _ = ctx.fill_text(&a.callsign, (sx + 14.0) * d, (sy - 10.0) * d);
        }

        // Red, and only here: live conflicts, then recent incidents. Nothing
        // pulses; a state shows on the tick it starts and goes when it ends.
        let g = &self.world.graph;
        ctx.set_stroke_style_str(red);
        ctx.set_line_width(2.0 * d);
        for c in &self.world.monitor.conflicts {
            let (pa, pb) = (self.world.aircraft[c.a].pos_m(ap, g), self.world.aircraft[c.b].pos_m(ap, g));
            let (a, b) = (self.to_screen(pa.0, pa.1), self.to_screen(pb.0, pb.1));
            ctx.begin_path();
            ctx.move_to(a.0 * d, a.1 * d);
            ctx.line_to(b.0 * d, b.1 * d);
            ctx.stroke();
            for p in [a, b] {
                ctx.begin_path();
                let _ = ctx.arc(p.0 * d, p.1 * d, (AIRCRAFT_PX + 5.0) * d, 0.0, std::f64::consts::TAU);
                ctx.stroke();
            }
        }
        ctx.set_fill_style_str(red);
        for x in self.recent_incidents() {
            let p = self.to_screen(x.at.0, x.at.1);
            ctx.fill_rect((p.0 - 5.0) * d, (p.1 - 5.0) * d, 10.0 * d, 10.0 * d);
        }
    }

    /// Screen-space heading in radians: along the edge the aircraft is on,
    /// or its last edge when stopped. Nose-in on stand, so tail-first while
    /// pushing back.
    fn heading(&self, a: &twr_sim::Aircraft) -> f64 {
        let ap = &self.world.airport;
        let dir = |p: u32, q: u32| {
            let (p, q) = (self.node_screen(p), self.node_screen(q));
            (q[1] - p[1]).atan2(q[0] - p[0])
        };
        if a.phase == Phase::OnStand {
            let s = a.route.end();
            return match ap.edges.iter().find(|e| e.kind == EdgeKind::StandLink && (e.a == s || e.b == s)) {
                Some(e) => dir(if e.a == s { e.b } else { e.a }, s),
                None => -std::f64::consts::FRAC_PI_2,
            };
        }
        let leg = if a.on_edge() { a.leg } else if a.leg > 0 { a.leg - 1 } else { return -std::f64::consts::FRAC_PI_2 };
        let h = dir(a.route.nodes[leg], a.route.nodes[leg + 1]);
        if a.phase == Phase::Pushback { h + std::f64::consts::PI } else { h }
    }

    /// A short bar across the route at route distance `d`: a hold-short line.
    fn hold_bar(&self, p: (f64, f64), r: &Route, d: u64, colour: &str) {
        let (ctx, dp) = (&self.ctx, self.dpr);
        let ahead = twr_sim::aircraft::point_at(&self.world.airport, &self.world.graph, r, d + 10_000);
        let (s0, s1) = (self.to_screen(p.0, p.1), self.to_screen(ahead.0, ahead.1));
        let a = (s1.1 - s0.1).atan2(s1.0 - s0.0) + std::f64::consts::FRAC_PI_2;
        let (c, s) = (a.cos() * 7.0, a.sin() * 7.0);
        ctx.set_stroke_style_str(colour);
        ctx.set_line_width(3.0 * dp);
        ctx.begin_path();
        ctx.move_to((s0.0 - c) * dp, (s0.1 - s) * dp);
        ctx.line_to((s0.0 + c) * dp, (s0.1 + s) * dp);
        ctx.stroke();
    }
}

fn mmss(ticks: u64) -> String {
    let s = ticks.div_ceil(u64::from(TICK_HZ));
    format!("{}:{:02}", s / 60, s % 60)
}

fn hhmmss(ticks: u64) -> String {
    let s = ticks / u64::from(TICK_HZ);
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}
