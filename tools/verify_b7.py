"""B7 verification: the score panel, the result dialog, personal bests and
the v1 -> v2 -> v3 storage migrations, driven from the page's own controls.

`fast_forward` runs the sim's own ticks, and `select` + `key('c')` clear
arrivals to land so a shift passes 10 movements; End shift and every other
action is a real click or key. Pixel contrast is read from screenshots.
"""
from __future__ import annotations
import io, json, pathlib, re, sys
from PIL import Image
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
SHIFT_TICKS = 30 * 60 * 60
results: list[tuple[bool, str]] = []

def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, e):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {e}; }})()")

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def to_end(pg):
    # No mode has a time limit (audit 2026-11-13, A1). Paused, so frames add
    # no ticks and two runs of one seed match. Every arrival that asks to
    # land is cleared; departures are left alone. Normal seed 1 then makes 12
    # movements by 30:00, past the 10 a movements best needs (seed 3 makes 7;
    # run 2026-11-15). The last jump lands on 30:00 exactly, then End shift,
    # clicked.
    if not pg.is_visible("#paused"):
        pg.keyboard.press(" "); frame(pg)
    while app(pg, "a.real_s()") < 1800:
        app(pg, "a.fast_forward(Math.min(600, (1800 - a.real_s()) * 60))")
        app(pg, "(() => { for (let i = 0; i < a.count(); i++) if (a.verbs(i).includes('C')) { a.select(i); a.key('c'); } return 0; })()")
    pg.click("#end-shift")
    frame(pg); frame(pg)

def lum(rgb):
    def ch(c):
        c /= 255
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (ch(x) for x in rgb[:3])
    return 0.2126 * r + 0.7152 * g + 0.0722 * b

def ratio(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)

def stored(pg):
    raw = pg.evaluate("localStorage.getItem('twr.v1')")
    return json.loads(raw) if raw else None

