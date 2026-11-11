"""Typed routes on the radio line (ground-commands REPORT rows 1, 6, 7), by
real typing and clicking in the page.

Run: python3 tools/verify_radio2.py <shots-folder> [page].

  1. IGO512 pushed back; typing "IGO512 TAXI TO " offers STAND and RUNWAY as buttons
  2. clicking the offered words builds the line; Enter sends; the aircraft taxis
  3. a typed stand, a typed runway with VIA, and a refusal that names the problem
  4. CROSS RUNWAY <designator> while holding short sends X
"""
import pathlib, sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").resolve().as_uri()
OUT = pathlib.Path(sys.argv[1]); OUT.mkdir(parents=True, exist_ok=True)
res = []
def check(ok, msg): res.append(bool(ok)); print(("PASS " if ok else "FAIL ") + msg, flush=True)
def app(pg, js): return pg.evaluate(f"(() => {{ const a = TWR.app; return {js}; }})()")
def frame(pg): pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")
def words(pg): return pg.evaluate("[...document.querySelectorAll('#radio-next button')].map(b => b.dataset.word)")
def say(pg, line):
    pg.fill("#radio-in", line); pg.dispatch_event("#radio-in", "input")
    pg.press("#radio-in", "Enter"); frame(pg)
    return pg.inner_text("#radio-said")

with sync_playwright() as pw:
    b = pw.chromium.launch()
    pg = b.new_context(viewport={"width": 1500, "height": 950}).new_page(); errs = []
    pg.on("pageerror", lambda e: errs.append(str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.goto(PAGE); pg.wait_for_timeout(700); frame(pg)

    # IGO512: push by the real button, wait for Holding
    pg.click("#strip0"); frame(pg)
    pg.click("#sel-verbs button[data-key=P]"); pg.keyboard.press("Enter"); frame(pg)
    t = 0
    while app(pg, "a.phase(0)") != "Holding" and t < 60000:
        app(pg, "a.fast_forward(200)"); t += 200
    frame(pg)
    check(app(pg, "a.phase(0)") == "Holding", "IGO512 pushed back and waiting for taxi")

    # 1 + 2: build the line by clicking offered words
    pg.click("#radio-in"); pg.fill("#radio-in", "IGO512 TAXI "); pg.dispatch_event("#radio-in", "input"); frame(pg)
    check("TO" in words(pg), f"after 'TAXI ' the radio offers TO: {words(pg)}")
    pg.click('#radio-next button[data-word="TO"]'); frame(pg)
    check(sorted(words(pg)) == ["RUNWAY", "STAND"], f"after 'TAXI TO ' it offers STAND and RUNWAY: {words(pg)}")
    pg.click('#radio-next button[data-word="RUNWAY"]'); frame(pg)
    check("29R" in words(pg), f"it offers runway designators: {words(pg)}")
    pg.click('#radio-next button[data-word="29R"]'); frame(pg)
    check(words(pg) == ["VIA"], f"after a runway it offers VIA: {words(pg)}")
    line = pg.input_value("#radio-in")
    check(line.strip() == "IGO512 TAXI TO RUNWAY 29R", f"the clicks built the line: {line!r}")
    pg.click("#radio-send"); frame(pg)
    check(app(pg, "a.phase(0)") == "Taxi" and pg.inner_text("#radio-said") == "", f"Send button sends it: phase {app(pg, 'a.phase(0)')}, said {pg.inner_text('#radio-said')!r}")
    pg.screenshot(path=str(OUT / "radio2-taxi.png"))

    # 3: refusals name the problem; stands and VIA by typing
    pg.goto(PAGE); pg.wait_for_timeout(700); frame(pg)
    pg.click("#strip0"); frame(pg)
    pg.click("#sel-verbs button[data-key=P]"); pg.keyboard.press("Enter"); frame(pg)
    t = 0
    while app(pg, "a.phase(0)") != "Holding" and t < 60000:
        app(pg, "a.fast_forward(200)"); t += 200
    frame(pg)
    said = say(pg, "IGO512 TAXI TO STAND NOPE9")
    check("no stand" in said.lower(), f"unknown stand refused by name: {said!r}")
    said = say(pg, "IGO512 TAXI TO RUNWAY 29R VIA ZZZ9")
    check("no taxiway" in said.lower(), f"unknown taxiway refused by name: {said!r}")
    check(app(pg, "a.phase(0)") == "Holding", "refusals leave the aircraft where it was")
    said = say(pg, "IGO512 TAXI TO RUNWAY 29R VIA Y6")
    check(said == "" and app(pg, "a.phase(0)") == "Taxi", f"typed VIA sends: said {said!r}, phase {app(pg, 'a.phase(0)')}")
    names = app(pg, "Array.from(a.preview_edges()).length")
    check(not errs, f"zero page errors {errs[:2]}")

    # 4: CROSS RUNWAY
    pg.goto(PAGE); pg.wait_for_timeout(700); frame(pg)
    check(app(pg, "a.load_scenario('c3-cross')"), "scenario c3-cross"); frame(pg)
    i = app(pg, "a.index_of('IGO520')")
    t = 0
    while not app(pg, f"a.phase({i}).startsWith('HoldShort')") and t < 80000:
        app(pg, "a.fast_forward(100)"); t += 100
    frame(pg)
    ph = app(pg, f"a.phase({i})")
    check(ph.startswith("HoldShort"), f"IGO520 holds short: {ph}")
    pg.click(f"#strip{i}"); frame(pg)
    pg.click("#radio-in"); pg.fill("#radio-in", "IGO520 CROSS "); pg.dispatch_event("#radio-in", "input"); frame(pg)
    offered = words(pg)
    check("RUNWAY" in offered, f"'CROSS ' offers RUNWAY: {offered}")
    said = say(pg, "IGO520 CROSS RUNWAY 99")
    check("holding short of" in said, f"wrong runway refused: {said!r}")
    d = ph.split('"')[1].split("/")[0]  # HoldShort("10/28") -> 10
    said = say(pg, f"IGO520 CROSS RUNWAY {d}")
    check(said == "" and not app(pg, f"a.phase({i})").startswith("HoldShort"), f"CROSS RUNWAY {d} sends X: said {said!r}, phase {app(pg, f'a.phase({i})')}")
    check(not errs, f"zero page errors {errs[:2]}")
    b.close()

bad = res.count(False)
print(f"\n{len(res) - bad} PASS, {bad} FAIL")
sys.exit(1 if bad else 0)
