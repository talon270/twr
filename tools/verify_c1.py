"""C1 verification: strip bays. The list splits by what each aircraft needs
from you; a strip changes bay by itself when its phase changes. Real clicks
and keys; `fast_forward` only moves time.

  1. headers carry counts that match the strips under them (Practice)
  2. a departure walks Pushback -> Ground -> Runway
  3. an arrival walks Landings -> Ground
  4. inside Landings, the strip nearest the threshold is first
  5. a header click folds its bay, keeps the count, and unfolds again
  6. the folded state holds when an aircraft changes phase; both themes

Run: python3 tools/verify_c1.py <screenshot dir>
"""
from __future__ import annotations
import pathlib, sys
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").as_uri()
OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
OUT.mkdir(parents=True, exist_ok=True)
results: list[tuple[bool, str]] = []
NAMES = ["Pushback", "Ground", "Runway", "Landings", "Parked"]
# 2026-11-11: a card that waits on you leaves its bay for the "Needs you" box
# above them all (bay 5, verify_ui5.py). This script is about the bays, so it
# turns that box off with body[data-noattn] after every load. An aircraft on
# its stand that asks for nothing is Parked (bay 4), not Pushback.
NOATTN = "document.body.dataset.noattn = '1'"

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

def under(pg, i):
    """The bay a strip sits in, read from the page: the nearest visible header above it, in its column."""
    return pg.evaluate("""i => {
        const s = document.getElementById('strip' + i);
        if (!s || getComputedStyle(s).display === 'none') return null;
        const r = s.getBoundingClientRect(), y = r.top;
        let best = null, by = -1e9;
        for (const h of document.querySelectorAll('.bayhead')) {
          if (getComputedStyle(h).display === 'none') continue;
          // Step 2 puts the bays in a 2x2 grid: a header counts only over its own column.
          const hr = h.getBoundingClientRect();
          if (hr.right <= r.left || hr.left >= r.right) continue;
          const hy = hr.top;
          if (hy <= y && hy > by) { by = hy; best = h.dataset.bay; }
        }
        return best === null ? null : Number(best);
    }""", i)

def heads(pg):
    """Headers of bays that hold strips. Since step 2 every header shows, so read the count."""
    return pg.evaluate("""[...document.querySelectorAll('.bayhead')].filter(h => !h.classList.contains('empty'))
        .map(h => [Number(h.dataset.bay), Number(h.lastElementChild.textContent), h.getAttribute('aria-expanded')])""")

def open_page(pw, theme="dark"):
    b = pw.chromium.launch()
    ctx = b.new_context(viewport={"width": 1500, "height": 950}, color_scheme=theme)
    pg = ctx.new_page()
    errs = []
    pg.on("pageerror", lambda e: errs.append("PAGEERROR " + str(e)))
    pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
    pg.on("dialog", lambda d: (errs.append("DIALOG " + d.message), d.dismiss()))
    pg.goto(PAGE); pg.wait_for_timeout(600); pg.evaluate(NOATTN); frame(pg)
    return b, pg, errs

