"""C3 checks drive the built page by clicks and keys; this script is unrun.

Build dist/index.html outside the restricted sandbox, then run:
    python3 tools/verify_c3.py /tmp/twr-c3-shots

Scenario loading sets up traffic; fast_forward only advances time. Every
instruction goes through a real key, panel button, map/strip click or radio
form. Read-only hooks locate targets and inspect the resulting sim state.
"""
from __future__ import annotations
import json
import pathlib
import sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/twr-c3-shots")
OUT.mkdir(parents=True, exist_ok=True)
results: list[tuple[bool, str]] = []


def check(ok, what):
    results.append((bool(ok), what))
    print(("PASS " if ok else "FAIL ") + what, flush=True)


def app(pg, expr):
    return pg.evaluate(f"(() => {{ const a = TWR.app; return {expr}; }})()")


def frame(pg):
    pg.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")


def until(pg, condition, limit=100_000, chunk=10):
    for _ in range(0, limit, chunk):
        if app(pg, condition):
            frame(pg)
            return True
        app(pg, f"a.fast_forward({chunk})")
    frame(pg)
    return app(pg, condition)


def setup(pg, name="ground", selected=1):
    check(app(pg, f"a.load_scenario({json.dumps(name)})"), f"load {name}")
    frame(pg)
    pg.click(f"#strip{selected}")
    frame(pg)
    pg.wait_for_timeout(300)


def command(pg, key, button):
    if button:
        selector = f'#sel-verbs button[data-key="{key}"]'
        height = pg.locator(selector).bounding_box()["height"]
        check(height >= 44, f"{key} button is {height:g} px tall")
        before = pg.evaluate("document.activeElement.id")
        pg.click(selector)
        check(pg.evaluate("document.activeElement.id") == before, f"{key} mouse press preserves focus")
    else:
        pg.keyboard.press(key)
    frame(pg)


def zoom_to(pg, expression):
    x, y = app(pg, expression)
    box = pg.locator("#map").bounding_box()
    pg.mouse.move(box["x"] + x, box["y"] + y)
    for _ in range(10):
        pg.mouse.wheel(0, -100)
        pg.wait_for_timeout(25)
    pg.wait_for_timeout(700)


def map_click(pg, expression):
    x, y = app(pg, expression)
    box = pg.locator("#map").bounding_box()
    check(0 <= x <= box["width"] and 0 <= y <= box["height"], "map target is inside the canvas")
    pg.mouse.click(box["x"] + x, box["y"] + y)
    frame(pg)


def radio(pg, text):
    pg.click("#radio-in")
    pg.keyboard.press("ControlOrMeta+A")
    pg.keyboard.type(text)
    pg.click("#radio-send")
    frame(pg)
    check(pg.text_content("#radio-said") == "", f"radio accepted: {text}")
    pg.click("#radio-in")
    pg.keyboard.press("Escape")
    frame(pg)


