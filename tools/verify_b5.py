"""B5 verification: one arrival from final to on-stand, driven by real input.

Scenarios load through the one write hook (`load_scenario`); every command
after that is a real click (strip, map) or key press. Read-only hooks report
state; `fast_forward` runs the sim's own tick code. A pass means the page did
what the plan says with the keys a player would press, in both themes.
"""
from __future__ import annotations
import pathlib, re, sys
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

def red_pixels_near(pg, x, y, radius=12):
    return pg.evaluate(f"""(() => {{
      const c = document.getElementById('map'), d = window.devicePixelRatio || 1;
      const r = {radius} * d, img = c.getContext('2d').getImageData(Math.round({x} * d - r), Math.round({y} * d - r), 2 * r, 2 * r).data;
      let n = 0;
      for (let k = 0; k < img.length; k += 4) if (img[k] > 180 && img[k + 1] < 90 && img[k + 2] < 100) n++;
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
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]
    frame(pg)

    # The page as it opens: a departure on stand and IGO713 on final.
    i = app(pg, "a.index_of('IGO713')")
    check(i >= 0 and app(pg, f"a.phase({i})") == 'Final("29L")', f"fresh page: IGO713 on final 29L ({app(pg, f'a.phase({i})') if i >= 0 else '-'})")

    def idx(cs):
        return app(pg, f"a.index_of({cs!r})")

    def text(k, i):
        # hint, notice and via moved to the panel under the map (step 2): they
        # read as empty unless strip i is the selected one, as on the strip.
        if k in ("hint", "notice", "via"):
            return pg.evaluate("([k, i]) => document.getElementById('sel').dataset.i === String(i) ? document.getElementById('sel-' + k).textContent : ''", [k, i])
        return pg.text_content(f"#{k}{i}")

    def ff_until(cond, max_ticks=400_000, chunk=20):
        t = 0
        while not app(pg, cond) and t < max_ticks:
            app(pg, f"a.fast_forward({chunk})"); t += chunk
        frame(pg)
        return app(pg, cond)

    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)

    def click_node(n, notches=10):
        x, y = app(pg, f"a.node_screen({n})")
        zoom_at(x, y, notches)
        x, y = app(pg, f"a.node_screen({n})")
        pg.mouse.click(ox + round(x), oy + round(y)); frame(pg)
        zoom_at(x, y, -notches)

    def load(name):
        ok = app(pg, f"a.load_scenario({name!r})")
        frame(pg)
        return ok

    # ---- final to on-stand -----------------------------------------------
    check(load("arrival"), "scenario arrival loads")
    i = idx("IGO713")
    pg.click(f"#strip{i}"); frame(pg)
    check("sel" in pg.get_attribute(f"#strip{i}", "class"), "clicking the strip selects IGO713")
    st = text("state", i)
    # The page clock runs between load and read, so the distance may have dropped a little.
    check(re.fullmatch(r"Final 29L · [78]\.\d km · not cleared · exit Z2", st), f"strip on final: {st!r}")
    hint = text("hint", i)
    check("C clear to land" in hint and "G go around" in hint and "E exit" in hint, f"hint offers C, G, E: {hint!r}")
    pg.screenshot(path=str(OUT / "b5_final_dark.png"))

    pg.keyboard.press("c"); frame(pg)
    check("cleared to land" in text("state", i) and text("notice", i) == "", f"C clears to land: {text('state', i)!r}")
    pg.keyboard.press("c"); frame(pg)
    check(text("notice", i) == "already cleared to land", f"second C refused inline: {text('notice', i)!r}")

    # E: refused on a stand, then a later exit accepted.
    exits = app(pg, f"Array.from(a.exit_nodes({i}))")
    first = app(pg, f"a.exit_node({i})")
    later = exits[exits.index(first) + 1]
    pg.keyboard.press("e"); frame(pg)
    check(text("hint", i).startswith("Click an exit"), f"E enters exit mode: {text('hint', i)!r}")
    stand = app(pg, "Array.from(a.stand_nodes())")[0]
    click_node(stand)
    check(text("notice", i) == "not an exit off 29L" and app(pg, f"a.exit_node({i})") == first, f"E on a stand refused inline: {text('notice', i)!r}")
    pg.keyboard.press("e"); frame(pg)
    click_node(later)
    check(app(pg, f"a.exit_node({i})") == later and text("notice", i) == "", f"E then click picks the next exit ({later})")
    check(ff_until(f"a.phase({i}).startsWith('Landing')"), "crosses the threshold")
    app(pg, "a.fast_forward(200)"); frame(pg)
    st = text("state", i)
    check(st.startswith("Landing roll 29L") and "· exit " in st, f"strip on the roll: {st!r}")
    x, y = app(pg, f"a.aircraft_screen({i})")
    zoom_at(x, y, 4)
    pg.screenshot(path=str(OUT / "b5_landing_dark.png"))
    zoom_at(x, y, -4)
    check(ff_until(f"a.phase({i}) === 'Holding'"), "vacates and holds at the end of the exit")
    check(app(pg, f"a.exit_node({i})") == later and app(pg, f"a.verbs({i})") == "T", f"left by the chosen exit; only T offered ({app(pg, f'a.verbs({i})')})")

    # T to a stand, X at any crossing, on stand.
    pg.keyboard.press("t"); frame(pg)
    sx = [n for n in app(pg, "Array.from(a.stand_nodes())") if n != app(pg, "a.route_end(0)")]
    picked = None
    for n in sx[::7]:
        click_node(n)
        if app(pg, "a.preview_edges().length") > 0:
            picked = n
            break
        pg.keyboard.press("Escape"); pg.keyboard.press("t"); frame(pg)
    prev = app(pg, "Array.from(a.preview_edges())")
    check(picked is not None and len(prev) > 0, f"stand {picked} clicked, route drawn ({len(prev)} edges)")
    before = len(app(pg, f"Array.from(a.trail({i}))"))
    pg.keyboard.press("Enter"); frame(pg)
    crossings = 0
    for _ in range(10):
        ff_until(f"a.phase({i}).startsWith('HoldShort') || a.phase({i}) === 'OnStand'")
        if app(pg, f"a.phase({i})") == "OnStand":
            break
        pg.keyboard.press("x"); frame(pg); crossings += 1
    trail = app(pg, f"Array.from(a.trail({i}))")
    check(app(pg, f"a.phase({i})") == "OnStand" and text("state", i).startswith("On stand"), f"on stand after {crossings} crossings: {text('state', i)!r}")
    check(trail[before:] == prev, f"taxi followed the drawn route, edge for edge ({len(prev)} edges)")
    check(app(pg, "a.incident_count()") == 0 and app(pg, "a.go_around_count()") == 0, "no incident, no go-around")

    # ---- no clearance: goes around, plain ink --------------------------------
    load("arrival")
    i = idx("IGO713")
    pg.click(f"#strip{i}"); frame(pg)
    check(ff_until(f"a.phase({i}).startsWith('GoAround')"), "no C: goes around")
    note = text("note", i)
    check(note == "went around: no landing clearance", f"strip note: {note!r}")
    colour = pg.evaluate(f"getComputedStyle(document.getElementById('note{i}')).color")
    check(colour in ("rgb(255, 255, 255)", "rgb(0, 0, 0)") and "alarm" not in pg.get_attribute(f"#strip{i}", "class"), f"go-around in plain ink, no red bar ({colour})")
    rows = pg.locator("#go-arounds li").all_text_contents()
    check(len(rows) == 1 and "GO AROUND · IGO713 (no landing clearance)" in rows[0] and "challenge" not in rows[0], f"go-around log: {rows}")
    row_colour = pg.evaluate("getComputedStyle(document.querySelector('#go-arounds li')).color")
    check(row_colour in ("rgb(255, 255, 255)", "rgb(0, 0, 0)"), f"go-around row in plain ink ({row_colour})")
    check(pg.text_content("#ga-count") == "1" and pg.text_content("#ga-challenges") == "0" and app(pg, "a.incident_count()") == 0, "counts: 1 go-around, 0 challenges, 0 incidents")
    check(ff_until(f"a.strip_state({i}).includes('final again in')"), f"out of sight on the circuit: {text('state', i)!r}")
    check(ff_until(f"a.phase({i}).startsWith('Final')"), "back on final after the circuit")

    # ---- G: instructed -----------------------------------------------------------
    load("arrival")
    i = idx("IGO713")
    pg.click(f"#strip{i}"); frame(pg)
    pg.keyboard.press("g"); frame(pg)
    rows = pg.locator("#go-arounds li").all_text_contents()
    check(app(pg, f"a.phase({i})").startswith("GoAround") and "(instructed)" in rows[0], f"G sends it around: {rows}")

    # ---- pilot go-around: a challenge ---------------------------------------------
    load("pilot")
    i = idx("IGO713")
    pg.click(f"#strip{i}"); frame(pg)
    pg.keyboard.press("c"); frame(pg)
    check(ff_until(f"a.phase({i}).startsWith('GoAround')"), "pilot goes around even after C")
    note = text("note", i)
    check(note == "went around: pilot: unstable approach · challenge", f"strip note: {note!r}")
    check(pg.text_content("#ga-challenges") == "1" and app(pg, "a.incident_count()") == 0, "counted as a challenge, not an incident")

    # ---- R5: threshold crossed with the runway occupied -----------------------------
    load("threshold")
    i, d = idx("IGO713"), idx("IGO512")
    pg.click(f"#strip{i}"); frame(pg)
    hint = text("hint", i)
    check("C clear to land (rwy busy · IGO512)" in hint, f"C hint names who is on the runway, plain ink: {hint!r}")
    pg.keyboard.press("c"); frame(pg)
    check(text("notice", i) == "", "C is never refused for separation")
    check(ff_until("a.incident_count() > 0", 20_000, 1), "crossing the threshold over IGO512: incident")
    rows = pg.locator("#incidents li").all_text_contents()
    check(len(rows) == 1 and "RUNWAY OCCUPIED · IGO713 crossed the 29L threshold with IGO512 on 11R/29L" in rows[0], f"incident row: {rows}")
    check(app(pg, "a.incident_rules()") == "Occupied", "rule is Occupied")
    x, y = app(pg, f"a.aircraft_screen({d})")
    zoom_at(x, y, 6)
    x, y = app(pg, f"a.aircraft_screen({d})")
    sq = red_pixels_near(pg, x, y)
    check(sq > 40, f"red square at IGO512 ({sq} px)")
    pg.screenshot(path=str(OUT / "b5_threshold_dark.png"))
    pg.click("#theme"); frame(pg)
    sq = red_pixels_near(pg, x, y)
    check(sq > 40, f"red square in light theme too ({sq} px)")
    light = pg.evaluate("getComputedStyle(document.querySelector('#incidents li')).color")
    check(light == "rgb(215, 25, 33)", f"light incident text #d71921: {light}")
    pg.screenshot(path=str(OUT / "b5_threshold_light.png"))
    pg.click("#theme"); frame(pg)

    check(not dialogs, f"no alert/confirm dialogs: {dialogs}")
    check(not errs, f"no console or page errors: {errs}")
    b.close()

passed = sum(ok for ok, _ in results)
print(f"{passed}/{len(results)} passed")
sys.exit(0 if passed == len(results) else 1)
