"""PLAN-ui-console step 3: Nothing OS Classic tokens from mockup A.

Run: python3 tools/verify_ui3.py <shots-folder> [page]. Selection by inversion,
sans prose and mono codes, no mixed-case text under 12 px, hairline contrast,
the red alarm bar on screen over a request tint, both themes, 1500 and 1920."""
import pathlib, sys, re
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").resolve().as_uri()
OUT = pathlib.Path(sys.argv[1]); OUT.mkdir(parents=True, exist_ok=True)
res = []
def check(ok, msg): res.append(ok); print(("PASS " if ok else "FAIL ") + msg)
def frame(pg): pg.wait_for_timeout(80)
def app(pg, js): return pg.evaluate(f"(() => {{ const a = TWR.app; return {js}; }})()")

def rgb(s):
    return [int(x) for x in re.findall(r"\d+", s)[:3]]
def lum(c):
    c = [x / 255 for x in c]
    c = [x / 12.92 if x <= 0.03928 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
def cr(a, b):
    la, lb = sorted([lum(rgb(a)), lum(rgb(b))], reverse=True)
    return (la + 0.05) / (lb + 0.05)

CS = "(sel, p) => { const e = document.querySelector(sel); return e ? getComputedStyle(e)[p] : null; }"
def css(pg, sel, p): return pg.evaluate(CS, [sel, p]) if False else pg.evaluate(f"([s, p]) => {{ const e = document.querySelector(s); return e ? getComputedStyle(e)[p] : null; }}", [sel, p])

with sync_playwright() as pw:
    b = pw.chromium.launch()
    for theme, w, h in (("dark", 1500, 950), ("light", 1500, 950), ("dark", 1920, 1080), ("light", 1920, 1080)):
        pg = b.new_page(viewport={"width": w, "height": h})
        errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
        pg.goto(PAGE + "?mode=practice&seed=7"); pg.wait_for_timeout(700); frame(pg)
        if pg.get_attribute("html", "data-theme") != theme: pg.click("#theme"); frame(pg)
        tag = f"[{theme} {w}]"
        # step 4 springs the inversion over 215 ms: measure it at rest
        pg.click("#strip0"); frame(pg); pg.wait_for_timeout(300)
        fg = pg.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--fg').trim()")
        sbg, scol = css(pg, "#strip0", "backgroundColor"), css(pg, "#strip0", "color")
        check(cr(sbg, scol) >= 7, f"{tag} selected strip inverted: bg {sbg}, text {scol}, {cr(sbg, scol):.1f}:1")
        ubg = css(pg, "#strip1", "backgroundColor")
        check(rgb(ubg) != rgb(sbg), f"{tag} unselected strip keeps the page ground: {ubg}")
        # request strip (IGO713 asks for landing): tint, words readable
        rq = pg.evaluate("[...document.querySelectorAll('.strip.req')].map(e => e.id)")
        if rq:
            r = rq[0]; i = r[5:]
            col = css(pg, f"#ask{i}", "color")
            # color-mix() comes back as color(srgb ...); resolve it through a canvas pixel.
            tint = pg.evaluate(f"""(() => {{ const c = document.createElement('canvas').getContext('2d');
                c.fillStyle = getComputedStyle(document.getElementById('{r}'), '::before').backgroundColor; c.fillRect(0, 0, 1, 1);
                return 'rgb(' + [...c.getImageData(0, 0, 1, 1).data.slice(0, 3)].join(',') + ')'; }})()""")
            check(cr(col, tint) >= 4.5, f"{tag} request words {col} on the tint {tint}: {cr(col, tint):.1f}:1 ({r})")
        fam = lambda s: (css(pg, s, "fontFamily") or "").split(",")[0].strip('" ')
        fonts = {s: fam(s) for s in ("#state0", "#cs0", ".label", "#start", "#radio-in", "#sel-hint", "#sel-cs", ".bayhead")}
        check(fonts["#state0"] == "Liberation Sans" and fonts["#sel-hint"] == "Liberation Sans"
              and all(fonts[k] == "ui-monospace" for k in ("#cs0", ".label", "#start", "#radio-in", "#sel-cs", ".bayhead")), f"{tag} fonts: {fonts}")
        small = pg.evaluate("""[...document.querySelectorAll('body *')].filter(e => e.offsetParent && [...e.childNodes].some(n => n.nodeType === 3 && n.textContent.trim()))
            .filter(e => parseFloat(getComputedStyle(e).fontSize) < 12 && getComputedStyle(e).textTransform !== 'uppercase').map(e => e.id || e.className)""")
        check(not small, f"{tag} no mixed-case text under 12 px: {small[:6]}")
        hair = css(pg, "#bay .baybox", "borderTopColor"); pbg = css(pg, "#strips", "backgroundColor")
        # Dark was lifted to about 3.7:1 on 2026-11-11 (the user asked for a
        # change to dark contrast); light stays near 2:1.
        lo, hi = (3, 5) if "dark" in tag else (1.6, 3)
        check(lo <= cr(hair, pbg) <= hi, f"{tag} bay hairline {hair} on rack {pbg}: {cr(hair, pbg):.2f}:1")
        over = pg.evaluate("(() => { const b = document.querySelector('.bar'); return b.scrollWidth - b.clientWidth; })()")
        check(over <= 0, f"{tag} header fits ({over} px over)")
        pg.screenshot(path=str(OUT / f"s3-{theme}-{w}.png"))
        check(not errs, f"{tag} zero console errors {errs[:2]}")
        pg.close()

    # Selected strip in a live conflict: inverted, red bar kept, alert legible.
    for theme in ("dark", "light"):
        pg = b.new_page(viewport={"width": 1500, "height": 950})
        pg.goto(PAGE); pg.wait_for_timeout(700); frame(pg)
        if pg.get_attribute("html", "data-theme") != theme: pg.click("#theme"); frame(pg)
        app(pg, "a.load_scenario('ground')"); frame(pg)
        tail = app(pg, "a.index_of('IGO602')")
        pg.click(f"#strip{tail}"); frame(pg); pg.wait_for_timeout(300)
        t = 0
        while not app(pg, "a.conflict_count() > 0") and t < 4000:
            app(pg, "a.fast_forward(20)"); t += 20
        frame(pg)
        lead = app(pg, "a.index_of('IGO601')")
        bar = lambda i: pg.evaluate(f"(() => {{ const s = getComputedStyle(document.getElementById('strip{i}'), '::after'); return [s.backgroundColor, s.width, s.zIndex]; }})()")
        bg = css(pg, f"#strip{tail}", "backgroundColor"); ac = css(pg, f"#alert{tail}", "color")
        check(rgb(bar(tail)[0]) == [215, 25, 33] and cr(bg, ac) >= 7,
              f"[{theme}] selected alarm strip: red bar {bar(tail)}, alert {cr(bg, ac):.1f}:1 on the inversion")
        # pixel test: the bar's colour on screen at the strip's left edge, requesting or not
        for i in (lead, tail):
            r = pg.locator(f"#strip{i}").bounding_box()
            # A card in "Needs you" has a 6 px attention border; the alarm bar
            # is the 4 px just inside it, so read one pixel in from the border.
            bw = float(css(pg, f"#strip{i}", "borderLeftWidth").replace("px", "") or 0)
            px = pg.screenshot(clip={"x": r["x"] + bw + 1, "y": r["y"] + r["height"] / 2, "width": 1, "height": 1})
            import io
            from PIL import Image
            c = list(Image.open(io.BytesIO(px)).convert("RGB").getpixel((0, 0)))
            req = "req" in pg.get_attribute(f"#strip{i}", "class")
            check(abs(c[0] - 215) < 12 and c[1] < 50 and c[2] < 60, f"[{theme}] strip{i} (request {req}) shows red at its left edge on screen: {c}")
        pg.screenshot(path=str(OUT / f"s3-conflict-{theme}.png"))
        pg.close()

    # Peak, dense rack, for the eye.
    pg = b.new_page(viewport={"width": 1500, "height": 950})
    pg.goto(PAGE + "?mode=peak&seed=7"); pg.wait_for_timeout(700)
    if pg.get_attribute("html", "data-theme") != "dark": pg.click("#theme"); frame(pg)
    app(pg, "a.fast_forward(6000)"); frame(pg); pg.wait_for_timeout(200)
    pg.screenshot(path=str(OUT / "s3-peak-dark.png"))
    b.close()
print(f"{sum(res)} PASS, {len(res) - sum(res)} FAIL")
