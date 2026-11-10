"""R1 shell verification (PLAN-twr.md § R1): one check per shell item.

  R1.5   zoom works while paused
  R1.6   after a seed and Enter, game keys work
  R1.8   a second finger does not take over the pan
  R1.13  a pixel-ratio change at the same size refits the canvas
  R1.15  first paint follows the saved theme, else the system's
  R1.16  a resize does not paint a blank canvas
  R1.22  a paused page does no strip work, and input still updates strips
  R1.25  press spring, squircle, 44 px hit area, dialog fits a phone

Every action is a real click, wheel, key, touch or viewport change. Read-only
hooks (TWR.app.zoom, node_screen, revision) report state; `fast_forward` only
moves time. Each check fails on the pre-R1 page and passes after R1.

Run: python3 tools/verify_r1.py [page.html]   (default dist/index.html)
Exit status 1 if any check fails.
"""
from __future__ import annotations
import pathlib, sys
from playwright.sync_api import sync_playwright

PAGE = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").resolve().as_uri()
results: list[tuple[bool, str]] = []

def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, e):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {e}; }})()")

def frame(pg, n=2):
    pg.evaluate(f"new Promise(r => {{ let k = {n}; const f = () => --k ? requestAnimationFrame(f) : r(); requestAnimationFrame(f); }})")

def open_page(b, theme="dark", w=1500, h=950, touch=False, init=None, url=""):
    ctx = b.new_context(viewport={"width": w, "height": h}, color_scheme=theme, has_touch=touch)
    if init:
        ctx.add_init_script(init)
    pg = ctx.new_page()
    errs: list[str] = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))
    pg.goto(PAGE + url); pg.wait_for_timeout(600); frame(pg)
    return ctx, pg, errs

def centre(pg):
    box = pg.locator("#map").bounding_box()
    return box["x"] + box["width"] / 2, box["y"] + box["height"] / 2

