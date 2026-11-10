"""A1 verification: aircraft are true-scale silhouettes on the page.

Checks that an A20N on stand, zoomed in with the real wheel, spans 35.8 m
±2%; that a widebody draws wider than it; that the 14 px floor holds at the
fitted zoom; that a click on a zoomed-in aircraft's tail selects it; and
zero page errors in both themes.

The span is read from `aircraft_outline`, the same points the frame fills,
not from screenshot pixels: the screenshots are for the eye.
Run: python3 tools/verify_a1.py <screenshot dir>
"""
from __future__ import annotations
import math, pathlib, sys
from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
PAGE = (ROOT / "dist" / "index.html").as_uri()
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

def outline(pg, i):
    f = app(pg, f"Array.from(a.aircraft_outline({i}))")
    return list(zip(f[0::2], f[1::2]))

def span_len_px(pts):
    """Width across and along the fuselage axis. pts[0] is the nose; the
    ring mirrors left onto right, so the mean of its points is on the axis.
    (The point farthest from the nose is not: on a 777 it is a tailplane tip.)"""
    nose = pts[0]
    mid = (sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts))
    ax = ((mid[0] - nose[0]), (mid[1] - nose[1]))
    n = math.hypot(*ax); ax = (ax[0] / n, ax[1] / n)
    perp = [(p[0] - nose[0]) * -ax[1] + (p[1] - nose[1]) * ax[0] for p in pts]
    along = [(p[0] - nose[0]) * ax[0] + (p[1] - nose[1]) * ax[1] for p in pts]
    return max(perp) - min(perp), max(along) - min(along)

def find(pg, typ, phase="OnStand"):
    for i in range(app(pg, "a.count()")):
        if app(pg, f"a.wake({i})").split()[0] == typ and app(pg, f"a.phase({i})") == phase and not app(pg, f"a.gone({i})"):
            return i
    return None

with sync_playwright() as pw:
    b = pw.chromium.launch()
    ctx = b.new_context(viewport={"width": 1500, "height": 950})
    pg = ctx.new_page()
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))
    pg.goto(PAGE); pg.wait_for_timeout(600)
    box = pg.locator("#map").bounding_box()
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]

    # Peak spawns departures onto stands; a few seeds give an A20N and a
    # widebody on stand together.
    i = w = None
    for seed in range(1, 40):
        app(pg, f"a.restart('peak', {seed})")
        app(pg, "a.fast_forward(6000)"); frame(pg)
        i = find(pg, "A20N")
        w = next((find(pg, t) for t in ("B77W", "B788", "B789", "A359", "A333", "A388") if find(pg, t) is not None), None)
        if i is not None and w is not None:
            break
    check(i is not None and w is not None, f"seed {seed}: an A20N ({i}) and a widebody ({w}) on stand")

    # Fitted zoom: every visible aircraft is at least 14 px across.
    spans = [span_len_px(outline(pg, k))[0] for k in range(app(pg, "a.count()"))
             if app(pg, f"a.phase({k})") == "OnStand" and not app(pg, f"a.gone({k})")]
    z_fit = app(pg, "a.zoom()")
    check(spans and min(spans) >= 14.0 - 0.01, f"fitted zoom {z_fit:.3f} px/m: narrowest silhouette {min(spans):.2f} px (floor 14)")

    # Zoom in on the A20N with the real wheel.
    for _ in range(12):
        ax, ay = app(pg, f"a.aircraft_screen({i})")
        pg.mouse.move(ox + ax, oy + ay)
        pg.mouse.wheel(0, -100); pg.wait_for_timeout(40)
    pg.wait_for_timeout(500); frame(pg)
    z = app(pg, "a.zoom()")
    s_px, l_px = span_len_px(outline(pg, i))
    check(abs(s_px / z - 35.8) <= 35.8 * 0.02, f"A20N at {z:.2f} px/m spans {s_px:.1f} px = {s_px / z:.2f} m (35.80 ±2%)")
    check(abs(l_px / z - 37.57) <= 37.57 * 0.02, f"A20N length {l_px / z:.2f} m (37.57 ±2%)")

    # Click the tail, far outside the old 22 px reach of the position point.
    pts = outline(pg, i)
    tail = max(pts, key=lambda p: math.dist(p, pts[0]))
    nose = pts[0]
    tx, ty = tail[0] + (nose[0] - tail[0]) * 0.05, tail[1] + (nose[1] - tail[1]) * 0.05
    reach = math.dist((tx, ty), app(pg, f"a.aircraft_screen({i})"))
    pg.mouse.click(ox + tx, oy + ty); frame(pg)
    check(app(pg, "a.selected_callsign()") == app(pg, f"a.callsign({i})"),
          f"click on the tail, {reach:.0f} px from the position point, selects it")

    for theme in ("dark", "light"):
        if pg.get_attribute("html", "data-theme") != theme:
            pg.click("#theme"); frame(pg)
        pg.screenshot(path=str(OUT / f"a1_a20n_{theme}.png"))

    # The widebody, same zoom: wider than the A20N by its published ratio.
    ws, _ = span_len_px(outline(pg, w))
    wt = app(pg, f"a.wake({w})").split()[0]
    pub = {"B77W": 64.8, "B788": 60.14, "B789": 60.14, "A359": 64.75, "A333": 60.29, "A388": 79.75}[wt]
    check(abs(ws / z - pub) <= pub * 0.02, f"{wt} spans {ws / z:.2f} m ({pub} ±2%)")
    wx, wy = app(pg, f"a.aircraft_screen({w})")
    app(pg, f"a.pan({box['width'] / 2 - wx}, {box['height'] / 2 - wy})"); frame(pg)
    for theme in ("light", "dark"):
        if pg.get_attribute("html", "data-theme") != theme:
            pg.click("#theme"); frame(pg)
        pg.screenshot(path=str(OUT / f"a1_{wt.lower()}_{theme}.png"))

    # Hit slop (44 px target) is measured from the drawn shape, not from its
    # middle: a click 15 px off the position point, in four directions, on 20
    # seeds, each with a different heading. The first A1 build missed 40 of 80.
    miss, total = [], 0
    for seed in range(1, 21):
        app(pg, f"a.restart('practice', {seed})"); pg.wait_for_timeout(100)
        ax, ay = app(pg, "a.aircraft_screen(0)")
        pg.mouse.move(ox + ax, oy + ay)
        for _ in range(9):
            pg.mouse.wheel(0, -100); pg.wait_for_timeout(30)
        pg.wait_for_timeout(500)
        for dx, dy in ((15, 0), (-15, 0), (0, 15), (0, -15)):
            app(pg, "a.key('Escape')")
            x, y = app(pg, "a.aircraft_screen(0)")
            if not (20 < x + dx < cw - 20 and 20 < y + dy < ch - 20):
                continue
            pg.mouse.click(ox + x + dx, oy + y + dy); pg.wait_for_timeout(30)
            total += 1
            if app(pg, "a.selected_callsign()") != "IGO512":
                miss.append((seed, dx, dy))
    check(total >= 60 and not miss, f"click 15 px off the position point selects it: {total - len(miss)} of {total} ({miss[:4]})")

    check(not errs, f"no page error or dialog ({errs[:2]})")
    b.close()

fails = sum(not ok for ok, _ in results)
print(f"{len(results) - fails} PASS, {fails} FAIL")
sys.exit(1 if fails else 0)
