"""PLAN-ui-depth: controls with depth, from the 2026-11-15 UI probe.

Run: python3 tools/verify_depth.py <shots-folder> [page]. In both themes at
1500 and 1920: controls carry a 3 px ink border and a 5 px hard ink shadow,
a press travels 5 px into the shadow and drops it, reduced motion travels
0 px, disabled End shift is flat, bay boxes keep a 3 px hairline and no
shadow, strips stay flat, and the result title is mixed case. Screenshots of
Practice and of a Peak shift at 40+ aircraft for review by eye."""
import pathlib, sys, re
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").resolve().as_uri()
OUT = pathlib.Path(sys.argv[1]); OUT.mkdir(parents=True, exist_ok=True)
res = []
def check(ok, msg): res.append(ok); print(("PASS " if ok else "FAIL ") + msg, flush=True)
def app(pg, js): return pg.evaluate(f"(() => {{ const a = TWR.app; return {js}; }})()")
def css(pg, sel, p): return pg.evaluate("([s, p]) => { const e = document.querySelector(s); return e ? getComputedStyle(e)[p] : null; }", [sel, p])
def rgb(s): return tuple(int(x) for x in re.findall(r"\d+", s or "")[:3])
def shadow(s):
    # "rgb(255, 255, 255) 5px 5px 0px 0px" -> ((255,255,255), (5, 5, 0))
    m = re.match(r"(rgba?\([^)]*\))\s+(-?\d+)px\s+(-?\d+)px\s+(\d+)px", s or "")
    return (rgb(m.group(1)), tuple(int(m.group(i)) for i in (2, 3, 4))) if m else (None, None)
def box(pg, sel):
    b = pg.locator(sel).bounding_box(); return (round(b["x"], 1), round(b["y"], 1))

