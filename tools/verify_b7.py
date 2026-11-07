"""B7 verification: the score panel, the result dialog, personal bests and
the v1 -> v2 storage migration, driven from the page's own controls.

`fast_forward` runs the sim's own ticks to reach the end of a shift; every
other action is a real click or key. Pixel contrast is read from screenshots.
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
    while not app(pg, "a.over()"):
        app(pg, "a.fast_forward(6000)")
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
    pg.select_option("#mode", "normal"); pg.fill("#seed-in", "3"); pg.click("#start"); frame(pg)
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
    check(s and s["v"] == 2 and s["bests"]["normal"]["movements"]["n"] == app(pg, "a.movements()") and s["bests"]["normal"]["movements"]["seed"] == 3,
          f"bests saved under twr.v1, v2: {json.dumps(s['bests']['normal']['movements']) if s else None}")
    check("new best" in rows[0], "first shift is a new best")
    import datetime
    check(s["bests"]["normal"]["movements"]["date"] == datetime.date.today().isoformat(), f"date is the local date: {s['bests']['normal']['movements']['date']}")
    check("needs 10 movements" in rows[2] and "incidents" not in s["bests"]["normal"], "an idle shift sets no incidents record")

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
    pg.keyboard.press("Escape"); frame(pg)
    check(not pg.is_visible("#result") and app(pg, "a.over()"), "Esc closes the dialog; the shift stays over")
    app(pg, "a.fast_forward(500)"); frame(pg)
    check(app(pg, "a.ticks()") == tick_end, "nothing moves after the end")

    # Again: same seed, fresh shift. New seed: a different one.
    pg.click("#start"); to_end(pg)
    rows2 = pg.locator("#res-rows tr").all_inner_texts()
    check("new best" not in rows2[0], "same seed, same movements: not a new best")
    pg.click("text=Again · same seed"); frame(pg)
    check(not app(pg, "a.over()") and app(pg, "a.seed()") == 3 and app(pg, "a.real_s()") < 2 and app(pg, "a.movements()") == 0, "Again restarts seed 3 from zero")
    to_end(pg)
    pg.click("text=New seed"); frame(pg)
    check(app(pg, "a.seed()") != 3 and not app(pg, "a.over()") and re.search(r"seed=\d+$", pg.url), f"New seed: {app(pg, 'a.seed()')}")

    # Bests survive a reload.
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    check(pg.evaluate("TWR.state.bests.normal.movements.seed") == 3, "bests survive a reload")

    # ---- storage: an old v1 copy migrates; a newer one is refused -----------
    pg.evaluate("localStorage.setItem('twr.v1', JSON.stringify({v: 1, settings: {theme: 'light'}}))")
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    st = pg.evaluate("TWR.state")
    check(st["v"] == 2 and st["settings"]["theme"] == "light" and st["bests"] == {} and pg.get_attribute("html", "data-theme") == "light",
          f"v1 (light) loads as v2, theme kept, bests empty: {st}")
    pg.evaluate("document.getElementById('theme').click()"); frame(pg)
    check(stored(pg)["v"] == 2 and stored(pg)["bests"] == {}, "the next save writes v2")
    v3 = json.dumps({"v": 3, "settings": {"theme": "dark"}, "future": True})
    pg.evaluate(f"localStorage.setItem('twr.v1', {json.dumps(v3)})")
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    banner = pg.text_content("#banner")
    pg.evaluate("document.getElementById('theme').click()"); frame(pg)
    check("newer twr" in banner and pg.evaluate("localStorage.getItem('twr.v1')") == v3, f"v3 refused with a banner and left untouched: {banner!r}")
    errs = [e for e in errs if "newer twr" not in e]

    check(not errs, f"no console errors ({errs[:3]})")
    b.close()

n = sum(ok for ok, _ in results)
print(f"\n{n}/{len(results)} passed")
sys.exit(0 if n == len(results) else 1)