with sync_playwright() as pw:
    b = pw.chromium.launch()
    all_errs: list[str] = []

    # ---- R1.5: zoom while paused --------------------------------------------
    for theme in ("dark", "light"):
        ctx, pg, errs = open_page(b, theme)
        pg.keyboard.press(" "); frame(pg)
        z0 = app(pg, "a.zoom()")
        x, y = centre(pg)
        pg.mouse.move(x, y)
        for _ in range(3):
            pg.mouse.wheel(0, -100); pg.wait_for_timeout(30)
        pg.wait_for_timeout(600)
        z1 = app(pg, "a.zoom()")
        t = app(pg, "a.ticks()"); frame(pg, 10)
        still = app(pg, "a.ticks()") == t
        check(z1 > z0 * 1.5 and still and pg.is_visible("#paused"),
              f"R1.5 [{theme}] paused: 3 wheel notches zoom {z0:.4f} -> {z1:.4f}, sim held ({still})")
        all_errs += errs; ctx.close()

    # ---- R1.6: Enter in the seed field, then game keys -----------------------
    ctx, pg, errs = open_page(b)
    pg.click("#seed-in"); pg.keyboard.type("7"); pg.keyboard.press("Enter"); frame(pg)
    pg.click("#strip0"); frame(pg)
    sel = app(pg, "a.selected_callsign()")
    pg.keyboard.press("Escape"); frame(pg)
    after_esc = app(pg, "a.selected_callsign()")
    pg.keyboard.press(" "); frame(pg)
    check(pg.text_content("#seed") == "7" and sel != "" and after_esc == "" and pg.is_visible("#paused"),
          f"R1.6 seed 7 + Enter: Esc clears selection ({sel!r} -> {after_esc!r}), Space pauses ({pg.is_visible('#paused')})")
    all_errs += errs; ctx.close()

    # ---- R1.8: a second finger ----------------------------------------------
    ctx, pg, errs = open_page(b, touch=True)
    cdp = ctx.new_cdp_session(pg)
    x, y = centre(pg)
    n0 = app(pg, "Array.from(a.node_screen(0))")
    tp = lambda pts: [{"x": px, "y": py, "id": i} for i, (px, py) in pts]
    one = (0, (x, y))
    cdp.send("Input.dispatchTouchEvent", {"type": "touchStart", "touchPoints": tp([one])})
    cdp.send("Input.dispatchTouchEvent", {"type": "touchStart", "touchPoints": tp([one, (1, (x + 100, y))])})
    for k in range(1, 11):
        cdp.send("Input.dispatchTouchEvent", {"type": "touchMove", "touchPoints": tp([one, (1, (x + 100 + 15 * k, y + 10 * k))])})
    frame(pg)
    n1 = app(pg, "Array.from(a.node_screen(0))")
    cdp.send("Input.dispatchTouchEvent", {"type": "touchEnd", "touchPoints": tp([one])})
    cdp.send("Input.dispatchTouchEvent", {"type": "touchEnd", "touchPoints": []})
    frame(pg)
    moved = abs(n1[0] - n0[0]) + abs(n1[1] - n0[1])
    # And the first finger still pans on its own.
    cdp.send("Input.dispatchTouchEvent", {"type": "touchStart", "touchPoints": tp([one])})
    for k in range(1, 11):
        cdp.send("Input.dispatchTouchEvent", {"type": "touchMove", "touchPoints": tp([(0, (x + 6 * k, y))])})
    cdp.send("Input.dispatchTouchEvent", {"type": "touchEnd", "touchPoints": []})
    frame(pg)
    n2 = app(pg, "Array.from(a.node_screen(0))")
    check(moved < 1 and abs(n2[0] - n1[0] - 60) < 2,
          f"R1.8 second finger moved 150 px: map moved {moved:.1f} px; one finger then pans {n2[0] - n1[0]:.1f} px (60 expected)")
    all_errs += errs; ctx.close()

    # ---- R1.13: pixel ratio changes, CSS size does not -----------------------
    ctx, pg, errs = open_page(b)
    cdp = ctx.new_cdp_session(pg)
    cdp.send("Emulation.setDeviceMetricsOverride", {"width": 1500, "height": 950, "deviceScaleFactor": 2, "mobile": False})
    pg.wait_for_timeout(300); frame(pg)
    cw, rw, dpr = pg.evaluate("[document.getElementById('map').width, document.getElementById('map').getBoundingClientRect().width, devicePixelRatio]")
    check(dpr == 2 and cw == round(rw * 2), f"R1.13 dpr 1 -> {dpr}: canvas {cw} px for {rw:.0f} CSS px (want {round(rw * 2)})")
    all_errs += errs; ctx.close()

    # ---- R1.15: theme at first paint -----------------------------------------
    # Record data-theme at the moment <body> is inserted: anything painted
    # before boot() runs uses that value.
    spy = """new MutationObserver((_, o) => { if (document.body) { window.__firstTheme = document.documentElement.dataset.theme; o.disconnect(); } })
               .observe(document, { childList: true, subtree: true });"""
    seed_light = "localStorage.setItem('twr.v1', JSON.stringify({ v: 2, settings: { theme: 'light' }, bests: {} }));"
    ctx, pg, errs = open_page(b, "dark", init=seed_light + spy)
    first, now = pg.evaluate("[window.__firstTheme, document.documentElement.dataset.theme]")
    check(first == "light" and now == "light", f"R1.15 saved light, system dark: first paint {first}, after boot {now}")
    all_errs += errs; ctx.close()
    ctx, pg, errs = open_page(b, "light", init=spy)
    first, now = pg.evaluate("[window.__firstTheme, document.documentElement.dataset.theme]")
    btn = pg.text_content("#theme")
    check(first == "light" and now == "light" and btn == "Dark", f"R1.15 nothing saved, system light: first paint {first}, after boot {now}, button {btn!r}")
    all_errs += errs; ctx.close()
    ctx, pg, errs = open_page(b, "dark", init=spy)
    first, now = pg.evaluate("[window.__firstTheme, document.documentElement.dataset.theme]")
    check(first == "dark" and now == "dark", f"R1.15 nothing saved, system dark: first paint {first}, after boot {now} (regression)")
    all_errs += errs; ctx.close()

    # ---- R1.16: resize must not paint a blank frame --------------------------
    # This observer is made after the page's, so it runs after fit() in the
    # same frame, just before paint: what it reads is what gets painted.
    for theme in ("dark", "light"):
        ctx, pg, errs = open_page(b, theme)
        pg.evaluate("""() => { window.__alpha = []; const c = document.getElementById('map'), g = c.getContext('2d');
          new ResizeObserver(() => window.__alpha.push(g.getImageData(c.width >> 1, c.height >> 1, 1, 1).data[3])).observe(c); }""")
        frame(pg)
        pg.set_viewport_size({"width": 1300, "height": 900}); pg.wait_for_timeout(200); frame(pg)
        pg.set_viewport_size({"width": 1500, "height": 950}); pg.wait_for_timeout(200); frame(pg)
        alpha = pg.evaluate("window.__alpha")
        check(len(alpha) >= 3 and all(a == 255 for a in alpha), f"R1.16 [{theme}] canvas alpha at paint after each resize: {alpha} (0 = blank)")
        all_errs += errs; ctx.close()

    # ---- R1.22: no strip work while nothing changes -------------------------
    ctx, pg, errs = open_page(b)
    pg.keyboard.press(" "); frame(pg, 3)
    pg.evaluate("""() => { const a = TWR.app, o = a.strip_state.bind(a); window.__calls = 0;
                           a.strip_state = i => (window.__calls++, o(i)); }""")
    frame(pg, 30)
    idle = pg.evaluate("window.__calls")
    pg.click("#strip1"); frame(pg)
    sel_ok = "sel" in (pg.get_attribute("#strip1", "class") or "")
    after_click = pg.evaluate("window.__calls")
    pg.keyboard.press(" "); frame(pg, 5)
    running = pg.evaluate("window.__calls") - after_click
    check(idle == 0 and sel_ok and after_click > 0 and running > 0,
          f"R1.22 paused 30 frames: {idle} strip reads; click while paused selects ({sel_ok}); running 5 frames: {running} reads")
    all_errs += errs; ctx.close()

    # ---- R1.25: handbook items -----------------------------------------------
    for theme in ("dark", "light"):
        ctx, pg, errs = open_page(b, theme)
        ease = pg.evaluate("getComputedStyle(document.getElementById('start')).transitionTimingFunction")
        corner = pg.evaluate("CSS.supports('corner-shape', 'squircle') ? getComputedStyle(document.getElementById('start')).cornerShape : 'unsupported'")
        # PLAN-ui-console A3: the controls are drawn 44 px tall, so the hit
        # area is the drawn box. Click 4 px inside its top and bottom edges.
        theme0 = pg.evaluate("document.documentElement.dataset.theme")
        tb = pg.locator("#theme").bounding_box()
        pg.mouse.click(tb["x"] + tb["width"] / 2, tb["y"] + 4); frame(pg)
        theme1 = pg.evaluate("document.documentElement.dataset.theme")
        pg.fill("#seed-in", "4242")
        sb = pg.locator("#start").bounding_box()
        pg.mouse.click(sb["x"] + sb["width"] / 2, sb["y"] + sb["height"] - 4); frame(pg)
        seed = pg.text_content("#seed")
        hs = [round(pg.locator(s).bounding_box()["height"]) for s in ("#theme", "#start", "#mode", "#seed-in")]
        check("linear(" in ease and corner in ("squircle", "superellipse(2)", "unsupported") and theme1 != theme0 and seed == "4242" and min(hs) >= 44,
              f"R1.25 [{theme}] press easing {ease[:30]}...; corner-shape {corner}; heights theme/start/mode/seed {hs}; edge of theme toggles ({theme0}->{theme1}); edge of Start starts seed {seed}")
        # Dialog on a phone-width window, at the end of a Light shift.
        pg.set_viewport_size({"width": 400, "height": 800})
        pg.select_option("#mode", "light"); pg.fill("#seed-in", "3"); pg.click("#start"); frame(pg)
        while not app(pg, "a.over()"):
            app(pg, "a.fast_forward(6000)")
        frame(pg, 4)
        box = pg.locator("#result").bounding_box()
        sub = pg.text_content("#res-sub")
        check(box and box["x"] >= 0 and box["x"] + box["width"] <= 400 and box["width"] <= 368 and sub.endswith("30:00"),
              f"R1.25 [{theme}] result dialog at 400 px: x {box['x']:.0f}, width {box['width']:.0f} (max 368); sub ends {sub[-12:]!r}")
        all_errs += errs; ctx.close()

    check(not all_errs, f"no page errors: {all_errs[:3]}")
    b.close()

bad = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(bad)}/{len(results)} passed")
sys.exit(1 if bad else 0)
