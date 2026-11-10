"""UX1 verification: five bugs reported on a test run of the A1 build,
2026-11-07. Every action is a real click, wheel or key press; `fast_forward`
only moves time, and read-only hooks report state.

  1. a departed aircraft's strip leaves the bay (Practice included)
  2. verb buttons on the selected strip: Line up and Takeoff, by real click
     (Line up is covered in verify_b3.py, where the aircraft reaches the runway)
  3. S: push, then the taxi route builder opens by itself when the push ends
  4. an arrival that parks leaves the bay after 120 real s (Practice included)
  5. an aircraft that asks for something is tinted and says so in words (no pulse since PLAN-ui-console A5)

Run: python3 tools/verify_ux1.py <screenshot dir>
"""
from __future__ import annotations
import pathlib, sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
OUT.mkdir(parents=True, exist_ok=True)
results: list[tuple[bool, str]] = []

def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, e):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {e}; }})()")

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def ff_until(pg, cond, max_ticks=400_000, chunk=100):
    t = 0
    while not app(pg, cond) and t < max_ticks:
        app(pg, f"a.fast_forward({chunk})"); t += chunk
    frame(pg)
    return app(pg, cond)

def keys(pg, i):
    # Step 2: the verbs sit in #sel under the map, for the selected strip only.
    return pg.evaluate(f"document.getElementById('sel').dataset.i === '{i}' ? [...document.querySelectorAll('#sel-verbs button')].map(b => b.dataset.key).join('') : ''")

def open_page(pw, reduced=False):
    b = pw.chromium.launch()
    ctx = b.new_context(viewport={"width": 1500, "height": 950}, reduced_motion="reduce" if reduced else "no-preference")
    pg = ctx.new_page()
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    return b, pg, errs