with sync_playwright() as pw:
    b, pg, errs = open_page(pw)

    # ---- 1: Practice. IGO512 on stand, IGO713 on final ----------------------
    check(under(pg, 0) == 4 and under(pg, 1) == 3, f"IGO512 under Parked (on stand, asks for nothing), IGO713 under Landings: {under(pg, 0)}, {under(pg, 1)}")
    h = heads(pg)
    check(h == [[3, 1, "true"], [4, 1, "true"]], f"bays with strips: Landings and Parked, counts 1 and 1: {h}")
    shown = pg.evaluate("[...document.querySelectorAll('.bayhead')].filter(h => getComputedStyle(h).display !== 'none').map(h => h.lastElementChild.textContent)")
    check(shown == ["0", "0", "0", "1", "1", "0"], f"step 2: all six headers show, empty ones read 0: {shown}")
    pg.screenshot(path=str(OUT / "c1_practice_dark.png"))

    # ---- 2: a departure walks Pushback -> Ground -> Runway -------------------
    pg.click("#strip0"); frame(pg)
    pg.click("#sel-verbs button[data-key=P]"); pg.keyboard.press("Enter"); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback" and under(pg, 0) == 0, "pushing back: still Pushback")
    check(ff_until(pg, "a.phase(0) === 'Holding'") and under(pg, 0) == 1, f"pushback ended (Holding): Ground, got {under(pg, 0)}")
    check(heads(pg) == [[1, 1, "true"], [3, 1, "true"]], f"headers follow: {heads(pg)}")
    check(app(pg, "a.load_scenario('runway')"), "scenario: IGO512 lined up on 29R"); frame(pg)
    i = app(pg, "a.index_of('IGO512')")
    check(app(pg, f"a.phase({i})").startswith("LinedUp") and under(pg, i) == 2, f"lined up: Runway, got {under(pg, i)}")
    pg.click(f"#strip{i}"); frame(pg)
    pg.click(f"#sel-verbs button[data-key=K]"); frame(pg)
    check(app(pg, f"a.phase({i})").startswith("TakeoffRoll") and under(pg, i) == 2, "takeoff roll: Runway")

    # ---- 3 and 4: an arrival, Landings -> Ground ----------------------------
    pg.goto(PAGE); pg.wait_for_timeout(600); pg.evaluate(NOATTN); frame(pg)
    k = 1
    check(app(pg, f"a.bay_of({k})") == 3, "IGO713 on final: Landings")
    pg.click(f"#strip{k}"); frame(pg)
    pg.keyboard.press("c"); frame(pg)
    got = ff_until(pg, f"a.phase({k}).startsWith('Landing')")
    check(got and under(pg, k) == 3, f"landing roll: still Landings (phase {app(pg, f'a.phase({k})')}, bay {under(pg, k)})")
    got = ff_until(pg, f"a.phase({k}) === 'Taxi'")
    check(got and under(pg, k) == 1, f"vacated, taxiing: Ground (phase {app(pg, f'a.phase({k})')}, bay {under(pg, k)})")
    pg.screenshot(path=str(OUT / "c1_arrival_ground_dark.png"))

    # ---- 4: nearest threshold first inside Landings --------------------------
    # A shift: wait until two arrivals are on final at once, then read the DOM order.
    pg.select_option("#mode", "normal"); pg.fill("#seed-in", "7"); pg.click("#start"); frame(pg)
    two = "Array.from({length: a.count()}, (_, i) => i).filter(i => !a.gone(i) && a.bay_of(i) === 3 && a.phase(i).startsWith('Final')).length >= 2"
    check(ff_until(pg, two, max_ticks=300_000, chunk=50), "shift, seed 7: two arrivals on final at once")
    finals = app(pg, "Array.from({length: a.count()}, (_, i) => i).filter(i => !a.gone(i) && a.bay_of(i) === 3).map(i => [i, a.bay_key(i)])")
    ys = {i: pg.evaluate("i => document.getElementById('strip' + i).getBoundingClientRect().top", i) for i, _ in finals}
    by_dist = [i for i, _ in sorted(finals, key=lambda v: v[1])]
    by_y = sorted(ys, key=ys.get)
    check(by_dist == by_y, f"Landings sorted nearest threshold first: by distance {by_dist}, on screen {by_y}")

    # ---- 5: fold ---------------------------------------------------------------
    pg.goto(PAGE); pg.wait_for_timeout(600); pg.evaluate(NOATTN); frame(pg)
    pg.click('.bayhead[data-bay="3"]'); frame(pg)
    check(not pg.is_visible("#strip1"), "folding Landings hides IGO713's strip")
    h = heads(pg)
    check(h == [[3, 1, "false"], [4, 1, "true"]], f"folded header keeps its count and says collapsed: {h}")
    check(pg.is_visible("#strip0"), "Parked strip unaffected")
    pg.screenshot(path=str(OUT / "c1_folded_dark.png"))
    pg.click('.bayhead[data-bay="3"]'); frame(pg)
    check(pg.is_visible("#strip1") and heads(pg)[0][2] == "true", "unfolding shows it again")

    # ---- 6: fold holds across a phase change; light theme -----------------------
    pg.click("#strip0"); frame(pg)
    pg.click('.bayhead[data-bay="4"]'); frame(pg)
    check(not pg.is_visible("#strip0"), "Parked folded: IGO512's strip is hidden but still selected")
    pg.keyboard.press("p"); pg.keyboard.press("Enter"); frame(pg)
    check(app(pg, "a.phase(0)") == "Pushback", "P pushes the selected IGO512 while its bay is folded")
    check(ff_until(pg, "a.phase(0) === 'Holding'") and pg.is_visible("#strip0") and under(pg, 0) == 1,
          "it moves to Ground, which is open: its strip shows")
    pg.click("#theme"); frame(pg)
    pg.screenshot(path=str(OUT / "c1_light.png"))
    ink = pg.evaluate("getComputedStyle(document.querySelector('.bayhead:not(.empty)')).color")
    print("  header colour in light:", ink)
    check(not errs, f"no page errors: {errs}")
    b.close()

bad = [w for ok, w in results if not ok]
print(f"\n{len(results) - len(bad)} PASS, {len(bad)} FAIL")
sys.exit(1 if bad else 0)