with sync_playwright() as pw:
    b = pw.chromium.launch()
    for theme, w, h in (("dark", 1500, 950), ("light", 1500, 950), ("dark", 1920, 1080), ("light", 1920, 1080)):
        pg = b.new_page(viewport={"width": w, "height": h})
        errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
        pg.goto(PAGE + "?mode=practice&seed=7"); pg.wait_for_timeout(700)
        if pg.get_attribute("html", "data-theme") != theme: pg.click("#theme"); pg.wait_for_timeout(300)
        tag = f"[{theme} {w}]"
        ink = rgb(css(pg, "html", "color"))

        for sel in ("#start", "#theme", "#mode", "#seed-in", "#lvp-btn", "#radio-send"):
            col, off = shadow(css(pg, sel, "boxShadow"))
            bw = css(pg, sel, "borderTopWidth")
            check(col == ink and off == (5, 5, 0) and bw == "3px", f"{tag} {sel}: 3 px border ({bw}), hard ink shadow {col} {off}")

        col, off = shadow(css(pg, "#end-shift", "boxShadow"))
        check(pg.is_disabled("#end-shift") and css(pg, "#end-shift", "boxShadow") == "none", f"{tag} disabled End shift is flat ({css(pg, '#end-shift', 'boxShadow')})")
        check(css(pg, "#bay .baybox", "borderTopWidth") == "3px" and css(pg, "#bay .baybox", "boxShadow") == "none", f"{tag} bay box: 3 px border, no shadow")
        check(css(pg, ".bayhead", "boxShadow") == "none", f"{tag} bay header: no shadow")
        strips = pg.evaluate("[...document.querySelectorAll('.strip')].filter(e => e.offsetParent).map(e => getComputedStyle(e).boxShadow)")
        check(len(strips) > 0 and all(s == "none" for s in strips), f"{tag} {len(strips)} strips flat")

        # Press: hold the mouse down on Start; it travels 5 px and drops its shadow.
        x0, y0 = box(pg, "#theme")
        bb = pg.locator("#theme").bounding_box()
        pg.mouse.move(bb["x"] + bb["width"] / 2, bb["y"] + bb["height"] / 2); pg.mouse.down(); pg.wait_for_timeout(300)
        x1, y1 = box(pg, "#theme"); col, off = shadow(css(pg, "#theme", "boxShadow"))
        pg.mouse.up(); pg.wait_for_timeout(300)
        # The release toggled the theme; put it back.
        if pg.get_attribute("html", "data-theme") != theme: pg.click("#theme"); pg.wait_for_timeout(300)
        check((round(x1 - x0), round(y1 - y0)) == (5, 5) and off == (0, 0, 0), f"{tag} press travels ({x1 - x0:.0f}, {y1 - y0:.0f}) px, shadow {off}")
        pg.screenshot(path=str(OUT / f"depth_practice_{theme}_{w}.png"))
        check(not errs, f"{tag} no console errors ({errs[:2]})")
        pg.close()

    # Reduced motion: no travel.
    pg = b.new_page(viewport={"width": 1500, "height": 950}, reduced_motion="reduce")
    pg.goto(PAGE + "?mode=practice&seed=7"); pg.wait_for_timeout(700)
    x0, y0 = box(pg, "#start"); bb = pg.locator("#start").bounding_box()
    pg.mouse.move(bb["x"] + 4, bb["y"] + 4); pg.mouse.down(); pg.wait_for_timeout(100)
    x1, y1 = box(pg, "#start"); pg.mouse.move(5, 5); pg.mouse.up()
    check((x1, y1) == (x0, y0) and css(pg, "#start", "transitionDuration").split(",")[0].strip() == "0s", f"reduced motion: press travels ({x1 - x0:.0f}, {y1 - y0:.0f}) px, no transition")
    pg.close()

    # A Peak shift at 40+ aircraft, then its result dialog, in both themes.
    for theme in ("dark", "light"):
        pg = b.new_page(viewport={"width": 1920, "height": 1080})
        pg.goto(PAGE + "?mode=peak&seed=7"); pg.wait_for_timeout(700)
        if pg.get_attribute("html", "data-theme") != theme: pg.click("#theme"); pg.wait_for_timeout(300)
        pg.keyboard.press(" ")
        while app(pg, "a.count()") < 40 and app(pg, "a.real_s()") < 3600: app(pg, "a.fast_forward(1200)")
        pg.wait_for_timeout(300)
        n = app(pg, "a.count()")
        check(n >= 40 and not pg.is_disabled("#end-shift"), f"[{theme}] Peak at {n} aircraft, End shift enabled")
        col, off = shadow(css(pg, "#end-shift", "boxShadow"))
        check(off == (5, 5, 0), f"[{theme}] enabled End shift has its shadow {off}")
        segs = pg.evaluate("[...document.querySelectorAll('.seg button')].filter(e => e.offsetParent).map(e => [e.getAttribute('aria-pressed'), getComputedStyle(e).boxShadow])")
        up = [shadow(s)[1] for p, s in segs if p == "false"]; down = [shadow(s)[1] for p, s in segs if p == "true"]
        check(up and down and all(o == (5, 5, 0) for o in up) and all(o == (0, 0, 0) for o in down),
              f"[{theme}] runway toggles: {len(up)} released stand up, {len(down)} pressed sit flat")
        pg.screenshot(path=str(OUT / f"depth_peak_{theme}.png"))
        pg.click("#end-shift"); pg.wait_for_timeout(400)
        t = pg.text_content("#res-title")
        col, off = shadow(css(pg, "#result", "boxShadow"))
        check(css(pg, "#res-title", "textTransform") == "none" and t == "Shift over" and off == (5, 5, 0),
              f"[{theme}] result: mixed-case title {t!r}, dialog shadow {off}")
        pg.screenshot(path=str(OUT / f"depth_result_{theme}.png"))
        pg.close()
    b.close()

print(f"\n{sum(res)}/{len(res)} passed")
sys.exit(0 if all(res) else 1)
