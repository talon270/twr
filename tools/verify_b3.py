"""B3 verification: one departure, stand to airborne, driven by real input.

Every command is a real key press or mouse click. Read-only hooks on
TWR.app give positions and phases; `fast_forward` runs the sim's own tick
code without waiting real time, so a 10-minute taxi checks in seconds.
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

def phase(pg):
    return app(pg, "a.phase(0)")

def ff_until(pg, prefix, max_ticks=400_000, chunk=200):
    t = 0
    while not phase(pg).startswith(prefix) and t < max_ticks:
        app(pg, f"a.fast_forward({chunk})"); t += chunk
    frame(pg)
    return phase(pg).startswith(prefix), t

def strip(pg):
    return {k: pg.text_content(f"#{k}0") for k in ("state", "hint", "notice")}

with sync_playwright() as pw:
    b = pw.chromium.launch()
    pg = b.new_page(viewport={"width": 1500, "height": 950})
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.goto(PAGE); pg.wait_for_timeout(600)
    box = pg.locator("#map").bounding_box()
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]
    print("seed", pg.text_content("#seed"))

    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)

    ax, ay = app(pg, "a.aircraft_screen(0)")
    zoom_at(ax, ay, 9)
    ax, ay = app(pg, "a.aircraft_screen(0)")
    pg.mouse.click(ox + ax, oy + ay); frame(pg)
    s = strip(pg)
    check(s["state"].startswith("On stand") and s["hint"].startswith("P push"), f"on stand, only P offered: {s['state']!r} / {s['hint']!r}")

    pg.keyboard.press("t"); frame(pg)
    check(strip(pg)["notice"] == "can't taxi now" and app(pg, "a.taxi_dest()") == -1, f"T on stand refused inline: {strip(pg)['notice']!r}")

    pg.keyboard.press("p"); frame(pg)
    check(phase(pg) == "Pushback" and strip(pg)["notice"] == "", f"P starts pushback: {strip(pg)['state']!r}")
    app(pg, "a.fast_forward(300)"); frame(pg)
    pg.screenshot(path=str(OUT / "b3_pushback.png"))
    ok, t = ff_until(pg, "Holding")
    check(ok, f"pushback ends Holding after {t} ticks ({t / 20:.0f} game s)")
    check(strip(pg)["hint"].startswith("T taxi"), "after push, T offered: " + strip(pg)["hint"])

    # Taxi to a runway entry with at least 2.5 km ahead: zoom out, then
    # zoom in on the entry so a whole-pixel click lands on it.
    pg.keyboard.press("t"); frame(pg)
    zoom_at(cw / 2, ch / 2, -12)
    entries = app(pg, "Array.from(a.departure_entries(2500))")
    ax, ay = app(pg, "a.aircraft_screen(0)")
    pts = [(n, app(pg, f"a.node_screen({n})")) for n in entries]
    pts = [(n, p) for n, p in pts if 40 < p[0] < cw - 40 and 40 < p[1] < ch - 40]
    # Farthest entry: the most chance of a runway crossing on the way.
    n, (ex, ey) = max(pts, key=lambda v: (v[1][0] - ax) ** 2 + (v[1][1] - ay) ** 2)
    zoom_at(ex, ey, 12)
    ex, ey = app(pg, f"a.node_screen({n})")
    pg.mouse.click(ox + ex, oy + ey); frame(pg)
    check(app(pg, "a.taxi_dest()") == n, f"click picks runway entry {n} (got {app(pg, 'a.taxi_dest()')})")
    prev = app(pg, "Array.from(a.preview_edges())")
    check(len(prev) > 0, f"preview drawn: {len(prev)} edges")
    zoom_at(ex, ey, -8)
    pg.screenshot(path=str(OUT / "b3_preview.png"))
    pg.keyboard.press("Enter"); frame(pg)
    check(phase(pg) == "Taxi", "Enter sends the taxi clearance: " + strip(pg)["state"])

    crossed = 0
    while True:
        ok, t = ff_until(pg, "HoldShort")
        if not ok:
            check(False, f"reached a hold-short (phase {phase(pg)})"); break
        s = strip(pg)
        check(s["state"].startswith("Holding short"), f"held short after {t} ticks: {s['state']!r}, keys {app(pg, 'a.verbs(0)')}")
        if "X" in app(pg, "a.verbs(0)"):
            pg.screenshot(path=str(OUT / "b3_holdshort_cross.png"))
            pg.keyboard.press("l"); frame(pg)
            check("cross (X) instead" in strip(pg)["notice"] and phase(pg).startswith("HoldShort"), "L at a crossing refused inline: " + strip(pg)["notice"])
            pg.keyboard.press("x"); frame(pg)
            check(phase(pg) == "Taxi", "X crosses")
            crossed += 1
            continue
        check("L" in app(pg, "a.verbs(0)"), "at the departure runway, L offered: " + s["hint"])
        pg.screenshot(path=str(OUT / "b3_holdshort.png"))
        break
    print(f"  runway crossings on the way: {crossed}")

    pg.keyboard.press("l"); frame(pg)
    ok, t = ff_until(pg, "LinedUp")
    s = strip(pg)
    check(ok and s["hint"].startswith("T taxi · K takeoff"), f"lined up after {t} ticks: {s['state']!r} / {s['hint']!r}")
    trail = app(pg, "Array.from(a.trail(0))")
    check(trail[1:] == prev, f"taxi followed the drawn route, edge for edge ({len(prev)} edges after the push edge)")
    pg.screenshot(path=str(OUT / "b3_linedup.png"))

    # Paused (Space) for the timed roll: otherwise the page's own frames step
    # the sim between fast-forwards, uncounted, and a loaded machine reads
    # the roll short (640 ticks under a parallel cargo build, 2026-11-07).
    pg.keyboard.press(" ")
    pg.keyboard.press("k"); frame(pg)
    check(phase(pg).startswith("TakeoffRoll"), "K starts the takeoff roll: " + strip(pg)["state"])
    ok, t = ff_until(pg, "Airborne", chunk=20)
    check(ok and 700 <= t <= 800, f"airborne after {t} ticks ({t / 20:.1f} s; 75 m/s at 2 m/s² is 37.5 s): {strip(pg)['state']!r}")
    pg.keyboard.press(" ")
    app(pg, "a.fast_forward(40)"); frame(pg)
    pg.screenshot(path=str(OUT / "b3_airborne.png"))
    ok, t = ff_until(pg, "Departed")
    check(ok and strip(pg)["state"].startswith("Departed"), f"departed: {strip(pg)['state']!r}")

    pg.click("#theme"); frame(pg)
    pg.screenshot(path=str(OUT / "b3_light_departed.png"))
    check(not errs, f"zero console errors: {errs}")
    b.close()

fails = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(fails)} of {len(results)} checks passed")
sys.exit(1 if fails else 0)
