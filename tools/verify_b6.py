"""B6 verification: shifts started from the page's own controls.

Mode and seed are set with real input (select, typing, the Start button, a
link). `fast_forward` runs the sim's own ticks so a shift's first minutes do
not take real minutes; it is the only hook that moves time. Read-only hooks
report state.
"""
from __future__ import annotations
import pathlib, re, sys
from playwright.sync_api import sync_playwright

# The built page next to this folder, so the scripts run on any machine.
PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
# B9: the operator table grew to 68 rows from the aircraft research; a
# callsign is any 3-letter ICAO prefix and 3-4 digits.
OPS_RE = re.compile(r"[A-Z]{3}\d{3,4}")
results: list[tuple[bool, str]] = []

def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)

def app(pg, expr):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {expr}; }})()")

def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")

def ff_until(pg, cond, max_ticks=400_000, chunk=200):
    t = 0
    while not app(pg, cond) and t < max_ticks:
        app(pg, f"a.fast_forward({chunk})"); t += chunk
    frame(pg)
    return app(pg, cond)

def first_five(pg):
    ff_until(pg, "a.spawn_log().length >= 5")
    return app(pg, "a.spawn_log().slice(0, 5)")

with sync_playwright() as pw:
    b = pw.chromium.launch()
    pg = b.new_page(viewport={"width": 1500, "height": 950})
    errs, dialogs = [], []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (dialogs.append(d.message), d.dismiss()))
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)

    # ---- Practice is the page as it opens --------------------------------
    names = app(pg, "Array.from({length: a.count()}, (_, i) => a.callsign(i))")
    check(sorted(names) == ["IGO512", "IGO713"], f"Practice: exactly IGO512 and IGO713 ({names})")
    check(pg.input_value("#mode") == "practice", "mode picker reads Practice")
    check(pg.text_content("#rate") == "-" and pg.text_content("#queues") == "-", "HUD shift fields blank in Practice")
    check(pg.text_content("#wake0") == "A20N M", f"strip corner shows type and wake: {pg.text_content('#wake0')!r}")

    # Keys typed into the seed field are not verbs: IGO512 stays on stand.
    pg.click("#strip0"); frame(pg)
    i512 = app(pg, "a.index_of('IGO512')")
    pg.click("#seed-in"); pg.keyboard.type("p"); frame(pg)
    check(app(pg, f"a.phase({i512})") == "OnStand", f"P typed in the seed field did not push ({app(pg, f'a.phase({i512})')})")
    pg.fill("#seed-in", "")

    # ---- Normal, started from the controls ------------------------------
    pg.select_option("#mode", "normal")
    pg.click("#start"); frame(pg)
    check(app(pg, "a.shift_mode()") == "normal" and app(pg, "a.count()") == 0, "Start: a Normal shift on an empty airport")
    check(re.search(r"\?mode=normal&seed=\d+$", pg.url) is not None, f"URL carries mode and seed: {pg.url}")
    check(pg.text_content("#rate") == "53/h", f"HUD rate {pg.text_content('#rate')!r}")
    check(pg.text_content("#shift-clock-label") == "Shift left" and pg.text_content("#shift-clock") == "30:00", f"shift clock {pg.text_content('#shift-clock')!r}")
    check(ff_until(pg, "a.count() > 0"), "traffic appears")
    cs, corner, state = pg.text_content("#cs0"), pg.text_content("#wake0"), pg.text_content("#state0")
    check(OPS_RE.fullmatch(cs) is not None and re.fullmatch(r"[A-Z0-9]{4} [MHJ]", corner) is not None, f"first strip {cs} {corner!r}: {state}")
    check(state.startswith("Final ") or " · dep " in state, f"arrival on final or departure with its runway: {state!r}")
    t = app(pg, "a.real_s()")
    check(pg.text_content("#shift-clock") == f"{(1800 - int(t)) // 60:02d}:{(1800 - int(t)) % 60:02d}", f"shift clock counts down: {pg.text_content('#shift-clock')} at {t:.0f} real s")
    check(re.fullmatch(r"final \d+ · stands full \d+", pg.text_content("#queues")) is not None, f"HUD queues {pg.text_content('#queues')!r}")

    # ---- Replay: a link and the seed field give the same shift ----------
    pg.goto(PAGE + "?mode=peak&seed=7"); pg.wait_for_timeout(600); frame(pg)
    check(pg.input_value("#mode") == "peak" and pg.text_content("#seed") == "7", "link opens Peak, seed 7")
    a5 = first_five(pg)
    pg.reload(); pg.wait_for_timeout(600); frame(pg)
    b5 = first_five(pg)
    check(a5 == b5 and len(a5) == 5, f"same link, same first five: {a5}")
    pg.goto(PAGE); pg.wait_for_timeout(600); frame(pg)
    pg.select_option("#mode", "peak"); pg.fill("#seed-in", "7"); pg.click("#start"); frame(pg)
    c5 = first_five(pg)
    check(c5 == a5, f"seed field + Start gives the same shift: {c5}")
    check(pg.url.endswith("?mode=peak&seed=7"), f"URL after Start: {pg.url}")

    # Land one arrival with real keys and clicks, park it, and watch it shut
    # down: its strip leaves the bay. Nobody else is controlled.
    box = pg.locator("#map").bounding_box()
    ox, oy = box["x"], box["y"]
    def zoom_at(sx, sy, notches):
        pg.mouse.move(ox + round(sx), oy + round(sy))
        for _ in range(abs(notches)):
            pg.mouse.wheel(0, -100 if notches > 0 else 100); pg.wait_for_timeout(25)
        pg.wait_for_timeout(700)
    def click_node(n, notches=10):
        x, y = app(pg, f"a.node_screen({n})")
        zoom_at(x, y, notches)
        x, y = app(pg, f"a.node_screen({n})")
        pg.mouse.click(ox + round(x), oy + round(y)); frame(pg)
        zoom_at(x, y, -notches)
    finals = "Array.from({length: a.count()}, (_, j) => j).filter(j => a.phase(j).startsWith('Final'))"
    # A pilot go-around (5% of approaches) sends one round again: take the
    # next arrival instead.
    pilot_ga = []
    for _ in range(5):
        ff_until(pg, f"{finals}.filter(j => !a.strip_state(j).includes('cleared to land')).length > 0")
        i = app(pg, f"{finals}.filter(j => !a.strip_state(j).includes('cleared to land'))[0]")
        cs = app(pg, f"a.callsign({i})")
        pg.click(f"#strip{i}"); pg.keyboard.press("c"); frame(pg)
        ff_until(pg, f"a.phase({i}) === 'Holding' || a.phase({i}).startsWith('GoAround')", chunk=20)
        if app(pg, f"a.phase({i})") == "Holding":
            break
        pilot_ga.append(cs)
    check(app(pg, f"a.phase({i})") == "Holding", f"{cs} cleared with C, landed, vacated (pilot go-arounds first: {pilot_ga})")
    pg.keyboard.press("t"); frame(pg)
    taken = set(app(pg, "Array.from({length: a.count()}, (_, j) => a.route_end(j))"))
    picked = None
    for n in [n for n in app(pg, "Array.from(a.stand_nodes())") if n not in taken][::7]:
        click_node(n)
        if app(pg, "a.preview_edges().length") > 0:
            picked = n
            break
        pg.keyboard.press("Escape"); pg.keyboard.press("t"); frame(pg)
    pg.keyboard.press("Enter"); frame(pg)
    for _ in range(10):
        ff_until(pg, f"a.phase({i}).startsWith('HoldShort') || a.phase({i}) === 'OnStand'", chunk=20)
        if app(pg, f"a.phase({i})") == "OnStand":
            break
        pg.keyboard.press("x"); frame(pg)
    check(app(pg, f"a.phase({i})") == "OnStand" and pg.is_visible(f"#strip{i}"), f"{cs} on stand {picked}, strip shown: {pg.text_content(f'#state{i}')!r}")
    on_at = app(pg, "a.real_s()")
    check(ff_until(pg, f"a.phase({i}) === 'ShutDown'", chunk=60), f"{cs} shut down")
    after = app(pg, "a.real_s()") - on_at
    check(115 <= after <= 125, f"shut down {after:.0f} real s after reaching the stand (plan: 120)")
    check(app(pg, f"a.gone({i})") and not pg.is_visible(f"#strip{i}"), f"{cs}'s strip left the bay")

    # ---- Surge -------------------------------------------------------------
    pg.select_option("#mode", "surge"); pg.fill("#seed-in", "3"); pg.click("#start"); frame(pg)
    check(pg.text_content("#rate") == "53/h" and pg.text_content("#shift-clock-label") == "Surge", "Surge starts at 53/h, clock counts up")
    app(pg, f"a.fast_forward({10 * 60 * 60})"); frame(pg)
    check(pg.text_content("#rate") == "85/h" and pg.text_content("#shift-clock") == "10:00", f"Surge at 10:00: {pg.text_content('#rate')}")

    # ---- Both themes, and the bar fits ------------------------------------
    for theme in ("dark", "light"):
        if pg.get_attribute("html", "data-theme") != theme:
            pg.click("#theme"); frame(pg)
        fg = pg.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--fg').trim()")
        sel_c = pg.evaluate("getComputedStyle(document.getElementById('mode')).color")
        want = "rgb(255, 255, 255)" if theme == "dark" else "rgb(0, 0, 0)"
        check(sel_c == want, f"{theme}: picker text {sel_c} (fg {fg})")
        for w in (1500, 1920):
            pg.set_viewport_size({"width": w, "height": 950}); frame(pg)
            over = pg.evaluate("(() => { const b = document.querySelector('.bar'); return b.scrollWidth - b.clientWidth; })()")
            check(over <= 0, f"{theme} {w}px: status bar fits (overflow {over}px)")
        pg.set_viewport_size({"width": 1500, "height": 950}); frame(pg)
        pg.screenshot(path=str(OUT / f"b6_{theme}.png"))

    banner = pg.text_content("#banner") if "on" in (pg.get_attribute("#banner", "class") or "") else ""
    check(banner == "", f"no error banner ({banner!r})")
    check(not errs, f"no console errors ({errs[:3]})")
    check(not dialogs, f"no dialogs ({dialogs})")
    b.close()

n = sum(ok for ok, _ in results)
print(f"\n{n}/{len(results)} passed")
sys.exit(0 if n == len(results) else 1)