with sync_playwright() as pw:
    b = pw.chromium.launch()
    ctx = b.new_context(viewport={"width": 1500, "height": 950})
    pg = ctx.new_page()
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)

    check(not pg.is_visible("#score"), "Practice: no score panel")

    # ---- a Normal shift to its end -----------------------------------------
    pg.select_option("#mode", "normal"); pg.fill("#seed-in", "1"); pg.click("#start"); frame(pg)
    check(pg.is_visible("#score") and pg.text_content("#sc-mov") == "0", "Normal: score panel shows, movements 0")
    app(pg, "a.fast_forward(20000)")
    # Pause (Space) so the sim does not move between the two reads.
    pg.keyboard.press(" "); frame(pg); frame(pg)
    shown = (pg.text_content("#sc-mov"), pg.text_content("#sc-off"), pg.text_content("#sc-delay"), pg.text_content("#sc-inc"))
    hooks = (str(app(pg, "a.movements()")), str(app(pg, "a.offered()")), f"{app(pg, 'a.delay_min()'):.1f}", str(app(pg, "a.incident_count()")))
    check(shown == hooks, f"panel matches the sim mid-shift: {shown}")
    pg.keyboard.press(" ")
    to_end(pg)
    tick_end = app(pg, "a.ticks()")
    check(app(pg, "a.real_s()") == 1800 and pg.is_visible("#result"), f"at 30:00 the result dialog opens ({app(pg, 'a.real_s()')} real s)")
    rows = pg.locator("#res-rows tr").all_inner_texts()
    print("  " + "\n  ".join(r.replace("\t", " | ") for r in rows))
    check(pg.text_content("#res-title") == "Shift over" and len(rows) == 4, "title and four rows")
    mv = f"{app(pg, 'a.movements()')} of {app(pg, 'a.offered()')} offered"
    check(mv in rows[0] and f"{app(pg, 'a.delay_min()'):.1f} game min" in rows[1], f"rows match the sim: {mv}")
    s = stored(pg)
    n = app(pg, "a.movements()")
    m = s and s["bests"]["normal"].get("movements")
    check(s and s["v"] == 3 and n >= 10 and m and m["n"] == n and m["seed"] == 1 and m["s"] == 1800 and abs(m["perh"] - n * 2) < 1e-9,
          f"bests saved under twr.v1, v3, as a rate: {json.dumps(m) if s else None}")
    check(f"{n * 2}/h over 30:00" in rows[0] and f"{n * 2}/h · {n} in 30:00" in rows[0], f"the movements row prints the rate, its count and its length: {rows[0]!r}")
    focused = pg.evaluate("document.activeElement.id")
    check(focused == "res-title", f"A3: first focus is the title, not Again ({focused!r})")
    check("new best" in rows[0], "first shift is a new best")
    import datetime
    check(s["bests"]["normal"]["movements"]["date"] == datetime.date.today().isoformat(), f"date is the local date: {s['bests']['normal']['movements']['date']}")
    check("incidents" in s["bests"]["normal"] and "delay" in s["bests"]["normal"], "past 10 movements, delay and incidents records are set")
    pg.keyboard.press("Enter"); frame(pg)
    check(pg.is_visible("#result") and app(pg, "a.over()"), "A3: Enter on the open dialog does not restart the shift")

    # Figure-ground: the dialog surface against the dimmed page, both themes.
    for theme in ("dark", "light"):
        if pg.get_attribute("html", "data-theme") != theme:
            pg.evaluate("document.getElementById('theme').click()"); frame(pg)
        img = Image.open(io.BytesIO(pg.screenshot())).convert("RGB")
        box = pg.locator("#result").bounding_box()
        surface = img.getpixel((int(box["x"] + 6), int(box["y"] + box["height"] - 6)))
        backdrop = img.getpixel((20, 120))
        r = ratio(surface, backdrop)
        check(r >= 2.5, f"{theme}: dialog {surface} on scrim {backdrop}: {r:.2f}:1 (needs 2.5)")
        pg.screenshot(path=str(OUT / f"b7_result_{theme}.png"))
    pg.evaluate("document.getElementById('theme').click()"); frame(pg)

    # Esc closes, the world stays stopped; keys do nothing while it is open.
    # PLAN-ui-console step 4: the review fades out over 215 ms and keeps
    # display:block until then; check once the exit has finished.
    pg.keyboard.press("Escape"); frame(pg); pg.wait_for_timeout(300)
    check(not pg.is_visible("#result") and app(pg, "a.over()"), "Esc closes the dialog; the shift stays over")
    app(pg, "a.fast_forward(500)"); frame(pg)
    check(app(pg, "a.ticks()") == tick_end, "nothing moves after the end")

    # Again: same seed, fresh shift. New seed: a different one.
    pg.click("#start"); to_end(pg)
    rows2 = pg.locator("#res-rows tr").all_inner_texts()
    n2 = app(pg, "a.movements()")
    check(("new best" in rows2[0]) == (n2 > n), f"second shift of seed 1: {n2} movements against {n}; a new best only if more")
    pg.click("text=Again · same seed"); frame(pg)
    check(not app(pg, "a.over()") and app(pg, "a.seed()") == 1 and app(pg, "a.real_s()") < 2 and app(pg, "a.movements()") == 0, "Again restarts seed 1 from zero")
    to_end(pg)
    pg.click("text=New seed"); frame(pg)
    check(app(pg, "a.seed()") != 1 and not app(pg, "a.over()") and re.search(r"seed=\d+$", pg.url), f"New seed: {app(pg, 'a.seed()')}")

    # Bests survive a reload.
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    check(pg.evaluate("TWR.state.bests.normal.movements.seed") == 1, "bests survive a reload")

    # ---- storage: an old v1 copy migrates; a newer one is refused -----------
    pg.evaluate("localStorage.setItem('twr.v1', JSON.stringify({v: 1, settings: {theme: 'light'}}))")
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    st = pg.evaluate("TWR.state")
    check(st["v"] == 3 and st["settings"]["theme"] == "light" and st["bests"] == {} and pg.get_attribute("html", "data-theme") == "light",
          f"v1 (light) loads as v3, theme kept, bests empty: {st}")
    pg.evaluate("document.getElementById('theme').click()"); frame(pg)
    check(stored(pg)["v"] == 3 and stored(pg)["bests"] == {}, "the next save writes v3")
    # v2 -> v3: a movements best outside Surge was a 30:00 shift, so it gains
    # s 1800 and its rate; Surge keeps its count, untouched.
    v2 = {"v": 2, "settings": {"theme": "dark"}, "bests": {
        "peak": {"movements": {"n": 43, "offered": 60, "seed": 7, "date": "2026-11-07"}, "delay": {"per": 0.5, "delay": 20, "movements": 40, "seed": 7, "date": "2026-11-07"}},
        "surge": {"movements": {"n": 12, "offered": 30, "seed": 1, "date": "2026-11-07"}, "held": {"s": 400, "movements": 12, "seed": 1, "date": "2026-11-07"}}}}
    pg.evaluate(f"localStorage.setItem('twr.v1', {json.dumps(json.dumps(v2))})")
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    st = pg.evaluate("TWR.state")
    pm = st["bests"]["peak"]["movements"]
    check(st["v"] == 3 and pm["s"] == 1800 and pm["perh"] == 86 and pm["n"] == 43 and st["bests"]["peak"]["delay"] == v2["bests"]["peak"]["delay"]
          and st["bests"]["surge"] == v2["bests"]["surge"] and pg.text_content("#banner") == "",
          f"v2 loads as v3: peak movements {pm}, surge untouched")
    # A8: a hand-edited mode or record that is not an object is dropped on load.
    bad = {"v": 3, "settings": {"theme": "dark"}, "bests": {"peak": "x", "normal": {"movements": "y", "delay": {"per": 1, "delay": 10, "movements": 10, "seed": 1, "date": "2026-11-07"}}}}
    pg.evaluate(f"localStorage.setItem('twr.v1', {json.dumps(json.dumps(bad))})")
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    st = pg.evaluate("TWR.state")
    pg.select_option("#mode", "peak"); pg.fill("#seed-in", "3"); pg.click("#start"); frame(pg)
    app(pg, "a.fast_forward(600)"); pg.click("#end-shift"); frame(pg); frame(pg)
    t0 = app(pg, "a.ticks()")
    pg.keyboard.press("Escape"); frame(pg); pg.wait_for_timeout(300)
    pg.select_option("#mode", "practice"); pg.click("#start"); frame(pg); pg.wait_for_timeout(300)
    check("peak" not in st["bests"] and "movements" not in st["bests"]["normal"] and "delay" in st["bests"]["normal"]
          and pg.is_visible("#result") is False and app(pg, "a.ticks()") > 0 and pg.text_content("#banner") == "",
          f"A8: bad entries dropped on load, the shift ends and the next one runs: {st['bests']}, ticks {app(pg, 'a.ticks()')}")
    v4 = json.dumps({"v": 4, "settings": {"theme": "dark"}, "future": True})
    pg.evaluate(f"localStorage.setItem('twr.v1', {json.dumps(v4)})")
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    banner = pg.text_content("#banner")
    pg.evaluate("document.getElementById('theme').click()"); frame(pg)
    check("newer twr" in banner and pg.evaluate("localStorage.getItem('twr.v1')") == v4, f"v4 refused with a banner and left untouched: {banner!r}")
    errs = [e for e in errs if "newer twr" not in e]

    check(not errs, f"no console errors ({errs[:3]})")
    b.close()

n = sum(ok for ok, _ in results)
print(f"\n{n}/{len(results)} passed")
sys.exit(0 if n == len(results) else 1)
