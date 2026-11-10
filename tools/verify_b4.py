"""B4 verification: the conflict model as the page shows it.

Each scenario is loaded through the one write hook (`load_scenario`), then
driven with real clicks and key presses. Read-only hooks report state;
`fast_forward` runs the sim's own tick code. Red is checked two ways: the
strip's DOM class and text colour, and canvas pixels around the aircraft.
"""
from __future__ import annotations
import pathlib, sys
from playwright.sync_api import sync_playwright

# The built page next to this folder, so the scripts run on any machine.
PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
results: list[tuple[bool, str]] = []

def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, expr):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {expr}; }})()")

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def idx(pg, cs):
    return app(pg, f"a.index_of({cs!r})")

def red_pixels_near(pg, i, radius=16):
    """Count canvas pixels close to --red within `radius` CSS px of aircraft i."""
    return pg.evaluate(f"""(() => {{
      const a = TWR.app, c = document.getElementById('map'), d = window.devicePixelRatio || 1;
      const [x, y] = a.aircraft_screen({i});
      const r = {radius} * d, img = c.getContext('2d').getImageData(Math.round(x * d - r), Math.round(y * d - r), 2 * r, 2 * r).data;
      let n = 0;
      for (let k = 0; k < img.length; k += 4) if (Math.abs(img[k] - 215) < 30 && img[k + 1] < 70 && img[k + 2] < 80) n++;
      return n;
    }})()""")

