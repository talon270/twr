"""C2 verification: tail direction on pushback. Real clicks and keys;
`fast_forward` only moves time, hooks only read.

  1. P opens a picker: one button and one arrow per tail direction
  2. <- and -> cycle the choice; Enter sends it; Esc cancels
  3. the push ends facing back: a taxi planned from there never uses the
     edge the tail went down (the U-turn), and a taxi onward is accepted
  4. a digit key and a button send their own direction
  5. S: push with a chosen tail, then the taxi builder opens by itself
  6. both themes

Run: python3 tools/verify_c2.py <screenshot dir>
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

def ff_until(pg, cond, max_ticks=100_000, chunk=100):
    t = 0
    while not app(pg, cond) and t < max_ticks:
        app(pg, f"a.fast_forward({chunk})"); t += chunk
    frame(pg)
    return app(pg, cond)

def buttons(pg):
    return pg.evaluate("[...document.querySelectorAll('#sel-verbs button')].map(b => b.dataset.key + ':' + b.textContent.slice(1))")

SEED = "7"

def start(pg):
    """Reload and start Practice on a fixed seed: the stand IGO512 sits on, and so its tail options, depend on it."""
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    pg.select_option("#mode", "practice"); pg.fill("#seed-in", SEED); pg.click("#start"); frame(pg)

def fresh(pw, theme="dark"):
    b = pw.chromium.launch()
    ctx = b.new_context(viewport={"width": 1500, "height": 950}, color_scheme=theme)
    pg = ctx.new_page()
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    return b, pg, errs

with sync_playwright() as pw:
    b, pg, errs = fresh(pw)
    box = pg.locator("#map").bounding_box()
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]

    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)

    # ---- 1 and 2: the picker --------------------------------------------------
    start(pg)
    ax, ay = app(pg, "a.aircraft_screen(0)")
    zoom_at(ax, ay, 10)
    pg.click("#strip0"); frame(pg)
    pg.keyboard.press("p"); frame(pg)
    hint = pg.text_content("#sel-hint")
    btn = buttons(pg)
    check(app(pg, "a.phase(0)") == "OnStand" and hint.startswith("← → choose"), f"P opens the picker, nothing pushed yet: {hint!r}")
    check(len(btn) >= 2 and all(x.split(":")[1].startswith("tail ") for x in btn), f"one button per direction: {btn}")
    pg.screenshot(path=str(OUT / "c2_picker_dark.png"))
    first = hint
    pg.keyboard.press("ArrowRight"); frame(pg)
    second = pg.text_content("#sel-hint")
    check(second != first and second.startswith("← → choose"), f"→ moves the choice: {first!r} then {second!r}")
    pg.keyboard.press("ArrowLeft"); frame(pg)
    check(pg.text_content("#sel-hint") == first, "← moves it back")
    pg.keyboard.press("Escape"); frame(pg)
    check(app(pg, "a.phase(0)") == "OnStand" and pg.text_content("#sel-hint").startswith("P push"), "Esc cancels: still on stand, P offered again")

    # ---- 3: Enter pushes the chosen tail; the U-turn is never offered ------------
    pg.keyboard.press("p"); pg.keyboard.press("ArrowRight"); frame(pg)
    chosen = pg.text_content("#sel-hint").split("Enter push ")[1].split(" ·")[0]
    pg.keyboard.press("Enter"); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback", f"Enter sends {chosen!r}: Pushback")
    check(ff_until(pg, "a.phase(0) === 'Holding'"), "push ends Holding")
    trail = app(pg, "Array.from(a.trail(0))")
    check(len(trail) >= 2, f"it went down the link and along a lane: {len(trail)} edges")
    # The builder opens on T; pick a runway entry far away, as verify_b3 does.
    pg.keyboard.press("t"); frame(pg)
    zoom_at(cw / 2, ch / 2, -12)
    entries = app(pg, "Array.from(a.departure_entries(2500))")
    ax, ay = app(pg, "a.aircraft_screen(0)")
    pts = [(n, app(pg, f"a.node_screen({n})")) for n in entries]
    pts = [(n, p) for n, p in pts if 40 < p[0] < cw - 40 and 40 < p[1] < ch - 40]
    n, (ex, ey) = max(pts, key=lambda v: (v[1][0] - ax) ** 2 + (v[1][1] - ay) ** 2)
    zoom_at(ex, ey, 12)
    ex, ey = app(pg, f"a.node_screen({n})")
    pg.mouse.click(ox + ex, oy + ey); frame(pg)
    check(app(pg, "a.taxi_dest()") == n, f"the click picked runway entry {n} (got {app(pg, 'a.taxi_dest()')}): {pg.text_content('#sel-hint')!r}")
    prev = app(pg, "Array.from(a.preview_edges())")
    check(len(prev) > 0 and trail[-1] not in prev, f"the preview ({len(prev)} edges) does not use the edge the tail went down ({trail[-1]})")
    pg.keyboard.press("Enter"); frame(pg)
    check(app(pg, "a.phase(0)") == "Taxi", "Enter sends the taxi: " + pg.text_content("#state0"))
    pg.screenshot(path=str(OUT / "c2_taxi_after_tail_dark.png"))

    # ---- 4: a digit key and a button -----------------------------------------------
    start(pg)
    pg.click("#strip0"); pg.keyboard.press("p"); frame(pg)
    n_opts = len(buttons(pg))
    pg.keyboard.press(str(n_opts)); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback", f"digit {n_opts} sends the last direction")
    ff_until(pg, "a.phase(0) === 'Holding'")
    trail_last = app(pg, "Array.from(a.trail(0))")
    start(pg)
    pg.click("#strip0"); pg.keyboard.press("p"); frame(pg)
    pg.click("#sel-verbs button[data-key='1']"); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback", "the button 1 sends the first direction")
    ff_until(pg, "a.phase(0) === 'Holding'")
    trail_first = app(pg, "Array.from(a.trail(0))")
    check(trail_first != trail_last, f"first and last direction push down different lanes: {trail_first[-1]} vs {trail_last[-1]}")

    # ---- 5: S, a chosen tail, then the builder opens ----------------------------------
    start(pg)
    pg.click("#strip0"); pg.keyboard.press("s"); frame(pg)
    check(app(pg, "a.phase(0)") == "OnStand" and pg.text_content("#sel-hint").startswith("← → choose"), "S opens the same picker")
    pg.keyboard.press("Enter"); frame(pg)
    check(ff_until(pg, "a.phase(0) === 'Holding'") and pg.text_content("#sel-hint").startswith("Click a stand"),
          f"push ends and the taxi builder opens: {pg.text_content('#sel-hint')!r}")

    # ---- 6: light theme ------------------------------------------------------------------
    start(pg)
    pg.click("#theme"); frame(pg)
    ax, ay = app(pg, "a.aircraft_screen(0)")
    zoom_at(ax, ay, 10)
    pg.click("#strip0"); pg.keyboard.press("p"); frame(pg)
    pg.screenshot(path=str(OUT / "c2_picker_light.png"))
    check(not errs, f"no page errors: {errs}")
    b.close()

bad = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(bad)} PASS, {len(bad)} FAIL")
sys.exit(1 if bad else 0)
