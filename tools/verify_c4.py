"""C4 verification: the runway picker. Real clicks; `fast_forward` only moves
time, hooks only read.

  1. Practice has no traffic: the picker is hidden and the note says so
  2. a shift shows the direction in force and a toggle per end
  3. Land and Depart toggles flip, and the last one of a kind refuses inline
  4. a switch of direction says it is waiting, then takes over
  5. too much tailwind refuses the switch, in words, and nothing waits
  6. both themes

Run: python3 tools/verify_c4.py <screenshot dir>
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

def start(pg, mode, seed):
    pg.goto(PAGE); pg.wait_for_timeout(500); frame(pg)
    pg.select_option("#mode", mode); pg.fill("#seed-in", str(seed)); pg.click("#start"); frame(pg)

def pressed(pg, role):
    sel = "#rw-land" if role == "land" else "#rw-dep"
    return pg.evaluate(f"[...document.querySelectorAll('{sel} button')].filter(b => b.getAttribute('aria-pressed') === 'true').map(b => b.dataset.end)")

def ends(pg):
    return pg.evaluate("[...document.querySelectorAll('#rw-land button')].map(b => b.dataset.end)")

def note(pg):
    return pg.text_content("#rw-note")

def flip(pg, flow):
    """Press the direction button; return the refusal text, '' if none."""
    pg.click(f'#rw-dir button[data-flow="{flow}"]'); frame(pg)

with sync_playwright() as pw:
    b = pw.chromium.launch()
    ctx = b.new_context(viewport={"width": 1500, "height": 950}, color_scheme="dark")
    pg = ctx.new_page()
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))

    # ---- 1: Practice -------------------------------------------------------------
    pg.goto(PAGE); pg.wait_for_timeout(500); frame(pg)
    check(not pg.is_visible("#rw-dir") and not pg.is_visible("#rw-land") and "Practice" in note(pg), f"Practice hides the picker; note: {note(pg)!r}")

    # ---- find a seed whose wind allows either direction -----------------------------
    seed = None
    for s in range(1, 60):
        start(pg, "normal", s)
        cur = app(pg, "a.flow()")
        other = "east" if cur == "west" else "west"
        if app(pg, f"a.runway_direction('{other}')") == "":
            app(pg, f"a.runway_direction('{cur}')")  # cancel the wait
            seed = s; break
    check(seed is not None, f"seed {seed}: wind {app(pg, 'a.wind_text()')} lets either direction run")
    start(pg, "normal", seed)

    # ---- 2: the picker on a shift -------------------------------------------------------
    cur = app(pg, "a.flow()")
    check(pg.is_visible("#rw-dir") and pg.get_attribute(f'#rw-dir button[data-flow="{cur}"]', "aria-pressed") == "true", f"direction in force ({cur}) is pressed")
    check(len(ends(pg)) == 4 and pressed(pg, "land") == (["29L", "28"] if cur == "west" else ["11R", "10"]), f"four ends; land pressed {pressed(pg, 'land')}")
    check(pressed(pg, "depart") == (["29R", "27"] if cur == "west" else ["11L", "09"]), f"depart pressed {pressed(pg, 'depart')}")
    pg.screenshot(path=str(OUT / "c4_picker_dark.png"))

    # ---- 3: toggles ---------------------------------------------------------------------------
    mixed = "29R" if cur == "west" else "11L"
    pg.click(f'#rw-land button[data-end="{mixed}"]'); frame(pg)
    check(mixed in pressed(pg, "land") and app(pg, "a.runways_text()").startswith("land") and mixed in app(pg, "a.runways_text()").split("·")[0], f"{mixed} now lands too: {app(pg, 'a.runways_text()')}")
    pg.click(f'#rw-land button[data-end="{mixed}"]'); frame(pg)
    check(mixed not in pressed(pg, "land"), f"pressed again: {mixed} off")
    # Remove departures until one is left, then the last refuses.
    dep = pressed(pg, "depart")
    pg.click(f'#rw-dep button[data-end="{dep[0]}"]'); frame(pg)
    check(pressed(pg, "depart") == dep[1:], f"{dep[0]} off: {pressed(pg, 'depart')}")
    pg.click(f'#rw-dep button[data-end="{dep[1]}"]'); frame(pg)
    check(pressed(pg, "depart") == dep[1:] and "keep at least one departure runway" in note(pg), f"the last departure end refuses inline: {note(pg)!r}")
    pg.click(f'#rw-dep button[data-end="{dep[0]}"]'); frame(pg)
    check("keep at least" not in note(pg), "the next press clears the refusal")

    # ---- 4: a switch waits, then takes over -----------------------------------------------------
    other = "east" if cur == "west" else "west"
    ff = 0
    while app(pg, "a.count()") < 6 and ff < 60_000:
        app(pg, "a.fast_forward(200)"); ff += 200
    frame(pg)
    flip(pg, other)
    lab = pg.text_content(f'#rw-dir button[data-flow="{other}"]')
    check("(waiting)" in lab and "switching to " + other in note(pg), f"waiting: button {lab!r}; note {note(pg)!r}")
    check(app(pg, "a.flow()") == cur, f"{cur} still in force while it waits")
    pg.screenshot(path=str(OUT / "c4_waiting_dark.png"))
    # Nobody flies these aircraft in this page, so the wait does not end by
    # itself: that is the player's job, and the note keeps saying what it waits for.
    app(pg, "a.fast_forward(4000)"); frame(pg)
    check(app(pg, "a.flow()") == cur and "switching to " + other in note(pg), f"still waiting 200 game s later, note kept: {note(pg)!r}")
    flip(pg, cur)
    check("(waiting)" not in pg.text_content(f'#rw-dir button[data-flow="{other}"]') and "switching" not in note(pg), f"pressing the direction in force cancels the wait: {note(pg)!r}")
    # A quiet airport has nothing on the old runways: the switch takes over at once.
    start(pg, "normal", seed)
    flip(pg, other)
    app(pg, "a.fast_forward(10)"); frame(pg)
    check(app(pg, "a.flow()") == other, f"nothing on {cur} runways: {other} in force at once")
    check("(waiting)" not in pg.text_content(f'#rw-dir button[data-flow="{other}"]') and "switching" not in note(pg), f"no waiting text left: {note(pg)!r}")
    want = ["11R", "10"] if other == "east" else ["29L", "28"]
    check(pressed(pg, "land") == want and set(ends(pg)) == ({"11R", "10", "11L", "09"} if other == "east" else {"29L", "28", "29R", "27"}), f"picker shows {other} ends, land {pressed(pg, 'land')}")
    check(not errs, f"no page errors so far: {errs}")

    # ---- 5: tailwind refuses a switch ---------------------------------------------------------------
    bad = None
    for s in range(1, 120):
        start(pg, "normal", s)
        cur2 = app(pg, "a.flow()")
        o2 = "east" if cur2 == "west" else "west"
        msg = app(pg, f"a.runway_direction('{o2}')")
        if msg:
            bad = (s, cur2, o2, msg); break
    check(bad is not None, f"a seed whose wind refuses the other direction: {bad}")
    if bad:
        s, cur2, o2, msg = bad
        start(pg, "normal", s)
        flip(pg, o2)
        check("tailwind" in note(pg) and "limit 10" in note(pg), f"the refusal reads: {note(pg)!r}")
        check(app(pg, "a.flow()") == cur2 and "(waiting)" not in pg.text_content(f'#rw-dir button[data-flow="{o2}"]'), "nothing waits after a refusal")
        pg.screenshot(path=str(OUT / "c4_tailwind_dark.png"))

    # ---- 6: light ---------------------------------------------------------------------------------------
    start(pg, "normal", seed)
    pg.click("#theme"); frame(pg)
    pg.screenshot(path=str(OUT / "c4_picker_light.png"))
    check(not errs, f"no page errors: {errs}")
    b.close()

bad = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(bad)} PASS, {len(bad)} FAIL")
sys.exit(1 if bad else 0)
