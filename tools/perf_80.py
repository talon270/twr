"""Frame time with 80+ aircraft in play (PLAN-twr.md § B9).

Two states: "parked", as measured early (most aircraft wait on stands), and
"moving" (B9): real keys and clicks push every departure on a stand with the
clock paused, and the frames are timed while they push, so 60 or so move at
once. The ground conflict check grows with moving aircraft, and a pushback
is a mover in it.

Why not taxi them on to the runway, as B9 first did: Surge ends on its third
incident (score.rs SURGE_INCIDENTS), and 62 uncontrolled pushes give three
GROUND incidents before they reach Holding (seed 1, 2026-11-10). The ended
world stops moving, and its result dialog then took the taxi loop's Enter
and restarted an empty world. Peak cannot stand in: it reaches 37 aircraft,
not 80, before its 30 minutes run out. A timing taken after the shift ended
is printed as INVALID, never as a number.

Surge with nobody at the controls fills the airport: departures wait on
stands, arrivals circle. `app.frame` is wrapped to time the sim step plus the
canvas draw; rAF intervals give the whole frame, strips included. The launch
flags put headless Chromium on the real GPU; without them it draws in
software (SwiftShader), and at 2x pixel density every frame then takes
about 50 ms. That is a property of the test rig, not of the page.

The "-lvp" states (PLAN-lvp.md step 9) time the same thing with the LSA
outline drawn. Scored modes take fog from the seed only (`App.set_lvp`
refuses outside Practice), and Surge seed 58 is the first that draws it, so
those states run a different world from seed 1: compare their medians as
"outline drawn", not as a same-world A/B.
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

# The first Surge seed whose shift draws LVP (World::shift(58, Surge).lvp(),
# probed 2026-11-11; seeds 61, 110, 183, 213 follow).
LVP_SEED = 58

MOVING = "['Pushback', 'Taxi'].includes(a.phase(i))"

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def set_moving(pg, live):
    """Pause, push every departure on a stand, unpause."""
    deps = app(pg, f"{live}.filter(i => a.phase(i) === 'OnStand' && / · dep /.test(a.strip_state(i)))")
    pg.keyboard.press(" ")
    for i in deps:
        # Enter takes the first tail option where the stand offers a choice.
        pg.click(f"#strip{i}"); pg.keyboard.press("p"); pg.keyboard.press("Enter")
    pg.keyboard.press(" ")
    frame(pg)
    return len(deps), sum(app(pg, f"a.phase({i})") == "Pushback" for i in deps)

with sync_playwright() as pw:
    b = pw.chromium.launch(args=["--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=vulkan", "--enable-features=Vulkan"])
    for (state, w, h, dpr) in [("parked", 1500, 950, 1), ("parked", 1920, 1080, 1), ("parked", 1500, 950, 2), ("moving", 1500, 950, 1), ("moving", 1920, 1080, 1),
                               ("parked-lvp", 1500, 950, 1), ("parked-lvp", 1920, 1080, 1), ("moving-lvp", 1500, 950, 1)]:
        ctx = b.new_context(viewport={"width": w, "height": h}, device_scale_factor=dpr)
        pg = ctx.new_page()
        errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        lvp = state.endswith("-lvp")
        pg.goto(PAGE + f"?mode=surge&seed={LVP_SEED if lvp else 1}"); pg.wait_for_timeout(600)
        tag = pg.evaluate("!document.getElementById('lvp-tag').hidden")
        if app(pg, "a.lvp()") != lvp or tag != lvp:
            print(f"{state}: INVALID, LVP is {app(pg, 'a.lvp()')} and the tag shows {tag}, expected {lvp}")
            ctx.close()
            continue
        live = "Array.from({length: a.count()}, (_, i) => i).filter(i => !a.gone(i))"
        while app(pg, f"{live}.length") < 80 and not app(pg, "a.over()"):
            app(pg, "a.fast_forward(600)")
        n_live = app(pg, f"{live}.length")
        n_circ = app(pg, f"{live}.filter(i => a.phase(i).startsWith('GoAround') && a.strip_state(i).includes('final again')).length")
        moving_note = ""
        if state.startswith("moving"):
            n_dep, sent = set_moving(pg, live)
            moving_note = f"; {n_dep} departures on stand, {sent} pushing, {app(pg, f'{live}.filter(i => {MOVING}).length')} moving at the start"
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
        if state.startswith("moving"):
            print(f"  moving at the end: {app(pg, f'{live}.filter(i => {MOVING}).length')}; incidents {app(pg, 'a.incident_count()')}")
        if app(pg, "a.over()"):
            print("  INVALID: the Surge ended during the timing, so the world stopped moving")
        else:
            print(f"  app.frame (sim + canvas): median {statistics.median(wk):.2f} ms, p95 {pct(wk, 95):.2f}, max {max(wk):.2f}")
            print(f"  whole frame (rAF gap):    median {statistics.median(gp):.2f} ms, p95 {pct(gp, 95):.2f}, max {max(gp):.2f}; frames over 20 ms: {sum(g > 20 for g in gp)}")
        print(f"  page errors: {errs}")
        ctx.close()
    b.close()
