"""LVP verification (PLAN-lvp.md step 8): the page under low-visibility procedures.

Practice's LVP toggle is pressed with real clicks; the `lsa` scenario is
loaded through the one write hook (`load_scenario`) and then driven with real
clicks and keys. The LSA outline is found by diffing the paused canvas with
LVP off and on, and its contrast is measured on the pixels that changed, in
both themes at 1500 and 1920 px. Fresh and seeded v2 profiles must load,
toggle and play with zero console errors and keep their saved bests.
"""
from __future__ import annotations
import collections, json, pathlib, sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
results: list[tuple[bool, str]] = []
# 3:1, the non-text contrast floor (WCAG 1.4.11) for a shape the player must see.
MIN_CONTRAST = 3.0


def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {expr}; }})()")


def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")


def lum(rgb):
    def ch(c):
        c /= 255
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (ch(v) for v in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def pixels(pg):
    return pg.evaluate("""(() => { const c = document.getElementById('map');
      return Array.from(c.getContext('2d').getImageData(0, 0, c.width, c.height).data); })()""")


def watch(pg):
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))
    return errs


with sync_playwright() as pw:
    b = pw.chromium.launch()

    # ---- the toggle and the outline, both themes, two widths -----------------
    for scheme, (w, h) in [("dark", (1500, 950)), ("light", (1500, 950)), ("dark", (1920, 1080)), ("light", (1920, 1080))]:
        ctx = b.new_context(viewport={"width": w, "height": h}, color_scheme=scheme)
        pg = ctx.new_page()
        errs = watch(pg)
        pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
        tag = f"{scheme} {w}"
        if scheme == "dark" and w == 1500:
            check(pg.is_visible("#lvp-btn") and not pg.is_visible("#lvp-tag") and not app(pg, "a.lvp()"),
                  "Practice: the LVP toggle shows, LVP is off, no LVP tag")
        pg.keyboard.press(" "); frame(pg)  # pause: only the toggle changes the canvas
        before = pixels(pg)
        pg.click("#lvp-btn"); frame(pg); pg.wait_for_timeout(100); frame(pg)
        after = pixels(pg)
        on = app(pg, "a.lvp()")
        wind = pg.text_content("#wind-box")
        changed = collections.Counter(tuple(after[k:k + 3]) for k in range(0, len(after), 4) if after[k:k + 3] != before[k:k + 3])
        bg = pg.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--bg').trim()")
        bg_rgb = tuple(int(bg[i:i + 2], 16) for i in (1, 3, 5)) if len(bg) == 7 else (0, 0, 0) if bg == "#000" else (255, 255, 255)
        top, n = changed.most_common(1)[0] if changed else ((0, 0, 0), 0)
        check(on and pg.is_visible("#lvp-tag") and "LVP" in wind and pg.get_attribute("#lvp-btn", "aria-pressed") == "true",
              f"{tag}: a click on LVP turns it on; the wind box reads {' '.join(wind.split())[:40]!r}")
        check(sum(changed.values()) > 200 and contrast(top, bg_rgb) >= MIN_CONTRAST,
              f"{tag}: the LSA outline is drawn ({sum(changed.values())} pixels changed); its colour {top} is {contrast(top, bg_rgb):.2f}:1 on {bg}")
        if scheme == "dark" and w == 1500:
            pg.screenshot(path=str(OUT / "lvp_outline_dark.png"))
            pg.click("#lvp-btn"); frame(pg)
            check(not app(pg, "a.lvp()") and not pg.is_visible("#lvp-tag"), "a second click turns LVP off and hides the tag")
            pg.select_option("#mode", "normal"); pg.fill("#seed-in", "5"); pg.click("#start"); frame(pg)
            check(not pg.is_visible("#lvp-btn"), "a scored shift hides the toggle: fog comes from the seed")
        check(not errs, f"{tag}: no console error ({errs[:2]})")
        ctx.close()

    # ---- the rules on the page: the lsa scenario -----------------------------
    ctx = b.new_context(viewport={"width": 1500, "height": 950}, color_scheme="dark")
    pg = ctx.new_page()
    errs = watch(pg)
    pg.goto(PAGE); pg.wait_for_timeout(600)
    for clear in (False, True):
        check(app(pg, "a.load_scenario('lsa')"), "scenario lsa loads")
        frame(pg); pg.wait_for_timeout(200)
        i = app(pg, "a.index_of('IGO714')")
        note, cls, ask = pg.text_content(f"#note{i}"), pg.get_attribute(f"#strip{i}", "class"), pg.text_content(f"#ask{i}")
        if not clear:
            check(note == "LSA: IGO713" and "req" not in cls and ask == "",
                  f"IGO713 held on Z: IGO714's note reads {note!r} and its strip does not ask for C ({ask!r})")
            # Withhold C: the go-around at 2 NM is the rule's, not the player's.
            for _ in range(200):
                if app(pg, f"a.phase({i})").startswith("GoAround"):
                    break
                app(pg, "a.fast_forward(20)")
            frame(pg)
            check(app(pg, f"a.strip_note({i})") == "went around: LSA occupied" and app(pg, "a.incident_count()") == 0,
                  f"C withheld: IGO714 goes around, note {app(pg, f'a.strip_note({i})')!r}, 0 incidents")
        else:
            pg.click(f"#strip{i}"); pg.keyboard.press("c"); frame(pg)
            for _ in range(200):
                if app(pg, "a.incident_count()") > 0:
                    break
                app(pg, "a.fast_forward(20)")
            frame(pg); pg.wait_for_timeout(300)
            rows = app(pg, "a.incident_rows()")
            check(not app(pg, f"a.phase({i})").startswith("GoAround") and len(rows) == 1
                  and rows[0].endswith("LSA · IGO713 in the 29L localizer sensitive area with IGO714 inside 2 NM"),
                  f"C still works, and inside 2 NM the busy LSA is one incident: {rows}")
            check(pg.text_content("#inc-count") == "1", f"the incident count reads {pg.text_content('#inc-count')!r}")
            pg.screenshot(path=str(OUT / "lvp_incident.png"))
    check(not errs, f"lsa scenario: no console error ({errs[:2]})")
    ctx.close()

    # ---- a seeded v2 profile keeps its bests ---------------------------------
    store = {"v": 2, "settings": {"theme": "dark"}, "bests": {"normal": {"movements": {"n": 30, "offered": 40, "seed": 9, "date": "2026-11-07"}}}}
    ctx = b.new_context(viewport={"width": 1500, "height": 950})
    ctx.add_init_script(f"if (!sessionStorage.getItem('seeded')) {{ localStorage.setItem('twr.v1', {json.dumps(json.dumps(store))}); sessionStorage.setItem('seeded', '1'); }}")
    pg = ctx.new_page()
    errs = watch(pg)
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    pg.click("#lvp-btn"); frame(pg)
    app(pg, "a.fast_forward(1200)"); frame(pg); pg.wait_for_timeout(300)
    saved = json.loads(pg.evaluate("localStorage.getItem('twr.v1')") or "null")
    check(not errs and app(pg, "a.lvp()") and saved and saved["bests"] == store["bests"] and saved["v"] == 2,
          f"seeded v2 profile: LVP on in Practice, a minute played, no error ({errs[:2]}), bests and v2 kept")
    ctx.close()
    b.close()

passed = sum(ok for ok, _ in results)
print(f"{passed} of {len(results)} checks passed")
sys.exit(0 if passed == len(results) else 1)
