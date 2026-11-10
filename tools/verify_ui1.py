"""PLAN-ui-console A2 + A3: the radio line, driven by real key presses, in
both themes at 1500 and 1920; every visible control is 44 px tall.

Run: python3 tools/verify_ui1.py <shots folder>
"""
import pathlib, sys
from playwright.sync_api import sync_playwright
PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1]); OUT.mkdir(parents=True, exist_ok=True)
res = []
def check(ok, msg): res.append(ok); print(("PASS " if ok else "FAIL ") + msg)
with sync_playwright() as pw:
    b = pw.chromium.launch()
    for theme, w, h in (("dark", 1500, 950), ("light", 1920, 1080)):
        ctx = b.new_context(viewport={"width": w, "height": h}, color_scheme=theme)
        pg = ctx.new_page(); errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
        pg.goto(PAGE); pg.wait_for_timeout(900)
        pg.click("#radio-in")
        pg.keyboard.type("ig"); pg.wait_for_timeout(100)
        words = pg.eval_on_selector_all("#radio-next button", "bs => bs.map(b => b.textContent)")
        check(len(words) >= 1 and all(x.startswith("IG") for x in words), f"[{theme} {w}] 'ig' offers callsigns {words}")
        pg.fill("#radio-in", ""); pg.keyboard.type("IGO713 "); pg.wait_for_timeout(100)
        words = pg.eval_on_selector_all("#radio-next button", "bs => bs.map(b => b.textContent)")
        check("LAND" in words and "PUSHBACK" not in words, f"[{theme}] IGO713 on final is offered {words}")
        pg.keyboard.type("la"); pg.keyboard.press("Tab"); pg.wait_for_timeout(50)
        val = pg.input_value("#radio-in")
        check(val.strip().upper() == "IGO713 LAND", f"[{theme}] Tab completes to {val!r}")
        pg.keyboard.press("Enter"); pg.wait_for_timeout(300)
        state = pg.evaluate("Array.from(document.querySelectorAll('.strip .state')).map(s => s.textContent).join(' | ')")
        check("cleared to land" in state and pg.input_value("#radio-in") == "" and pg.text_content("#radio-said") == "", f"[{theme}] Enter sends: strips say {state[:120]!r}")
        pg.keyboard.type("IGO713 FLY"); pg.keyboard.press("Enter"); pg.wait_for_timeout(100)
        said = pg.text_content("#radio-said")
        check("not an instruction" in said, f"[{theme}] bad line says why: {said!r}")
        pg.keyboard.press("Escape"); pg.wait_for_timeout(50)
        check(pg.input_value("#radio-in") == "" and pg.evaluate("document.activeElement.id") != "radio-in", f"[{theme}] Escape clears and leaves the line")
        hs = pg.eval_on_selector_all("button:not([hidden]), select, input", "es => es.filter(e => e.offsetParent).map(e => [e.id || e.textContent.trim().slice(0,10), Math.round(e.getBoundingClientRect().height)])")
        small = [x for x in hs if x[1] < 44]
        check(not small, f"[{theme}] every visible control at least 44 px tall (under: {small})")
        sw = pg.evaluate("document.documentElement.scrollWidth"); check(sw <= w, f"[{theme}] no horizontal scroll ({sw} <= {w})")
        pg.screenshot(path=str(OUT / f"radio-{theme}-{w}.png"))
        check(not errs, f"[{theme}] console errors {errs}")
        ctx.close()
    b.close()
print(f"{sum(res)}/{len(res)}")
