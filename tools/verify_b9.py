"""B9 verification: wind and flow on the page, east flow end to end by real
keys and clicks, and the standing hardening checklist (skills.md Phase 4):
zero errors on fresh and seeded profiles, both themes at 1500 and 1920 px,
reduced motion, no alert()/confirm().

`fast_forward` is the only hook that moves time; read-only hooks report
state. Run: python3 tools/verify_b9.py <screenshot dir>
"""
from __future__ import annotations
import json, pathlib, re, sys
from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
PAGE = (ROOT / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
results: list[tuple[bool, str]] = []

def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, e):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {e}; }})()")

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def ff_until(pg, cond, max_ticks=400_000, chunk=100):
    t = 0
    while not app(pg, cond) and t < max_ticks:
        app(pg, f"a.fast_forward({chunk})"); t += chunk
    frame(pg)
    return app(pg, cond)

def watch(pg):
    errs, dialogs = [], []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (dialogs.append(d.message), d.dismiss()))
    return errs, dialogs

def banner(pg):
    return pg.text_content("#banner") if "on" in (pg.get_attribute("#banner", "class") or "") else ""

# ---- no alert() / confirm() in the source or the build -----------------------
CALL = re.compile(r"\b(?:window\.)?(?:alert|confirm|prompt)\s*\(")
for f in (ROOT / "web" / "shell.html", ROOT / "dist" / "index.html"):
    hits = CALL.findall(f.read_text())
    check(not hits, f"no alert/confirm/prompt calls in {f.name} ({len(hits)})")

