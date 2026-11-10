"""PLAN-ui-console step 2: the selected-aircraft and conflict panel under the map.

Run: python3 tools/verify_ui2.py <shots-folder>. Map, panel and radio line stack
with no gap; the panel fills from a strip click; its verbs work; neither the
canvas nor the selected strip changes height; 44 px buttons; both themes."""
import pathlib, sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1]); OUT.mkdir(parents=True, exist_ok=True)
res = []
def check(ok, msg): res.append(ok); print(("PASS " if ok else "FAIL ") + msg)
def frame(pg): pg.wait_for_timeout(80)
def app(pg, js): return pg.evaluate(f"(() => {{ const a = TWR.app; return {js}; }})()")

with sync_playwright() as pw:
    b = pw.chromium.launch()
    for theme, w, h in (("dark", 1500, 950), ("light", 1920, 1080)):
        pg = b.new_page(viewport={"width": w, "height": h})
        errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
        pg.goto(PAGE + "?mode=practice&seed=7"); pg.wait_for_timeout(700); frame(pg)
        if pg.get_attribute("html", "data-theme") != theme: pg.click("#theme"); frame(pg)
        mb, cb, rb = (pg.locator(s).bounding_box() for s in ("#map", "#ctx", "#radio"))
        sb = pg.locator("#strips").bounding_box()
        check(abs(mb["y"] + mb["height"] - cb["y"]) < 1 and abs(cb["y"] + cb["height"] - rb["y"]) < 1,
              f"[{theme} {w}] map {mb['height']:.0f} px tall meets panel at {cb['y']:.0f}; panel {cb['height']:.0f} px meets radio at {rb['y']:.0f}")
        check(abs(cb["width"] - mb["width"]) < 1 and abs(sb["x"] - (cb["x"] + cb["width"])) < 2,
              f"[{theme} {w}] panel spans the map column ({cb['width']:.0f} vs {mb['width']:.0f}), rack starts at its edge ({sb['x']:.0f})")
        gap = pg.evaluate("innerWidth - Math.max(...[...document.querySelectorAll('main > *')].filter(e => e.offsetParent || e.tagName === 'CANVAS').map(e => e.getBoundingClientRect().right))")
        check(gap <= 16, f"[{theme} {w}] {gap:.0f} px empty at the right")
        # Strip click selects; panel fills; strip itself grows no verbs.
        h0 = pg.evaluate("document.getElementById('map').getBoundingClientRect().height")
        sh0 = pg.evaluate("document.getElementById('strip0').getBoundingClientRect().height")
        pg.click("#strip0"); frame(pg)
        cs = pg.text_content("#sel-cs"); hint = pg.text_content("#sel-hint")
        check(cs == app(pg, "a.callsign(0)") and hint.startswith("P push"), f"[{theme} {w}] strip0 click fills panel: {cs} / {hint!r}")
        keys = pg.eval_on_selector_all("#sel-verbs button", "bs => bs.map(b => b.dataset.key).join('')")
        check("P" in keys, f"[{theme} {w}] panel verbs: {keys}")
        sh1 = pg.evaluate("document.getElementById('strip0').getBoundingClientRect().height")
        check(abs(sh1 - sh0) < 1, f"[{theme} {w}] selected strip keeps its height ({sh0:.0f} -> {sh1:.0f})")
        pg.click("#sel-verbs button[data-key=P]"); frame(pg)
        check(pg.text_content("#sel-hint").startswith("← → choose") and pg.evaluate("document.activeElement === document.body"),
              f"[{theme} {w}] panel P opens tail picker, focus stays off the button: {pg.text_content('#sel-hint')[:40]!r}")
        pg.keyboard.press("Escape"); frame(pg)
        h1 = pg.evaluate("document.getElementById('map').getBoundingClientRect().height")
        check(abs(h1 - h0) < 0.5, f"[{theme} {w}] canvas height unchanged by the panel's content ({h0:.1f} -> {h1:.1f})")
        hs = pg.eval_on_selector_all("#ctx button", "bs => bs.map(b => Math.round(b.getBoundingClientRect().height))")
        check(all(x >= 44 for x in hs), f"[{theme} {w}] panel buttons 44 px or more: {hs}")
        check(pg.text_content("#conflict-none") == "No live conflict" and pg.is_visible("#conflict-none"), f"[{theme} {w}] no pair: panel says so")
        pg.screenshot(path=str(OUT / f"panel-{theme}-{w}.png"))
        check(not errs, f"[{theme} {w}] zero console errors {errs[:2]}")
        pg.close()

    # A live conflict at Peak: run time forward until one appears.
    pg = b.new_page(viewport={"width": 1500, "height": 950})
    errs = []
    pg.on("pageerror", lambda e: errs.append(str(e)))
    pg.goto(PAGE + "?mode=peak&seed=7"); pg.wait_for_timeout(700); frame(pg)
    n = 0
    for t in range(400):
        app(pg, "a.fast_forward(50)"); frame(pg)
        n = pg.evaluate("document.querySelectorAll('#conflict-list button').length")
        if n: break
    if n:
        txt = pg.text_content("#conflict-list button")
        border = pg.evaluate("getComputedStyle(document.getElementById('conflicts')).boxShadow")
        check("+" in txt and " m" in txt and pg.is_hidden("#conflict-none"), f"conflict row after {t * 50} ticks: {txt!r}, inset {border}")
        i = pg.get_attribute("#conflict-list button", "data-i")
        pg.click("#conflict-list button"); frame(pg)
        check(pg.get_attribute("#sel", "data-i") == i, f"conflict row selects aircraft {i}: {pg.text_content('#sel-cs')}")
        pg.screenshot(path=str(OUT / "panel-conflict-dark-1500.png"))
    else:
        print("NOTE no conflict appeared in 20000 ticks; conflict row unverified")
    check(not errs, f"peak run: zero page errors {errs[:2]}")
    b.close()
print(f"{sum(res)} PASS, {len(res) - sum(res)} FAIL")
