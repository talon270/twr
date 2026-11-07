"""B2 verification: drive dist/index.html with real mouse and keyboard input.

Read-only hooks on TWR.app give screen positions and route lists; every
action itself is a real click, drag, wheel or key press.
"""
from __future__ import annotations
import pathlib, sys, json
from playwright.sync_api import sync_playwright

# The built page next to this folder, so the scripts run on any machine.
PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
results: list[tuple[bool, str]] = []

def check(ok: bool, what: str) -> None:
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, expr: str):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {expr}; }})()")

def canvas_origin(pg):
    b = pg.locator("#map").bounding_box()
    return b["x"], b["y"], b["width"], b["height"]

def run(pw, viewport, reduce=False):
    b = pw.chromium.launch()
    ctx = b.new_context(viewport=viewport, reduced_motion="reduce" if reduce else "no-preference")
    pg = ctx.new_page()
    errs, reqs = [], []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("request", lambda r: reqs.append(r.url))
    pg.goto(PAGE)
    pg.wait_for_timeout(600)
    return b, pg, errs, reqs

def settle(pg, ms=500):
    pg.wait_for_timeout(ms)

def frame(pg):
    """Let the page draw once: the strip is written by the frame loop."""
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

with sync_playwright() as pw:
    b, pg, errs, reqs = run(pw, {"width": 1500, "height": 950})
    ox, oy, cw, ch = canvas_origin(pg)
    check(all(u.startswith("file://") for u in reqs), f"no network requests ({len(reqs)} requests, all file://)")
    check("2026-11-05" in pg.text_content("#attr"), "attribution names the OSM date")
    pg.screenshot(path=str(OUT / "b2_fit_dark.png"))

    # Zoom in on the aircraft with the real wheel; the aircraft must stay
    # under the cursor (zoom pivots on the pointer).
    ax, ay = app(pg, "a.aircraft_screen(0)")
    z0 = app(pg, "a.zoom()")
    # Pointer events carry whole-pixel coordinates, so aim at a whole pixel
    # and check the aircraft's offset from it scales by the zoom ratio.
    mx, my = round(ox + ax) - ox, round(oy + ay) - oy
    pg.mouse.move(ox + mx, oy + my)
    for _ in range(9):
        pg.mouse.wheel(0, -100)
        pg.wait_for_timeout(30)
    settle(pg, 700)
    z1 = app(pg, "a.zoom()")
    bx, by = app(pg, "a.aircraft_screen(0)")
    check(z1 > z0 * 4, f"wheel zooms in: {z0:.3f} -> {z1:.3f} px/m")
    k = z1 / z0
    ex, ey = mx + (ax - mx) * k, my + (ay - my) * k
    check(abs(bx - ex) < 0.05 and abs(by - ey) < 0.05,
          f"zoom pivots on the cursor: aircraft at {bx:.2f},{by:.2f}, expected {ex:.2f},{ey:.2f} (offset {ax-mx:.2f},{ay-my:.2f} px x {k:.2f})")

    # Drag pans 1:1.
    pg.mouse.move(ox + 300, oy + 300); pg.mouse.down(); pg.mouse.move(ox + 420, oy + 380, steps=6); pg.mouse.up()
    cx, cy = app(pg, "a.aircraft_screen(0)")
    check(abs((cx - bx) - 120) < 1 and abs((cy - by) - 80) < 1, f"drag pans 1:1: moved {cx-bx:.1f},{cy-by:.1f} for 120,80")
    check(app(pg, "a.selected_callsign()") == "", "a drag is not a click: nothing selected")
    # Pan back so the aircraft sits mid-canvas.
    pg.mouse.move(ox + 500, oy + 500); pg.mouse.down(); pg.mouse.move(ox + 500 + (cw / 2 - cx), oy + 500 + (ch / 2 - cy), steps=6); pg.mouse.up()
    ax, ay = app(pg, "a.aircraft_screen(0)")

    # Hit slop: a click 15 px off the glyph still selects it.
    pg.mouse.click(ox + ax + 15, oy + ay)
    frame(pg)
    check(app(pg, "a.selected_callsign()") == "IGO512", "click 15 px off the aircraft selects it (44 px target)")
    check("sel" in pg.get_attribute("#strip0", "class"), "strip shows the selection")
    check(pg.text_content("#hint0").startswith("P push"), "hint offers P on stand: " + pg.text_content("#hint0"))
    # B3: taxi needs a pushback first. Push, and run the sim to its end.
    pg.keyboard.press("p")
    for _ in range(200):
        if app(pg, "a.phase(0)") == "Holding":
            break
        app(pg, "a.fast_forward(200)")
    frame(pg)
    check(pg.text_content("#hint0").startswith("T taxi"), "after pushback, hint offers T: " + pg.text_content("#hint0"))

    # T, then Esc cancels.
    pg.keyboard.press("t"); frame(pg)
    check(pg.text_content("#hint0").startswith("Click a stand"), "T enters taxi mode: " + pg.text_content("#hint0"))
    pg.keyboard.press("Escape"); frame(pg)
    check(pg.text_content("#hint0").startswith("T taxi"), "Esc leaves taxi mode")

    # T, click a destination: the on-screen holding point farthest from the aircraft.
    pg.keyboard.press("t")
    holds = app(pg, "Array.from(a.hold_nodes()).map(n => [n, Array.from(a.node_screen(n))])")
    vis = [(n, p) for n, p in holds if 30 < p[0] < cw - 30 and 30 < p[1] < ch - 30]
    if not vis:  # zoomed too far in for any hold; use a stand instead
        vis = [(n, p) for n, p in app(pg, "Array.from(a.stand_nodes()).map(n => [n, Array.from(a.node_screen(n))])")
               if 30 < p[0] < cw - 30 and 30 < p[1] < ch - 30 and abs(p[0] - ax) + abs(p[1] - ay) > 60]
    dest, (dx, dy) = max(vis, key=lambda v: (v[1][0] - ax) ** 2 + (v[1][1] - ay) ** 2)
    pg.mouse.click(ox + dx, oy + dy)
    p0 = app(pg, "Array.from(a.preview_edges())")
    check(len(p0) > 0, f"destination click draws a preview ({len(p0)} edges)")
    pg.screenshot(path=str(OUT / "b2_preview.png"))

    # Right-click a preview segment that a click would hit unambiguously: avoid it.
    def clean_edge(cands):
        for e in cands:
            mx, my = app(pg, f"a.edge_screen({e})")
            if 20 < mx < cw - 20 and 20 < my < ch - 20 and app(pg, f"a.edge_under({mx},{my})") == e:
                return e, mx, my
        return None
    # Middle edges first: avoiding the stand's only exit leaves no route.
    mid = sorted(p0, key=lambda e: abs(p0.index(e) - len(p0) / 2))
    rerouted = None
    tried_avoid = []
    for e in mid:
        hit = clean_edge([e])
        if not hit:
            continue
        _, mx, my = hit
        tried_avoid.append(e)
        pg.mouse.click(ox + mx, oy + my, button="right")
        p1 = app(pg, "Array.from(a.preview_edges())")
        if p1 and e not in p1:
            rerouted = (e, p1)
            break
        frame(pg)
        no_route = "No route" in pg.text_content("#hint0")
        pg.mouse.click(ox + mx, oy + my, button="right")  # toggle the avoid off again
        back = app(pg, "Array.from(a.preview_edges())")
        if not (no_route and back == p0):
            check(False, f"avoid of edge {e} left no route, hint says so, second right-click restores the preview")
    if tried_avoid and not rerouted:
        print(f"  (tried {len(tried_avoid)} of {len(p0)} preview edges; each avoid cut every route)")
    if rerouted:
        e, p1 = rerouted
        check(True, f"right-click avoids edge {e}'s taxiway: preview {len(p0)} -> {len(p1)} edges, edge gone")
    else:
        check(False, f"a right-click avoid produced a different route (seed {pg.text_content('#seed')})")
    # Left-click a taxiway segment off the route: it becomes a via.
    cur = app(pg, "Array.from(a.preview_edges())")
    others = app(pg, f"""(() => {{ const out = []; for (let e = 0; e < a.edge_count(); e += 7) {{
        const m = a.edge_screen(e); if (m[0] > 40 && m[0] < {cw} - 40 && m[1] > 40 && m[1] < {ch} - 40) out.push(e); }}
        return out; }})()""")
    # A dead-end via is refused (U-turn); click it again to undo and try the next.
    via_ok = False
    for v in [e for e in others if e not in cur][:40]:
        hit = clean_edge([v])
        if not hit:
            continue
        _, mx, my = hit
        pg.mouse.click(ox + mx, oy + my)
        p2 = app(pg, "Array.from(a.preview_edges())")
        if v in p2 and all(p2[k] != p2[k + 1] for k in range(len(p2) - 1)):
            frame(pg)
            check(True, f"left-click makes edge {v} a via; preview now {len(p2)} edges, no U-turn")
            check(pg.text_content("#via0").startswith("Via "), "strip lists the via: " + pg.text_content("#via0"))
            via_ok = True
            break
        frame(pg)
        refused = not p2 and "U-turn" in pg.text_content("#hint0")
        pg.mouse.click(ox + mx, oy + my)  # toggle off
        check(refused and app(pg, "Array.from(a.preview_edges())") == cur, f"via {v} refused with a reason, second click restores the preview")
    check(via_ok, "found a via that gives a route")

    sent = app(pg, "Array.from(a.preview_edges())")
    pg.screenshot(path=str(OUT / "b2_via.png"))
    pg.screenshot(path=str(OUT / "b2_via_zoom.png"), clip={"x": ox + cw / 2 - 350, "y": oy + 40, "width": 700, "height": 450})
    t0 = app(pg, "a.ticks()")
    pg.keyboard.press("Enter")
    check(app(pg, "a.moving(0)"), "Enter sends: aircraft rolling")
    p_a = app(pg, "a.aircraft_screen(0)")
    pg.wait_for_timeout(1000)
    p_b = app(pg, "a.aircraft_screen(0)")
    moved_px = ((p_b[0] - p_a[0]) ** 2 + (p_b[1] - p_a[1]) ** 2) ** 0.5
    check(moved_px > 1, f"aircraft moves in real time: {moved_px:.1f} px in 1 s at {z1:.2f} px/m (expect ~30 m)")
    pg.screenshot(path=str(OUT / "b2_rolling.png"))

    # Space pauses.
    pg.keyboard.press(" ")
    frame(pg)
    k1 = app(pg, "a.ticks()"); pg.wait_for_timeout(400); k2 = app(pg, "a.ticks()")
    check(k1 == k2 and pg.is_visible("#paused"), f"Space pauses: ticks {k1} -> {k2}")
    pg.keyboard.press(" ")
    pg.wait_for_timeout(200)
    check(app(pg, "a.ticks()") > k2, "Space resumes")

    # B3: a route across a runway stops short of it; clear each with a real X.
    for _ in range(20):
        app(pg, "a.fast_forward(200000)")
        if not app(pg, "a.phase(0)").startswith("HoldShort"):
            break
        pg.keyboard.press("x")
    trail = app(pg, "Array.from(a.trail(0))")
    check(not app(pg, "a.moving(0)") and trail[1:] == sent, f"followed path equals drawn path, edge for edge ({len(sent)} edges after the push edge)")
    check(app(pg, "a.route_end(0)") == dest, "aircraft stops at the clicked destination")

    # Theme click must not steal the next Enter.
    pg.click("#theme")
    theme = pg.evaluate("document.documentElement.dataset.theme")
    pg.wait_for_timeout(100)
    pg.screenshot(path=str(OUT / "b2_light.png"))
    ax, ay = app(pg, "a.aircraft_screen(0)")
    pg.mouse.click(ox + ax, oy + ay)
    # Since B5 a route that ends at a stand ends OnStand, and T needs a push
    # first. The random destination above is sometimes a stand (4 of 8 runs).
    if app(pg, "a.phase(0)") == "OnStand":
        pg.click("#strip0"); pg.keyboard.press("p")
        while app(pg, "a.phase(0)") != "Holding":
            app(pg, "a.fast_forward(20)")
    pg.keyboard.press("t")
    stands = [(n, p) for n, p in app(pg, "Array.from(a.stand_nodes()).map(n => [n, Array.from(a.node_screen(n))])")
              if 30 < p[0] < cw - 30 and 30 < p[1] < ch - 30]
    n, (sx, sy) = stands[0]
    pg.mouse.click(ox + sx, oy + sy)
    pg.keyboard.press("Enter")
    check(pg.evaluate("document.documentElement.dataset.theme") == theme and app(pg, "a.moving(0)") or app(pg, "a.route_end(0)") == n,
          "Enter after clicking the theme button sends the clearance, theme unchanged")
    check(not errs, f"zero console errors (1500x950): {errs}")
    b.close()

    # 1920 wide: map fills the window, no stranded band.
    b, pg, errs, _ = run(pw, {"width": 1920, "height": 1080})
    box = pg.locator("#map").bounding_box()
    check(abs(box["y"] + box["height"] - 1080) < 1, f"1920x1080: map reaches the bottom ({box['y'] + box['height']:.0f})")
    pg.screenshot(path=str(OUT / "b2_1920.png"))
    check(not errs, f"zero console errors (1920): {errs}")
    b.close()

    # Reduced motion: zoom jumps, no spring.
    b, pg, errs, _ = run(pw, {"width": 1500, "height": 950}, reduce=True)
    ox, oy, cw, ch = canvas_origin(pg)
    z0 = app(pg, "a.zoom()")
    pg.mouse.move(ox + cw / 2, oy + ch / 2); pg.mouse.wheel(0, -100)
    pg.wait_for_timeout(60)
    z1 = app(pg, "a.zoom()")
    pg.wait_for_timeout(400)
    z2 = app(pg, "a.zoom()")
    check(z1 > z0 and abs(z2 - z1) < 1e-9, f"reduced motion: zoom lands at once ({z0:.4f} -> {z1:.4f}, 400 ms later {z2:.4f})")
    check(not errs, f"zero console errors (reduced motion): {errs}")
    b.close()

fails = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(fails)} of {len(results)} checks passed")
sys.exit(1 if fails else 0)
