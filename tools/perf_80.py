"""Frame time with 80+ aircraft in play (PLAN-twr.md § B9).

Two states: "parked", as measured early (most aircraft wait on stands), and
"moving" (B9): real keys and clicks push every departure on a stand and
taxi it to the far entry of its runway, so 30 or more are on the move when
the frames are timed. The ground conflict check grows with moving aircraft.

Surge with nobody at the controls fills the airport: departures wait on
stands, arrivals circle. `app.frame` is wrapped to time the sim step plus the
canvas draw; rAF intervals give the whole frame, strips included. The launch
flags put headless Chromium on the real GPU; without them it draws in
software (SwiftShader), and at 2x pixel density every frame then takes
about 50 ms. That is a property of the test rig, not of the page.
"""
from __future__ import annotations
import pathlib, statistics, sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()

def app(pg, e):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {e}; }})()")

def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p / 100 * len(xs)))]

MOVING = "['Pushback', 'Taxi'].includes(a.phase(i))"

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def set_moving(pg, live):
    """Pause, push and taxi every departure on a stand, unpause."""
    box = pg.locator("#map").bounding_box()
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]
    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(20)
        pg.wait_for_timeout(500)
    deps = app(pg, f"{live}.filter(i => a.phase(i) === 'OnStand' && / · dep /.test(a.strip_state(i)))")
    # The strip names the runway only while on stand: read it before the push.
    rwys = {i: app(pg, f"a.strip_state({i})").split("dep ")[-1].split()[0] for i in deps}
    pg.keyboard.press(" ")
    for i in deps:
        pg.click(f"#strip{i}"); pg.keyboard.press("p")
    pg.keyboard.press(" ")
    # Pushback is 1 m/s: let them all reach Holding.
    for _ in range(40):
        if all(app(pg, f"a.phase({i})") != "Pushback" for i in deps):
            break
        app(pg, "a.fast_forward(200)")
    pg.keyboard.press(" ")
    sent = 0
    for i in deps:
        if app(pg, f"a.phase({i})") != "Holding":
            continue
        rwy = rwys[i]
        need = 3100 if app(pg, f"a.wake({i})").endswith(("H", "J")) else 2030
        entries = app(pg, f"Array.from(a.entries_for('{rwy}', {need}))")
        ax, ay = app(pg, f"a.aircraft_screen({i})")
        pts = [(e, app(pg, f"a.node_screen({e})")) for e in entries]
        pts = [(e, p) for e, p in pts if 40 < p[0] < cw - 40 and 40 < p[1] < ch - 40]
        if not pts:
            continue
        e, (ex, ey) = max(pts, key=lambda v: (v[1][0] - ax) ** 2 + (v[1][1] - ay) ** 2)
        pg.click(f"#strip{i}"); pg.keyboard.press("t")
        zoom_at(ex, ey, 10)
        ex, ey = app(pg, f"a.node_screen({e})")
        pg.mouse.click(ox + ex, oy + ey)
        if app(pg, "a.taxi_dest()") == e and app(pg, "a.preview_edges().length") > 0:
            pg.keyboard.press("Enter"); sent += 1
        else:
            pg.keyboard.press("Escape")
        zoom_at(ex, ey, -10)
    pg.keyboard.press(" ")
    app(pg, "a.fast_forward(100)"); frame(pg)
    return len(deps), sent

with sync_playwright() as pw:
    b = pw.chromium.launch(args=["--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=vulkan", "--enable-features=Vulkan"])
    for (state, w, h, dpr) in [("parked", 1500, 950, 1), ("parked", 1920, 1080, 1), ("parked", 1500, 950, 2), ("moving", 1500, 950, 1), ("moving", 1920, 1080, 1)]:
        ctx = b.new_context(viewport={"width": w, "height": h}, device_scale_factor=dpr)
        pg = ctx.new_page()
        errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.goto(PAGE + "?mode=surge&seed=1"); pg.wait_for_timeout(600)
        live = "Array.from({length: a.count()}, (_, i) => i).filter(i => !a.gone(i))"
        while app(pg, f"{live}.length") < 80:
            app(pg, "a.fast_forward(600)")
        n_live = app(pg, f"{live}.length")
        n_circ = app(pg, f"{live}.filter(i => a.phase(i).startsWith('GoAround') && a.strip_state(i).includes('final again')).length")
        moving_note = ""
        if state == "moving":
            n_dep, sent = set_moving(pg, live)
            moving_note = f"; {n_dep} departures on stand, {sent} sent taxiing, {app(pg, f'{live}.filter(i => {MOVING}).length')} moving at the start"
        res = pg.evaluate("""() => new Promise(done => {
          const a = TWR.app, orig = a.frame.bind(a), work = [], gaps = [];
          a.frame = (...args) => { const t = performance.now(); const r = orig(...args); work.push(performance.now() - t); return r; };
          let last = null, k = 0;
          function f(now) {
            if (last !== null) gaps.push(now - last);
            last = now;
            if (++k < 600) requestAnimationFrame(f); else { a.frame = orig; done({work, gaps}); }
          }
          requestAnimationFrame(f);
        })""")
        wk, gp = res["work"], res["gaps"]
        print(f"{state} {w}x{h} dpr {dpr}: {n_live} aircraft in play ({n_live - n_circ} drawn, {n_circ} out on the circuit){moving_note}, {len(gp)} frames")
        if state == "moving":
            print(f"  moving at the end: {app(pg, f'{live}.filter(i => {MOVING}).length')}; incidents {app(pg, 'a.incident_count()')}")
        print(f"  app.frame (sim + canvas): median {statistics.median(wk):.2f} ms, p95 {pct(wk, 95):.2f}, max {max(wk):.2f}")
        print(f"  whole frame (rAF gap):    median {statistics.median(gp):.2f} ms, p95 {pct(gp, 95):.2f}, max {max(gp):.2f}; frames over 20 ms: {sum(g > 20 for g in gp)}")
        print(f"  page errors: {errs}")
        ctx.close()
    b.close()