with sync_playwright() as pw:
    browser = pw.chromium.launch()
    for theme in ("dark", "light"):
        pg = browser.new_page(viewport={"width": 1500, "height": 950}, color_scheme=theme)
        errors = []
        pg.on("pageerror", lambda e: errors.append(str(e)))
        pg.on("console", lambda m: m.type == "error" and errors.append(m.text))
        pg.on("dialog", lambda d: (errors.append("DIALOG " + d.message), d.dismiss()))
        pg.goto(PAGE + "?seed=7")
        pg.wait_for_function("window.TWR && TWR.app")
        frame(pg)
        pg.keyboard.press("Space")
        check(pg.locator("#paused").is_visible(), "pause real time; tests advance only the sim clock")

        for button in (False, True):
            path = "button" if button else "key"
            setup(pg)
            command(pg, "H", button)
            check(app(pg, "a.phase(1)") == "HoldPosition", f"H stops now by {path}")
            command(pg, "Enter", button)
            check(app(pg, "a.phase(1)") == "Taxi", f"Continue resumes H by {path}")
            points = app(pg, "Array.from(a.hold_points(1))")
            node = points[2]
            label = app(pg, f"a.hold_point_name({node})")
            zoom_to(pg, f"a.node_screen({node})")
            before = pg.locator("#map").screenshot()
            command(pg, "J", button)
            check(app(pg, "a.ground_pick()") == "hold-at", f"J opens the point picker by {path}")
            check(pg.locator("#map").screenshot() != before, "hold picker adds visible map marks")
            pg.screenshot(path=str(OUT / f"c3_hold_{theme}_{path}.png"))
            map_click(pg, f"a.node_screen({node})")
            check(app(pg, "a.hold_point(1)") == node, "click on the marked point sets that hold")
            check(label in pg.text_content("#state1") and label in pg.text_content("#sel-state"), "strip and panel name the hold point")
            check(until(pg, "a.phase(1) === 'HoldPosition'"), "aircraft reaches and holds at the point")
            check(label in pg.text_content("#state1"), "the reached point stays visible on the held strip")
            command(pg, "Enter", button)
            check(app(pg, "a.phase(1)") == "Taxi", f"Continue releases the point by {path}")

            setup(pg)
            command(pg, "J", button)
            command(pg, "Escape", button)
            check(app(pg, "a.ground_pick()") == "" and app(pg, "a.hold_point(1)") == -1, "Cancel leaves no hold instruction")

            for target in ("map", "strip"):
                setup(pg, "c3-give-way", 0)
                zoom_to(pg, "a.aircraft_screen(1)")
                command(pg, "Y", button)
                if target == "map":
                    map_click(pg, "a.aircraft_screen(1)")
                else:
                    pg.click("#strip1")
                    frame(pg)
                check(app(pg, "a.selected_callsign()") == "IGO601", "give-way selection keeps the issuing aircraft selected")
                check(app(pg, "a.give_way_to(0)") == "IGO602", f"Y by {path}, target by {target}")
                check("giving way to IGO602" in pg.text_content("#state0") and "giving way to IGO602" in pg.text_content("#sel-state"), "strip and panel name the give-way target")
                check(until(pg, "a.phase(0) === 'HoldPosition'", chunk=1), "give-way stops before the junction")
                check(until(pg, "a.give_way_to(0) === ''", chunk=1), "give-way releases itself after passage")
                check(app(pg, "a.phase(0)") == "Taxi" and app(pg, "a.incident_count()") == 0, "automatic release resumes with zero incidents")
                check("giving way" not in pg.text_content("#state0"), "released give-way disappears from the strip")

            setup(pg, "c3-give-way", 0)
            command(pg, "Y", button)
            pg.click("#strip0")
            frame(pg)
            check("itself" in pg.text_content("#sel-notice"), "self give-way refusal is inline")
            check(app(pg, "a.ground_pick()") == "give-way", "refused target leaves picker open")
            command(pg, "Escape", button)

            setup(pg)
            command(pg, "-", button)
            check(app(pg, "a.taxi_speed(1)") == "slow", f"slower by {path}")
            command(pg, "=", button)
            check(app(pg, "a.taxi_speed(1)") == "normal", f"faster returns to normal by {path}")
            command(pg, "=", button)
            check(app(pg, "a.taxi_speed(1)") == "expedite", f"faster reaches expedite by {path}")
            pg.keyboard.press("=")
            frame(pg)
            check("already" in pg.text_content("#sel-notice"), "speed limit refusal is inline")

            setup(pg, "c3-cross", 0)
            command(pg, "X", button)
            check(bool(app(pg, "a.cross_ahead(0)")), f"early X arms crossing by {path}")
            check(until(pg, "a.cross_ahead(0) === ''", chunk=1), "early crossing consumed at the runway")
            check(app(pg, "a.phase(0)") == "Taxi", "clear runway crossed without holding")

            setup(pg, "c3-backtrack", 0)
            command(pg, "B", button)
            check(app(pg, "a.backtrack_to(0)") == "27", f"backtrack by {path}")
            check(until(pg, "a.phase(0).startsWith('LinedUp')"), "backtrack ends lined up")

        setup(pg)
        node = app(pg, "Array.from(a.hold_points(1))[2]")
        label = app(pg, f"a.hold_point_name({node})")
        pg.fill("#radio-in", "IGO602 HOLD AT ")
        frame(pg)
        check(pg.locator("#radio-next button").count() > 0, "radio offers live point completions")
        radio(pg, f"IGO602 HOLD AT {label}")
        check(app(pg, "a.hold_point(1)") == node, "radio named hold reaches the sim")
        radio(pg, "IGO602 CONTINUE")
        check(app(pg, "a.hold_point(1)") == -1, "radio Continue cancels an ahead hold")
        radio(pg, "IGO602 TAXI SLOWER")
        check(app(pg, "a.taxi_speed(1)") == "slow", "radio slower reaches the sim")
        radio(pg, "IGO602 TAXI FASTER")
        check(app(pg, "a.taxi_speed(1)") == "normal", "radio faster reaches the sim")
        radio(pg, "IGO602 EXPEDITE TAXI")
        check(app(pg, "a.taxi_speed(1)") == "expedite", "radio expedite sets the top preset")
        setup(pg, "c3-give-way", 0)
        radio(pg, "IGO601 GIVE WAY TO IGO602")
        check(app(pg, "a.give_way_to(0)") == "IGO602", "radio give-way reaches the sim")
        setup(pg, "c3-cross", 0)
        radio(pg, "IGO520 CROSS IF CLEAR")
        check(bool(app(pg, "a.cross_ahead(0)")), "radio cross-if-clear reaches the sim")
        setup(pg, "c3-backtrack", 0)
        radio(pg, "IGO530 BACKTRACK")
        check(app(pg, "a.backtrack_to(0)") == "27", "radio backtrack reaches the sim")
        check(not errors, f"{theme}: no page errors or dialogs: {errors}")
        pg.close()
    browser.close()

bad = [what for ok, what in results if not ok]
print(f"\n{len(results) - len(bad)} PASS, {len(bad)} FAIL")
sys.exit(1 if bad else 0)
