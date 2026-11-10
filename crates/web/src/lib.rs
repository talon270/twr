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
/// A1: aircraft are true-scale silhouettes, but never narrower than this
/// many CSS px of wingspan. At full zoom-out a 36 m A320 is 2 px across;
/// below the floor the shape keeps its proportions at 14 px, so it still
/// reads as an aircraft and stays visible.
const SILHOUETTE_MIN_PX: f64 = 14.0;
/// B10: a terminal's name is drawn only once the building spans this many
/// CSS px across; at full zoom-out, four "FREIGHT TERMINAL" labels would sit
/// on top of each other.
const TERMINAL_LABEL_MIN_PX: f64 = 120.0;

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
    /// After P or S (C2): choosing which way the tail goes. `sel` indexes
    /// the stand's tail options; `taxi_after` is S.
    Push { sel: usize, taxi_after: bool },
}

/// Distance from (x, y) to a closed polygon: 0 inside it, else to the
/// nearest edge. A silhouette is not convex, so the inside test is even-odd.
fn dist_to_shape(pts: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let (mut inside, mut best) = (false, f64::MAX);
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1) {
            inside = !inside;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = if dx == 0.0 && dy == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0) };
        best = best.min((x - a.0 - t * dx).hypot(y - a.1 - t * dy));
    }
    if inside { 0.0 } else { best }
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
    /// S (push, then taxi): the aircraft pushing back, whose taxi route
    /// builder opens the moment the push ends. Dropped if you select another
    /// aircraft, press Esc, or the aircraft is not the one selected then.
    after_push: Option<usize>,
    /// B10: Area::label_point per area, in metres; the airport never changes
    /// after load, so the grid search runs once.
    area_labels: Vec<(f64, f64)>,
    /// R1.22: bumped by every tick and every input that can change what a
    /// strip shows. The page rebuilds strips only when it moves, so a paused
    /// or ended shift costs no strip work at all.
    rev: u32,
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
        let world = World::new(u64::from(seed));
        let area_labels = world.airport.areas.iter().map(twr_sim::airport::Area::label_point).collect();
        Ok(App {
            world,
            area_labels,
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
            after_push: None,
            rev: 0,
        })
    }

    /// One animation frame: advance the sim by `real_ms` unless `paused`,
    /// run the camera spring, draw. The spring always gets the real frame
    /// time: pausing stops the traffic, not the camera (R1.5). Colours come
    /// from the page's CSS custom properties so both themes are drawn by the
    /// same code.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, real_ms: f64, paused: bool, dpr: f64, reduce: bool, bg: &str, fg: &str, dim: &str, line: &str, red: &str, apron: &str, term: &str) -> u32 {
        self.dpr = dpr;
        self.w = f64::from(self.canvas.width()) / dpr;
        self.h = f64::from(self.canvas.height()) / dpr;
        if !self.cam.fitted {
            self.fit();
        }
        let steps = self.world.frame(if paused { 0.0 } else { real_ms });
        if steps > 0 {
            self.rev = self.rev.wrapping_add(1);
        }
        self.open_taxi_after_push();
        self.spring(real_ms, reduce);
        self.draw(bg, fg, dim, line, red, apron, term);
        steps
    }

    /// Changes whenever strip content may have changed; see `rev`.
    pub fn revision(&self) -> u32 {
        self.rev
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
        self.rev = self.rev.wrapping_add(1);
        self.world = World::shift(u64::from(seed), m);
        self.cam.fitted = false;
        self.selected = None;
        self.after_push = None;
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

    /// PLAN-lvp.md: low-visibility procedures for this shift.
    pub fn lvp(&self) -> bool {
        self.world.lvp()
    }

    /// The Practice toggle. Scored modes take fog from the seed only, so
    /// two players on one seed play the same weather; there it refuses.
    pub fn set_lvp(&mut self, on: bool) -> bool {
        if self.world.mode() != twr_sim::Mode::Practice {
            return false;
        }
        self.world.set_lvp(on);
        self.rev = self.rev.wrapping_add(1);
        true
    }

    /// The runway plan in force: the player's, else the wind's own pick.
    fn plan(&self) -> twr_sim::weather::Plan {
        self.world.traffic.as_ref().map_or_else(|| twr_sim::weather::Plan::default_for(self.world.wind().flow()), |t| t.schedule.plan())
    }

    /// B9, C4: "west" or "east", the direction in use.
    pub fn flow(&self) -> String {
        self.plan().flow.name().into()
    }

    /// B9, C4: the runways in use, for the tests and the panel's summary.
    pub fn runways_text(&self) -> String {
        let p = self.plan();
        format!("land {} · depart {}", p.land.join(" "), p.depart.join(" "))
    }

    /// C4: the picker can only plan a shift's traffic; Practice has none.
    pub fn can_plan(&self) -> bool {
        self.world.traffic.is_some()
    }

    /// C4: the direction the picker shows: the waiting one if there is one.
    pub fn runway_target(&self) -> String {
        self.world.target_plan().map_or_else(|| self.plan().flow.name().into(), |p| p.flow.name().into())
    }

    /// C4: one row per runway end of the shown direction:
    /// "29L<TAB>land 0/1<TAB>depart 0/1<TAB>tailwind kt<TAB>length m".
    pub fn runway_rows(&self) -> Vec<String> {
        let p = self.world.target_plan().unwrap_or_else(|| self.plan());
        let tw = self.world.wind().tailwind_kt(p.flow);
        twr_sim::weather::ENDS
            .iter()
            .filter(|e| e.1 == p.flow)
            .map(|e| format!("{}\t{}\t{}\t{:.0}\t{}", e.0, u8::from(p.land.contains(&e.0)), u8::from(p.depart.contains(&e.0)), tw, e.2))
            .collect()
    }

    /// C4: tailwind, a waiting switch, held movements, heavies without a runway.
    pub fn runway_status(&self) -> Vec<String> {
        self.world.runway_status()
    }

    /// C4: turn an end on or off for landings or departures. Returns the
    /// refusal, "" when done.
    pub fn runway_set(&mut self, end: &str, land: bool, on: bool) -> String {
        self.rev = self.rev.wrapping_add(1);
        self.world.runway_set(end, land, on).err().unwrap_or_default()
    }

    /// C4: "west" or "east". Returns the refusal, "" when done.
    pub fn runway_direction(&mut self, flow: &str) -> String {
        self.rev = self.rev.wrapping_add(1);
        let f = if flow == "east" { twr_sim::weather::Flow::East } else { twr_sim::weather::Flow::West };
        self.world.runway_direction(f).err().unwrap_or_default()
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
        let e = self.edge_at(x, y);
        if e != self.hover_edge {
            self.rev = self.rev.wrapping_add(1);
        }
        self.hover_edge = e;
    }

    /// A click that was not a drag. `button`: 0 left, 2 right.
    pub fn click(&mut self, x: f64, y: f64, button: i16) {
        self.rev = self.rev.wrapping_add(1);
        let dest_set = match &self.mode {
            Mode::Idle => {
                if button == 0 {
                    self.selected = self.aircraft_at(x, y);
                }
                return;
            }
            Mode::Taxi { dest, .. } => dest.is_some(),
            Mode::Push { taxi_after, .. } => {
                // A click on an arrow sends that push.
                let taxi_after = *taxi_after;
                if button == 0 {
                    if let Some(k) = self.tail_at(x, y) {
                        self.send_push(k, taxi_after);
                    }
                }
                return;
            }
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
        self.rev = self.rev.wrapping_add(1);
        if matches!(self.mode, Mode::Exit) {
            if key != "Escape" {
                return false;
            }
            self.mode = Mode::Idle;
            return true;
        }
        if let Mode::Push { sel, taxi_after } = self.mode {
            let n = self.selected.map_or(0, |i| self.world.tail_options(i).len());
            match key {
                "Escape" => self.mode = Mode::Idle,
                "ArrowLeft" => self.mode = Mode::Push { sel: (sel + n - 1) % n.max(1), taxi_after },
                "ArrowRight" => self.mode = Mode::Push { sel: (sel + 1) % n.max(1), taxi_after },
                "Enter" => self.send_push(sel, taxi_after),
                d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() && d != "0" => self.send_push(usize::from(d.as_bytes()[0] - b'1'), taxi_after),
                _ => return false,
            }
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
            // C2: P and S ask which way the tail goes; Enter takes the first.
            "P" if self.world.verbs(i).contains(&'P') => self.start_push(i, false),
            "P" => self.world.push(i),
            // S: push, then the taxi route builder opens when the push ends:
            // taxi is refused while pushing, so the wait is the page's.
            "S" if self.world.verbs(i).contains(&'P') => self.start_push(i, true),
            "S" => Err("only an aircraft on stand can push back".to_string()),
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

    /// C1: the strip bay aircraft `i` sits in, by what it needs from you:
    /// 0 Pushback, 1 Ground, 2 Runway, 3 Landings. An arrival parked on its
    /// stand is Ground until its shut-down removes the strip.
    pub fn bay_of(&self, i: usize) -> u8 {
        let a = &self.world.aircraft[i];
        match a.phase {
            Phase::OnStand if a.arrival => 1,
            Phase::OnStand | Phase::Pushback => 0,
            Phase::LineUp(_) | Phase::LinedUp(_) | Phase::TakeoffRoll(_) | Phase::Airborne(_) => 2,
            Phase::Final(_) | Phase::Landing(_) | Phase::GoAround(_) => 3,
            _ => 1,
        }
    }

    /// C1: sort key inside the bay, lowest first. Landings: metres to the
    /// threshold (a landing roll first, a go-around still circling last).
    /// Elsewhere: strips asking for something first, then order of
    /// appearance. The game keeps no per-aircraft wait time on the ground,
    /// so this stands in for "time waited".
    pub fn bay_key(&self, i: usize) -> f64 {
        let a = &self.world.aircraft[i];
        match a.phase {
            Phase::Final(_) => a.final_m(),
            Phase::Landing(_) => -1.0,
            Phase::GoAround(_) => 1e6 + a.circuit_left() as f64,
            _ if self.world.requesting(i) => -1.0,
            _ => i as f64,
        }
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
            Phase::Final(_) => {
                return self.world.runway_blocker(i).map(|by| format!("rwy busy · {by}"))
                    .or_else(|| self.world.lsa_blocker(i).map(|by| format!("LSA: {by}")))
                    .unwrap_or_default()
            }
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
        self.rev = self.rev.wrapping_add(1);
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
        self.rev = self.rev.wrapping_add(1);
        match World::scenario(self.world.seed, name) {
            Some(w) => {
                self.world = w;
                // A new airport picture: frame it again, or the aircraft
                // can sit off-canvas under the last scenario's zoom.
                self.cam.fitted = false;
                self.selected = None;
                self.after_push = None;
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
                    'S' => "S push + taxi".into(),
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
                let mut parts: Vec<String> = self.page_verbs(i).into_iter().map(words).collect();
                parts.push("Esc deselect".into());
                parts.join(" · ")
            }
            Mode::Taxi { dest: None, .. } => "Click a stand, holding point or runway entry · Esc cancel".into(),
            Mode::Taxi { preview: None, note, .. } => format!("{note} · click it again to undo · Esc cancel"),
            Mode::Taxi { .. } => "Enter send · click taxiway: via · right-click: avoid, again to undo · Esc cancel".into(),
            Mode::Exit => "Click an exit along the runway · Esc cancel".into(),
            Mode::Push { sel, .. } => {
                let o = self.world.tail_options(i);
                match o.get(*sel) {
                    Some((o, _)) => format!("← → choose · Enter push {} · click an arrow · Esc cancel", o.label),
                    None => "Esc cancel".into(),
                }
            }
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
            Mode::Idle | Mode::Exit | Mode::Push { .. } => String::new(),
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

    /// One line per button on aircraft `i`'s strip: "K<TAB>Takeoff", the key
    /// the button presses and its label. Only while no taxi or exit pick is
    /// open, when the hint line already says what the clicks mean.
    pub fn verb_buttons(&self, i: usize) -> Vec<String> {
        // C2: while choosing a tail, one button per direction, pressing its digit.
        if matches!(self.mode, Mode::Push { .. }) {
            return self
                .world
                .tail_options(i)
                .iter()
                .enumerate()
                .map(|(k, (o, by))| {
                    let lane = if o.taxiway.is_empty() { String::new() } else { format!(" · {}", o.taxiway) };
                    let busy = if by.is_some() { " · blocked" } else { "" };
                    format!("{}\t{}{lane}{busy}", k + 1, o.label)
                })
                .collect();
        }
        if !matches!(self.mode, Mode::Idle) {
            return Vec::new();
        }
        let label = |c: char| match c {
            'P' => "Push",
            'S' => "Push + taxi",
            'T' => "Taxi",
            'H' => "Hold",
            'X' => "Cross",
            'L' => "Line up",
            'K' => "Takeoff",
            'C' => "Clear to land",
            'G' => "Go around",
            'E' => "Exit",
            _ => "",
        };
        self.page_verbs(i).into_iter().filter(|&c| !label(c).is_empty()).map(|c| format!("{c}\t{}", label(c))).collect()
    }

    /// What aircraft `i` is asking for, or "" if nothing; the strip flashes
    /// while it is not empty. The sim decides (`World::requesting`); this
    /// only words it.
    pub fn request_text(&self, i: usize) -> String {
        if !self.world.requesting(i) {
            return String::new();
        }
        match self.world.aircraft[i].phase {
            Phase::OnStand => "ready to push",
            Phase::Holding => "requests taxi",
            Phase::LinedUp(_) => "ready for takeoff",
            Phase::Final(_) => "requests landing clearance",
            _ => "",
        }
        .into()
    }

    /// Verb keys aircraft `i` accepts now, e.g. "TX".
    pub fn verbs(&self, i: usize) -> String {
        self.world.verbs(i).into_iter().collect()
    }

    /// PLAN-ui-console A2: the words the radio line may offer next after
    /// `line`. The page keeps no verb list of its own; see `twr_sim::radio`.
    pub fn complete(&self, line: &str) -> Vec<String> {
        twr_sim::radio::next_tokens(&self.world, line)
    }

    /// Send a typed radio line. Selects the aircraft and presses the verb
    /// key, so a typed line and a key press can never disagree. Returns ""
    /// when the sim took it, else the reason it did not.
    pub fn send_line(&mut self, line: &str) -> String {
        match twr_sim::radio::parse(&self.world, line) {
            Err(e) => {
                self.rev = self.rev.wrapping_add(1);
                e
            }
            Ok((i, k)) => {
                self.select(i);
                self.key(&k.to_string());
                self.notice.clone()
            }
        }
    }

    // ---- C3 ground control: read-only. Keys are wired by the shell slice ----

    /// C3 verbs aircraft `i` accepts now, one line each, "key<TAB>label",
    /// as `verb_buttons` words its own: "Enter\tContinue", "J\tHold at
    /// point", "Y\tGive way", "X\tCross if clear", "B\tBacktrack",
    /// "-\tSlower", "=\tFaster". Y is offered whenever `i` could give way
    /// to someone; whether to whom you click is accepted is the sim's call.
    pub fn ground_verbs(&self, i: usize) -> Vec<String> {
        self.world.ground_verbs(i).into_iter().map(|(k, l)| format!("{k}\t{l}")).collect()
    }

    /// Nodes aircraft `i` can be told to hold at (J): ahead on its route,
    /// before the end, off the runways. Empty when it is not taxiing.
    pub fn hold_points(&self, i: usize) -> Vec<u32> {
        self.world.aircraft[i].hold_points(&self.world.graph)
    }

    /// The node aircraft `i` will stop at or for (a hold point, or the
    /// junction it gives way at), or -1.
    pub fn hold_point(&self, i: usize) -> i64 {
        self.world.aircraft[i].stop.as_ref().map_or(-1, |s| i64::from(s.node))
    }

    /// Who aircraft `i` is giving way to, or "".
    pub fn give_way_to(&self, i: usize) -> String {
        self.world.aircraft[i].stop.as_ref().and_then(|s| s.give_way.clone()).unwrap_or_default()
    }

    /// "slow", "normal" or "expedite".
    pub fn taxi_speed(&self, i: usize) -> String {
        self.world.aircraft[i].taxi_speed.name().into()
    }

    /// The runway aircraft `i` will cross without stopping if it is clear
    /// on arrival (X given ahead of the hold), or "".
    pub fn cross_ahead(&self, i: usize) -> String {
        self.world.aircraft[i].cross_ahead.as_ref().map(|(_, r)| r.clone()).unwrap_or_default()
    }

    /// The designator aircraft `i` will face after its backtrack, or "".
    pub fn backtrack_to(&self, i: usize) -> String {
        self.world.aircraft[i].backtrack.clone().unwrap_or_default()
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

    /// The silhouette aircraft `i` is drawn as, CSS px: [x0, y0, x1, y1, …].
    /// The same points the frame draws, for tests that measure it.
    pub fn aircraft_outline(&self, i: usize) -> Vec<f64> {
        self.silhouette(i).0.into_iter().flat_map(|(x, y)| [x, y]).collect()
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
        self.rev = self.rev.wrapping_add(1);
        self.world.step_ticks(n);
        self.open_taxi_after_push();
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
                // A1: the reach is HIT_PX beyond the drawn shape, not beyond
                // one point: a zoomed-in 777 is hundreds of px long and its
                // tail must select it. Ties (a click inside two shapes) go
                // to the nearer position point, as before A1.
                let (pts, _, _) = self.silhouette(i);
                let p = self.aircraft_screen(i);
                (i, dist_to_shape(&pts, x, y), (p[0] - x).hypot(p[1] - y))
            })
            .filter(|(_, d, _)| *d <= HIT_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
            .map(|(i, _, _)| i)
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

    /// P or S on a stand: open the tail picker, or push at once when the
    /// stand's junction offers no taxilane to push along.
    fn start_push(&mut self, i: usize, taxi_after: bool) -> Result<(), String> {
        if self.world.tail_options(i).is_empty() {
            let r = self.world.push(i);
            self.after_push = (r.is_ok() && taxi_after).then_some(i);
            return r;
        }
        self.mode = Mode::Push { sel: 0, taxi_after };
        Ok(())
    }

    /// Send the selected aircraft's push along option `k`. A refusal (no such
    /// option, or another aircraft in the way) says why and leaves the picker open.
    fn send_push(&mut self, k: usize, taxi_after: bool) {
        let Some(i) = self.selected else { return };
        match self.world.push_tail(i, k) {
            Ok(()) => {
                self.mode = Mode::Idle;
                self.notice.clear();
                self.after_push = taxi_after.then_some(i);
            }
            Err(m) => self.notice = m,
        }
    }

    /// The tail option whose arrow is under (x, y), if any.
    fn tail_at(&self, x: f64, y: f64) -> Option<usize> {
        let i = self.selected?;
        self.world
            .tail_options(i)
            .iter()
            .enumerate()
            .map(|(k, (o, _))| {
                let pts: Vec<Vec<f64>> = o.route.nodes[1..].iter().map(|&n| self.node_screen(n)).collect();
                let d = pts.windows(2).map(|w| seg_dist((x, y), (w[0][0], w[0][1]), (w[1][0], w[1][1]))).fold(f64::MAX, f64::min);
                (k, d)
            })
            .filter(|(_, d)| *d <= HIT_PX / 2.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
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

    #[allow(clippy::too_many_arguments)]
    fn draw(&self, bg: &str, fg: &str, dim: &str, line: &str, red: &str, apron: &str, term: &str) {
        let (ctx, d) = (&self.ctx, self.dpr);
        ctx.set_fill_style_str(bg);
        ctx.fill_rect(0.0, 0.0, self.w * d, self.h * d);
        ctx.set_line_cap("round");
        let ap = &self.world.airport;
        let s = self.cam.scale();
        let of = |k: EdgeKind| (0..ap.edges.len() as u32).filter(move |&i| ap.edges[i as usize].kind == k);

        // B10: aprons, then terminals over them, under everything that moves.
        // One path per tone, so 55 polygons cost two fills.
        for (kind, tone) in [('a', apron), ('t', term)] {
            ctx.begin_path();
            for r in ap.areas.iter().filter(|r| r.kind == kind) {
                for (k, &(x, y)) in r.ring.iter().enumerate() {
                    let (px, py) = self.to_screen(f64::from(x) / 10.0, f64::from(y) / 10.0);
                    if k == 0 { ctx.move_to(px * d, py * d) } else { ctx.line_to(px * d, py * d) }
                }
                ctx.close_path();
            }
            ctx.set_fill_style_str(tone);
            ctx.fill();
        }
        // LVP (PLAN-lvp.md step 8): each landing end's LSA, dashed in --dim,
        // drawn only under LVP. The player cannot keep clear an area they
        // cannot see. --dim is 7.0:1 on the light map and 6.2:1 on the dark.
        if self.world.lvp() {
            let land = self.plan().land;
            for dir in self.world.monitor.dirs.iter().filter(|d| land.contains(&d.desig.as_str())) {
                let Some(st) = self.world.monitor.lsa.0.get(&dir.desig) else { continue };
                let left = (-dir.u.1, dir.u.0);
                let at = |a: f64, l: f64| self.to_screen(dir.from.0 + dir.u.0 * a + left.0 * l, dir.from.1 + dir.u.1 * a + left.1 * l);
                ctx.begin_path();
                let edge = st.iter().map(|s| at(s.along, s.left)).chain(st.iter().rev().map(|s| at(s.along, -s.right)));
                for (k, (x, y)) in edge.enumerate() {
                    if k == 0 { ctx.move_to(x * d, y * d) } else { ctx.line_to(x * d, y * d) }
                }
                ctx.close_path();
                let pattern = js_sys::Array::of2(&JsValue::from_f64(6.0 * d), &JsValue::from_f64(4.0 * d));
                let _ = ctx.set_line_dash(&pattern);
                ctx.set_stroke_style_str(dim);
                ctx.set_line_width(1.5 * d);
                ctx.stroke();
                let _ = ctx.set_line_dash(&js_sys::Array::new());
            }
        }
        // Terminal names inside the outline (Area::label_point), once it is wide
        // enough on screen to hold one; unnamed outlines stay unlabelled.
        ctx.set_fill_style_str(dim);
        ctx.set_text_align("center");
        ctx.set_font(&format!("{}px ui-monospace, \"DejaVu Sans Mono\", monospace", 11.0 * d));
        for (r, &(mx, my)) in ap.areas.iter().zip(&self.area_labels).filter(|(r, _)| r.kind == 't' && !r.name.is_empty()) {
            let (lo, hi) = r.ring.iter().fold((i32::MAX, i32::MIN), |(lo, hi), p| (lo.min(p.0), hi.max(p.0)));
            if f64::from(hi - lo) / 10.0 * s < TERMINAL_LABEL_MIN_PX {
                continue;
            }
            let (px, py) = self.to_screen(mx, my);
            let _ = ctx.fill_text(&r.name.to_uppercase(), px * d, py * d);
        }
        ctx.set_text_align("start");

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

        // C2: one arrow per tail direction, from the junction to where the
        // tail would end; the chosen one solid and heavy, a blocked one dotted.
        if let (Mode::Push { sel, .. }, Some(i)) = (&self.mode, self.selected) {
            for (k, (o, by)) in self.world.tail_options(i).iter().enumerate() {
                let pts: Vec<Vec<f64>> = o.route.nodes[1..].iter().map(|&n| self.node_screen(n)).collect();
                let (colour, width) = if k == *sel { (fg, 4.0) } else { (dim, 3.0) };
                let pattern = js_sys::Array::new();
                if by.is_some() {
                    pattern.push(&JsValue::from_f64(2.0 * d));
                    pattern.push(&JsValue::from_f64(5.0 * d));
                }
                let _ = ctx.set_line_dash(&pattern);
                ctx.set_stroke_style_str(colour);
                ctx.set_fill_style_str(colour);
                ctx.set_line_width(width * d);
                ctx.begin_path();
                for (n, p) in pts.iter().enumerate() {
                    if n == 0 { ctx.move_to(p[0] * d, p[1] * d) } else { ctx.line_to(p[0] * d, p[1] * d) }
                }
                ctx.stroke();
                let _ = ctx.set_line_dash(&js_sys::Array::new());
                // Head and number at the tail end.
                let (a, b) = (&pts[pts.len() - 2], &pts[pts.len() - 1]);
                let h = (b[1] - a[1]).atan2(b[0] - a[0]);
                ctx.begin_path();
                for (da, r) in [(0.0, 9.0), (2.5, 6.0), (-2.5, 6.0)] {
                    let (x, y) = (b[0] + r * (h + da).cos(), b[1] + r * (h + da).sin());
                    if da == 0.0 { ctx.move_to(x * d, y * d) } else { ctx.line_to(x * d, y * d) }
                }
                ctx.close_path();
                ctx.fill();
                ctx.set_font(&format!("{}px ui-monospace, \"DejaVu Sans Mono\", monospace", 11.0 * d));
                let _ = ctx.fill_text(&format!("{} {}", k + 1, o.label.trim_start_matches("tail ")), (b[0] + 12.0 * h.cos() + 4.0) * d, (b[1] + 12.0 * h.sin() + 4.0) * d);
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
            let (pts, (mx, my), r) = self.silhouette(i);
            ctx.set_fill_style_str(fg);
            ctx.begin_path();
            for (k, p) in pts.iter().enumerate() {
                if k == 0 { ctx.move_to(p.0 * d, p.1 * d) } else { ctx.line_to(p.0 * d, p.1 * d) }
            }
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
                let _ = ctx.arc(mx * d, my * d, (r + 7.0) * d, 0.0, std::f64::consts::TAU);
                ctx.stroke();
            }
            ctx.set_font(&format!("{}px ui-monospace, \"DejaVu Sans Mono\", monospace", 11.0 * d));
            let _ = ctx.fill_text(&a.callsign, (mx + r + 4.0) * d, (my - 4.0) * d);
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
            for k in [c.a, c.b] {
                let (_, m, r) = self.silhouette(k);
                ctx.begin_path();
                let _ = ctx.arc(m.0 * d, m.1 * d, (r + 5.0) * d, 0.0, std::f64::consts::TAU);
                ctx.stroke();
            }
        }
        ctx.set_fill_style_str(red);
        for x in self.recent_incidents() {
            let p = self.to_screen(x.at.0, x.at.1);
            ctx.fill_rect((p.0 - 5.0) * d, (p.1 - 5.0) * d, 10.0 * d, 10.0 * d);
        }
    }

    /// S: when the pushing aircraft stops, open its taxi route builder, as T
    /// would. Dropped when it is no longer the selected aircraft or the page
    /// is busy with another mode: the builder must never open on a surprise.
    fn open_taxi_after_push(&mut self) {
        let Some(i) = self.after_push else { return };
        let a = &self.world.aircraft[i];
        if a.phase == Phase::Pushback && self.selected == Some(i) {
            return;
        }
        self.after_push = None;
        if a.phase == Phase::Holding && self.selected == Some(i) && matches!(self.mode, Mode::Idle) {
            self.mode = Mode::Taxi { dest: None, via: Vec::new(), avoid: Vec::new(), preview: None, note: String::new() };
            self.notice.clear();
        }
    }

    /// The verbs aircraft `i` accepts, plus S where P is accepted.
    fn page_verbs(&self, i: usize) -> Vec<char> {
        let mut v = self.world.verbs(i);
        if let Some(p) = v.iter().position(|&c| c == 'P') {
            v.insert(p + 1, 'S');
        }
        v
    }

    /// Aircraft `i`'s silhouette in CSS px, turned to its heading with the
    /// nose gear on its position; then the shape's middle and the radius
    /// that reaches every point from it (rings, hit test, label).
    fn silhouette(&self, i: usize) -> (Vec<(f64, f64)>, (f64, f64), f64) {
        let a = &self.world.aircraft[i];
        let (x, y) = a.pos_m(&self.world.airport, &self.world.graph);
        let (sx, sy) = self.to_screen(x, y);
        let (_, span, _) = twr_sim::shape::dims(a.type_code);
        let k = self.cam.scale().max(SILHOUETTE_MIN_PX / span);
        let (c, s) = { let h = self.heading(a); (h.cos(), h.sin()) };
        // forward along the heading, right 90 degrees clockwise on a y-down canvas
        let pts: Vec<_> = twr_sim::shape::outline(a.type_code)
            .into_iter()
            .map(|(f, r)| (sx + (f * c - r * s) * k, sy + (f * s + r * c) * k))
            .collect();
        let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
        for p in &pts {
            lo = (lo.0.min(p.0), lo.1.min(p.1));
            hi = (hi.0.max(p.0), hi.1.max(p.1));
        }
        let mid = ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0);
        let r = pts.iter().fold(0.0_f64, |m, p| m.max((p.0 - mid.0).hypot(p.1 - mid.1)));
        (pts, mid, r)
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