with sync_playwright() as pw:
    b = pw.chromium.launch()

    # ---- fresh, v1 and v2 profiles: a minute of Normal, zero errors ---------
    profiles = {
        "fresh": None,
        "v1": {"v": 1, "settings": {"theme": "light"}},
        "v2": {"v": 2, "settings": {"theme": "dark"}, "bests": {"normal": {"movements": {"n": 30, "offered": 40, "seed": 9, "date": "2026-11-07"}}}},
    }
    for name, store in profiles.items():
        ctx = b.new_context(viewport={"width": 1500, "height": 950})
        if store is not None:
            ctx.add_init_script(f"if (!sessionStorage.getItem('seeded')) {{ localStorage.setItem('twr.v1', {json.dumps(json.dumps(store))}); sessionStorage.setItem('seeded', '1'); }}")
        pg = ctx.new_page()
        errs, dialogs = watch(pg)
        pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
        pg.select_option("#mode", "normal"); pg.fill("#seed-in", "5"); pg.click("#start"); frame(pg)
        ff_until(pg, "a.count() > 0")
        pg.click("#strip0"); pg.keyboard.press(" "); frame(pg); pg.keyboard.press(" ")
        app(pg, f"a.fast_forward({60 * 60})"); frame(pg)
        pg.wait_for_timeout(300)
        saved = json.loads(pg.evaluate("localStorage.getItem('twr.v1')") or "null")
        kept = store is None or (saved and saved["settings"] == store["settings"] and (store.get("bests") is None or saved["bests"] == store["bests"]))
        check(not errs and not dialogs and banner(pg) == "" and kept,
              f"{name} profile: a minute of Normal, no error ({errs[:2]}), no dialog, no banner, saved data kept")
        ctx.close()

    ctx = b.new_context(viewport={"width": 1500, "height": 950})
    pg = ctx.new_page()
    errs, dialogs = watch(pg)
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)

    # ---- wind and flow on screen ---------------------------------------------
    check(pg.text_content("#wind") == "calm" and pg.text_content("#flow") == "west", f"Practice: calm, west ({pg.text_content('#wind')}, {pg.text_content('#flow')})")
    east_seed = west_seed = None
    for s in range(1, 200):
        pg.select_option("#mode", "light"); pg.fill("#seed-in", str(s)); pg.click("#start"); frame(pg)
        if app(pg, "a.flow()") == "east" and east_seed is None:
            east_seed = s
        if app(pg, "a.flow()") == "west" and west_seed is None:
            west_seed = s
        if east_seed and west_seed:
            break
    check(east_seed is not None, f"a seed draws east flow: {east_seed}")
    pg.select_option("#mode", "light"); pg.fill("#seed-in", str(east_seed)); pg.click("#start"); frame(pg)
    wind, flow = pg.text_content("#wind"), pg.text_content("#flow")
    check(wind == app(pg, "a.wind_text()") and flow == "east", f"bar shows the sim's wind: {wind}, {flow}")
    # C4: the panel names them as pressed toggles, the sim's text agrees.
    pr = lambda sel: pg.evaluate(f"[...document.querySelectorAll('{sel} button')].filter(b => b.getAttribute('aria-pressed') === 'true').map(b => b.dataset.end).join(' ')")
    rwys = f"land {pr('#rw-land')} · depart {pr('#rw-dep')}"
    check(rwys == "land 11R 10 · depart 11L 09" and app(pg, "a.runways_text()") == rwys, f"panel names the runways: {rwys!r}")
    check(re.fullmatch(r"\d{3}/\d{2}(G\d{2})?", wind) is not None, f"METAR-style wind: {wind}")

    # ---- east flow end to end: one arrival, one departure, real keys ----------
    box = pg.locator("#map").bounding_box()
    ox, oy, cw, ch = box["x"], box["y"], box["width"], box["height"]
    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)

    finals = "Array.from({length: a.count()}, (_, j) => j).filter(j => a.phase(j).startsWith('Final') && !a.strip_state(j).includes('cleared to land'))"
    landed = None
    for _ in range(6):
        ff_until(pg, f"{finals}.length > 0")
        i = app(pg, f"{finals}[0]")
        rwy = app(pg, f"a.phase({i})")
        pg.click(f"#strip{i}"); pg.keyboard.press("c"); frame(pg)
        ff_until(pg, f"a.phase({i}).startsWith('Landing') || a.phase({i}).startsWith('GoAround')", chunk=20)
        if app(pg, f"a.phase({i})").startswith("Landing"):
            landed = (app(pg, f"a.callsign({i})"), app(pg, f"a.phase({i})"), rwy)
            break
    check(landed and re.search(r"\b(11R|10)\b", landed[1]), f"east arrival landed: {landed}")
    if landed:
        tel = pg.get_attribute(f"#strip{i}", "title")
        check(re.fullmatch(r"[A-Z][A-Z ]* \d+ · [A-Z0-9]{4}", tel or "") is not None, f"strip tooltip gives telephony and type: {tel!r}")

    deps = "Array.from({length: a.count()}, (_, j) => j).filter(j => a.phase(j) === 'OnStand' && / · dep /.test(a.strip_state(j)))"
    ok = ff_until(pg, f"{deps}.length > 0")
    i = app(pg, f"{deps}[0]")
    state = app(pg, f"a.strip_state({i})")
    want = re.search(r"dep (\w+)", state).group(1)
    pg.click(f"#strip{i}"); pg.keyboard.press("p"); pg.keyboard.press("Enter"); frame(pg)
    ff_until(pg, f"a.phase({i}) === 'Holding'")
    pg.keyboard.press("t"); frame(pg)
    zoom_at(cw / 2, ch / 2, -12)
    need = 3100 if re.search(r" [HJ]$", app(pg, f"a.wake({i})")) else 2030
    entries = app(pg, f"Array.from(a.entries_for('{want}', {need}))")
    pts = [(n, app(pg, f"a.node_screen({n})")) for n in entries]
    pts = [(n, p) for n, p in pts if 40 < p[0] < cw - 40 and 40 < p[1] < ch - 40]
    n, (ex, ey) = pts[0]
    zoom_at(ex, ey, 12)
    ex, ey = app(pg, f"a.node_screen({n})")
    pg.mouse.click(ox + ex, oy + ey); frame(pg)
    picked = app(pg, "a.taxi_dest()") == n
    pg.keyboard.press("Enter"); frame(pg)
    zoom_at(ex, ey, -12)
    for _ in range(10):
        ff_until(pg, f"a.phase({i}).startsWith('HoldShort')")
        if "X" in app(pg, f"a.verbs({i})"):
            pg.keyboard.press("x"); frame(pg); continue
        break
    pg.keyboard.press("l"); frame(pg)
    ff_until(pg, f"a.phase({i}).startsWith('LinedUp')")
    pg.keyboard.press("k"); frame(pg)
    up = ff_until(pg, f"a.phase({i}).startsWith('Airborne') || a.phase({i}).startsWith('Departed')", chunk=20)
    ph = app(pg, f"a.phase({i})")
    check(picked and up and want in ("11L", "09") and want in ph, f"east departure {app(pg, f'a.callsign({i})')} ({app(pg, f'a.wake({i})')}) took off from {want}: {ph}")

    # ---- the result dialog repeats the wind ----------------------------------
    while not app(pg, "a.over()"):
        app(pg, "a.fast_forward(6000)")
    frame(pg); frame(pg)
    sub = pg.text_content("#res-sub")
    check(f"wind {wind}, east flow" in sub, f"result names the wind: {sub!r}")
    ch_row = pg.text_content("#res-rows")
    check("go-arounds, pilot or weather" in ch_row, "challenges row names both causes")
    pg.keyboard.press("Escape"); frame(pg)

    # ---- both themes, 1500 and 1920 px: bar fits, nothing lopsided ------------
    for theme in ("dark", "light"):
        if pg.get_attribute("html", "data-theme") != theme:
            pg.click("#theme"); frame(pg)
        for w in (1500, 1920):
            pg.set_viewport_size({"width": w, "height": 950}); frame(pg)
            over = pg.evaluate("(() => { const b = document.querySelector('.bar'); return b.scrollWidth - b.clientWidth; })()")
            gap = pg.evaluate("innerWidth - Math.max(...[...document.querySelectorAll('main > *')].filter(e => e.offsetParent || e.tagName === 'CANVAS').map(e => e.getBoundingClientRect().right))")
            check(over <= 0 and gap <= 16, f"{theme} {w}px: bar fits (overflow {over}px), {gap:.0f}px empty at the right")
            pg.screenshot(path=str(OUT / f"b9_{theme}_{w}.png"))
    pg.set_viewport_size({"width": 1500, "height": 950}); frame(pg)
    check(not errs and not dialogs and banner(pg) == "", f"no console error, dialog or banner ({errs[:2]}, {dialogs})")
    ctx.close()

    # ---- reduced motion: zoom jumps, no spring -------------------------------
    seen = {}
    for motion in ("reduce", "no-preference"):
        ctx = b.new_context(viewport={"width": 1500, "height": 950}, reduced_motion=motion)
        pg = ctx.new_page()
        pg.goto(PAGE); pg.wait_for_timeout(800); frame(pg)
        box = pg.locator("#map").bounding_box()
        pg.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
        pg.mouse.wheel(0, -300)
        frame(pg); z1 = app(pg, "a.zoom()")
        pg.wait_for_timeout(600); z2 = app(pg, "a.zoom()")
        seen[motion] = (z1, z2)
        ctx.close()
    r, n = seen["reduce"], seen["no-preference"]
    check(abs(r[0] - r[1]) < 1e-9 and abs(n[0] - n[1]) > 1e-6, f"reduced motion: zoom lands in one frame {r}; without it, it springs {n}")
    b.close()

n = sum(ok for ok, _ in results)
print(f"\n{n}/{len(results)} passed")
sys.exit(0 if n == len(results) else 1)