with sync_playwright() as pw:
    b, pg, errs = open_page(pw)
    box = pg.locator("#map").bounding_box()
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]

    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)

    # ---- 5: a request shows, in words and a static tint ---------------------
    # Practice: IGO713 is on final and not cleared; IGO512 has no due time.
    cls = lambda i: pg.get_attribute(f"#strip{i}", "class")
    check("req" in cls(1) and pg.text_content("#ask1") == "requests landing clearance", f"IGO713 on final asks: {pg.text_content('#ask1')!r}")
    check("req" not in cls(0) and pg.text_content("#ask0") == "", "IGO512 on stand, no due time: no request")
    # PLAN-ui-console A5: the pulse is gone. The tint layer is static.
    anim = pg.evaluate("getComputedStyle(document.getElementById('strip1'), '::before').animationName")
    check(anim == "none", f"the request tint does not loop (animation {anim})")
    pg.screenshot(path=str(OUT / "ux1_flash_dark.png"))
    pg.click("#strip1"); frame(pg)
    pg.keyboard.press("c"); frame(pg)
    check("req" not in cls(1) and pg.text_content("#ask1") == "", "C clears the request")
    # A shift departure past its grace asks to push.
    pg.select_option("#mode", "normal"); pg.fill("#seed-in", "7"); pg.click("#start"); frame(pg)
    got = ff_until(pg, "Array.from({length: a.count()}, (_, i) => a.request_text(i)).includes('ready to push')", max_ticks=200_000)
    check(got, "a departure on stand past its grace asks: ready to push")
    pg.click("#theme"); frame(pg)
    pg.screenshot(path=str(OUT / "ux1_flash_light.png"))
    pg.click("#theme"); frame(pg)

    # ---- 3: S pushes, then the taxi builder opens ---------------------------
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    pg.click("#strip0"); frame(pg)
    check(keys(pg, 0) == "PS", f"IGO512 on stand: buttons {keys(pg, 0)!r} (P push, S push + taxi)")
    check(pg.text_content("#sel-hint").startswith("P push · S push + taxi"), f"hint names S: {pg.text_content('#sel-hint')!r}")
    pg.click("#sel-verbs button[data-key=S]"); pg.keyboard.press("Enter"); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback", "the S button pushes back")
    check(not pg.text_content("#sel-hint").startswith("Click a stand"), "no taxi builder while still pushing")
    check(ff_until(pg, "a.phase(0) === 'Holding'"), "push ends at Holding")
    check(pg.text_content("#sel-hint").startswith("Click a stand"), f"the taxi builder opened by itself: {pg.text_content('#sel-hint')!r}")
    pg.keyboard.press("Escape"); frame(pg)
    check(pg.text_content("#sel-hint").startswith("T taxi"), "Esc leaves the builder; T is still offered")
    # The S key, then pick another aircraft: the builder must not open.
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    pg.click("#strip0"); pg.keyboard.press("s"); pg.keyboard.press("Enter"); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback", "the S key pushes back")
    pg.click("#strip1"); frame(pg)
    ff_until(pg, "a.phase(0) === 'Holding'")
    check(pg.get_attribute("#sel", "data-i") == "1" and not pg.text_content("#sel-hint").startswith("Click a stand") and app(pg, "a.taxi_dest()") == -1, "selected elsewhere: no builder opens")
    pg.click("#strip0"); frame(pg)
    check(pg.text_content("#sel-hint").startswith("T taxi"), "back on IGO512: the builder did not open late")

    # ---- 2 and 1: Takeoff button, then the departed strip leaves ------------
    check(app(pg, "a.load_scenario('runway')"), "scenario: IGO512 lined up on 29R")
    frame(pg)
    i = app(pg, "a.index_of('IGO512')")
    pg.click(f"#strip{i}"); frame(pg)
    check("K" in keys(pg, i), f"lined up: buttons {keys(pg, i)!r} include K")
    pg.screenshot(path=str(OUT / "ux1_buttons.png"), clip={"x": cw, "y": oy, "width": 320, "height": 380})
    pg.click(f"#sel-verbs button[data-key=K]"); frame(pg)
    check(app(pg, f"a.phase({i})").startswith("TakeoffRoll"), "the Takeoff button starts the roll: " + app(pg, f"a.phase({i})"))
    check(ff_until(pg, f"a.phase({i}).startsWith('Departed')"), "departed")
    check(not pg.is_visible(f"#strip{i}"), "the departed strip left the bay in Practice")

    # ---- 4: an arrival parks and its strip leaves ---------------------------
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    landed = False
    for seed in range(1, 12):
        app(pg, f"a.restart('practice', {seed})"); frame(pg)
        i = app(pg, "a.index_of('IGO713')")
        pg.click(f"#strip{i}"); pg.keyboard.press("c"); frame(pg)
        ff_until(pg, f"a.phase({i}) === 'Holding' || a.phase({i}).startsWith('GoAround')", chunk=20)
        if app(pg, f"a.phase({i})") == "Holding":
            landed = True; break
    check(landed, f"IGO713 cleared with C, landed, vacated (seed {seed})")
    pg.keyboard.press("t"); frame(pg)
    taken = set(app(pg, "Array.from({length: a.count()}, (_, j) => a.route_end(j))"))
    def click_node(n):
        x, y = app(pg, f"a.node_screen({n})")
        zoom_at(x, y, 10)
        x, y = app(pg, f"a.node_screen({n})")
        pg.mouse.click(ox + round(x), oy + round(y)); frame(pg)
        zoom_at(x, y, -10)
    picked = None
    for n in [n for n in app(pg, "Array.from(a.stand_nodes())") if n not in taken][::7]:
        click_node(n)
        if app(pg, "a.preview_edges().length") > 0:
            picked = n; break
        pg.keyboard.press("Escape"); pg.click(f"#strip{i}"); pg.keyboard.press("t"); frame(pg)
    pg.keyboard.press("Enter"); frame(pg)
    for _ in range(10):
        ff_until(pg, f"a.phase({i}).startsWith('HoldShort') || a.phase({i}) === 'OnStand'", chunk=20)
        if app(pg, f"a.phase({i})") == "OnStand":
            break
        pg.keyboard.press("x"); frame(pg)
    check(app(pg, f"a.phase({i})") == "OnStand" and pg.is_visible(f"#strip{i}"), f"IGO713 on stand {picked}, strip still shown")
    on_at = app(pg, "a.real_s()")
    check(ff_until(pg, f"a.phase({i}) === 'ShutDown'", chunk=60), "Practice arrival shut down on its stand")
    after = app(pg, "a.real_s()") - on_at
    check(115 <= after <= 125, f"shut down {after:.0f} real s after reaching the stand (120)")
    check(not pg.is_visible(f"#strip{i}"), "its strip left the bay")
    check(not errs, f"no page error or dialog ({errs[:2]})")
    b.close()

    # ---- 5, reduced motion: no pulse, the tint stays ------------------------
    b, pg, errs = open_page(pw, reduced=True)
    tint = "(() => { const s = getComputedStyle(document.getElementById('strip1'), '::before'); return [s.animationName, s.opacity, s.backgroundColor]; })()"
    anim, op, bg = pg.evaluate(tint)
    plain = pg.evaluate("getComputedStyle(document.getElementById('strip0')).backgroundColor")
    check(anim == "none" and op == "1" and bg != plain, f"reduced motion: no animation ({anim}), tint layer at opacity {op}, {bg} against {plain}")
    check(not errs, f"no page error ({errs[:2]})")
    b.close()

fails = sum(not ok for ok, _ in results)
print(f"{len(results) - fails} PASS, {fails} FAIL")
sys.exit(1 if fails else 0)