with sync_playwright() as pw:
    b = pw.chromium.launch()
    pg = b.new_page(viewport={"width": 1500, "height": 950}, color_scheme="dark")  # R1.15: first paint follows the system; these checks start dark
    errs, dialogs = [], []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (dialogs.append(d.message), d.dismiss()))
    pg.goto(PAGE); pg.wait_for_timeout(600)
    box = pg.locator("#map").bounding_box()
    ox, oy = box["x"], box["y"]
    frame(pg)
    check(app(pg, "a.incident_count()") == 0 and pg.text_content("#inc-count") == "0", "fresh page: no incidents")
    check(red_pixels_near(pg, 0, 40) == 0, "fresh page: no red near the aircraft")

    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)

    def select(cs):
        i = idx(pg, cs)
        x, y = app(pg, f"a.aircraft_screen({i})")
        pg.mouse.click(ox + round(x), oy + round(y)); frame(pg)
        return i

    def text(k, i):
        # hint, notice and via moved to the panel under the map (step 2): they
        # read as empty unless strip i is the selected one, as on the strip.
        if k in ("hint", "notice", "via"):
            return pg.evaluate("([k, i]) => document.getElementById('sel').dataset.i === String(i) ? document.getElementById('sel-' + k).textContent : ''", [k, i])
        return pg.text_content(f"#{k}{i}")

    def ff_until(cond, max_ticks=200_000, chunk=20):
        t = 0
        while not app(pg, cond) and t < max_ticks:
            app(pg, f"a.fast_forward({chunk})"); t += chunk
        frame(pg)
        return app(pg, cond)

    # ---- ground: taxi toward a stopped aircraft -----------------------------
    check(app(pg, "a.load_scenario('ground')"), "scenario ground loads")
    frame(pg)
    lead, tail = idx(pg, "IGO601"), idx(pg, "IGO602")
    x, y = app(pg, f"a.aircraft_screen({tail})")
    zoom_at(x, y, 7)
    select("IGO602")
    check(pg.get_attribute(f"#strip{tail}", "class").find("sel") >= 0, "IGO602 selected by click")
    check(ff_until("a.conflict_count() > 0"), "conflict raised as IGO602 closes on IGO601")
    alert = text("alert", tail)
    check(alert.startswith("CONFLICT IGO601"), f"strip red line: {alert!r}")
    check("alarm" in pg.get_attribute(f"#strip{tail}", "class") and "alarm" in pg.get_attribute(f"#strip{lead}", "class"), "both strips carry the red bar")
    check(red_pixels_near(pg, tail) > 20, f"red ring on the map ({red_pixels_near(pg, tail)} px)")
    check(app(pg, "a.incident_count()") == 0, "a conflict is not an incident")
    pg.screenshot(path=str(OUT / "b4_ground_conflict_dark.png"))
    # PLAN-ui-console step 2: the pair also sits in the panel under the map,
    # one row per pair, red-ruled; the row selects the first of the pair.
    rows = pg.eval_on_selector_all("#conflict-list button", "bs => bs.map(b => [b.textContent, b.dataset.i])")
    check(len(rows) == 1 and "IGO601" in rows[0][0] and "IGO602" in rows[0][0] and rows[0][0].endswith(" m")
          and pg.is_hidden("#conflict-none") and pg.evaluate("getComputedStyle(document.getElementById('conflicts')).boxShadow") != "none",
          f"panel lists the pair once: {rows}")
    pg.click("#conflict-list button"); frame(pg)
    check(pg.get_attribute("#sel", "data-i") == rows[0][1], f"the row selects aircraft {rows[0][1]}: {pg.text_content('#sel-cs')}")
    pg.click(f"#strip{tail}"); frame(pg)
    pg.keyboard.press("h"); app(pg, "a.fast_forward(5)"); frame(pg)
    check(pg.locator("#conflict-list button").count() == 0 and pg.is_visible("#conflict-none"), "H: the panel says no live conflict")
    check(app(pg, "a.conflict_count()") == 0 and text("alert", tail) == "", "H: both stopped, red clears")
    check(red_pixels_near(pg, tail) == 0, "no red on the map once held")
    pg.keyboard.press("h"); frame(pg)
    check(ff_until("a.incident_count() > 0"), "continuing into it makes an incident")
    rows = pg.locator("#incidents li").all_text_contents()
    check(len(rows) == 1 and "GROUND · IGO602" in rows[0] and "min 45" in rows[0], f"incident log row: {rows}")
    check(pg.text_content("#inc-count") == "1", "incident count shows 1")
    check(text("alert", lead) in ("INCIDENT",) or text("alert", lead).startswith("CONFLICT"), f"leader strip tagged: {text('alert', lead)!r}")
    app(pg, "a.fast_forward(2000)"); frame(pg)
    check(app(pg, "a.incident_count()") == 1, "one episode counts once")

    # ---- wake: M lined up behind a departed H -------------------------------
    check(app(pg, "a.load_scenario('wake')"), "scenario wake loads")
    frame(pg)
    i = idx(pg, "IGO512")
    x, y = app(pg, f"a.aircraft_screen({i})")
    zoom_at(x, y, 3)
    select("IGO512")
    note, hint = text("note", i), text("hint", i)
    check(note.startswith("wake ") and note.endswith("AIC101"), f"wake timer on the strip: {note!r}")
    check("K takeoff (wake" in hint, f"K hint names the wake: {hint!r}")
    fg = pg.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--fg').trim()")
    # PLAN-ui-console step 4: selecting springs the strip's inversion over
    # 215 ms; read the colour at rest, not a value on the way.
    pg.wait_for_timeout(300)
    note_colour = pg.evaluate(f"getComputedStyle(document.getElementById('note{i}')).color")
    check(note_colour in ("rgb(255, 255, 255)", "rgb(0, 0, 0)"), f"wake timer in plain ink, not red: {note_colour} (fg {fg})")
    check(text("alert", i) == "" and "alarm" not in pg.get_attribute(f"#strip{i}", "class"), "waiting for wake is not red")
    pg.keyboard.press("k"); frame(pg)
    check(ff_until("a.incident_count() > 0"), "K inside the wake minimum: incident at lift-off")
    rows = pg.locator("#incidents li").all_text_contents()
    check(len(rows) == 1 and "WAKE · IGO512 (M)" in rows[0] and "AIC101 (H), min 2:00" in rows[0], f"wake row: {rows}")
    check(app(pg, "a.incident_rules()") == "Wake", "rule is Wake")

    # ---- runway: K, then X across the roll -----------------------------------
    check(app(pg, "a.load_scenario('runway')"), "scenario runway loads")
    frame(pg)
    d, x_ = idx(pg, "IGO512"), idx(pg, "IGO701")
    select("IGO512")
    check(text("note", d) == "", f"runway clear before X: note {text('note', d)!r}")
    pg.keyboard.press("k"); frame(pg)
    select("IGO701")
    check("X cross" in text("hint", x_), f"crosser offered X: {text('hint', x_)!r}")
    pg.keyboard.press("x"); frame(pg)
    check(ff_until("a.incident_count() > 0", 5_000, 1), "X ahead of a takeoff roll: incident")
    rows = pg.locator("#incidents li").all_text_contents()
    check(len(rows) == 1 and rows[0].split(" ", 1)[1].startswith("INCURSION · IGO701"), f"incursion row: {rows}")
    sq = red_pixels_near(pg, x_, 12)
    check(sq > 50, f"red square on the map, at the crosser ({sq} px)")
    pg.screenshot(path=str(OUT / "b4_runway_incident_dark.png"))
    dark_log = pg.evaluate("getComputedStyle(document.querySelector('#incidents li')).color")
    check(dark_log == "rgb(255, 77, 85)", f"dark incident text is #ff4d55: {dark_log}")

    # ---- light theme ----------------------------------------------------------
    pg.click("#theme"); frame(pg)
    light_log = pg.evaluate("getComputedStyle(document.querySelector('#incidents li')).color")
    check(light_log == "rgb(215, 25, 33)", f"light incident text is #d71921: {light_log}")
    sq = red_pixels_near(pg, x_, 12)
    check(sq > 50, f"red square drawn in light theme too ({sq} px)")
    pg.screenshot(path=str(OUT / "b4_runway_incident_light.png"))
    pg.click("#theme"); frame(pg)

    check(not dialogs, f"no alert/confirm dialogs: {dialogs}")
    check(not errs, f"no console or page errors: {errs}")
    b.close()

passed = sum(ok for ok, _ in results)
print(f"{passed}/{len(results)} passed")
sys.exit(0 if passed == len(results) else 1)
