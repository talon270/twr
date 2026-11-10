"""PLAN-ui-console step 4: motion. The 215 ms state spring on selection and
fold, the 8 px review rise that turns back mid-entrance, no @keyframes, and
every change snapping under reduced motion.

Run: python3 tools/verify_ui4.py <shots-folder> [page]. The folder is unused
(the motion has no picture); it keeps the suite's calling convention."""
import pathlib, sys, re
from playwright.sync_api import sync_playwright

PAGE = (pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).resolve().parents[1] / "dist" / "index.html").resolve().as_uri()
res = []
def check(ok, msg): res.append(ok); print(("PASS " if ok else "FAIL ") + msg)
def app(pg, js): return pg.evaluate(f"(() => {{ const a = TWR.app; return {js}; }})()")
CS = "([s, p]) => getComputedStyle(document.querySelector(s))[p]"
def css(pg, s, p): return pg.evaluate(CS, [s, p])
def nums(s): return [float(x) for x in re.findall(r"-?\d+\.?\d*", s)]

def end_shift(pg):
    pg.select_option("#mode", "light"); pg.fill("#seed-in", "3"); pg.click("#start"); pg.wait_for_timeout(80)
    # stop just short of the end, so the dialog opens on a real frame we can watch
    while not app(pg, "a.over()"):
        app(pg, "a.fast_forward(6000)")
        pg.wait_for_timeout(16)

with sync_playwright() as pw:
    b = pw.chromium.launch()
    for motion in ("no-preference", "reduce"):
        ctx = b.new_context(viewport={"width": 1500, "height": 950}, reduced_motion=motion)
        pg = ctx.new_page(); errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.on("console", lambda m: m.type == "error" and errs.append(m.text))
        pg.goto(PAGE + "?mode=practice&seed=7"); pg.wait_for_timeout(700)
        spring = motion == "no-preference"
        kf = pg.evaluate("[...document.styleSheets].flatMap(s => [...s.cssRules]).filter(r => r.type === CSSRule.KEYFRAMES_RULE).length")
        check(kf == 0, f"[{motion}] no @keyframes in the page ({kf})")

        # Strip selection: the inversion springs over ~215 ms, or snaps.
        dur = css(pg, "#strip1", "transitionDuration"); tf = css(pg, "#strip1", "transitionTimingFunction")
        before = css(pg, "#strip1", "backgroundColor")
        # sample the colour on frames right after the click
        pg.click("#strip1")
        # 10 frames of samples: the app sets .sel in its own frame loop, so a
        # fixed "2 frames in" sometimes lands before the class is applied.
        seq = pg.evaluate("""new Promise(r => { const e = document.getElementById('strip1'), out = [];
            (function f() { out.push(getComputedStyle(e).backgroundColor); if (out.length < 10) requestAnimationFrame(f); else r(out); })(); })""")
        pg.wait_for_timeout(400)
        after = css(pg, "#strip1", "backgroundColor")
        mids = [c for c in seq if c not in (before, after)]
        between = bool(mids)
        mid = mids[0] if mids else (seq[-1] if spring else next((c for c in seq if c != before), seq[-1]))
        if spring:
            check(dur.startswith("0.215s") and tf.startswith("linear(0 0%, 0.011"), f"[{motion}] strip transition {dur.split(',')[0]} on the state spring ({tf[:24]}...)")
            check(between, f"[{motion}] inversion passes through {len(mids)} in-between colours: {before} -> {mid} -> {after}")
        else:
            check(dur.split(",")[0] in ("0s",), f"[{motion}] strip transition {dur}")
            check(not mids, f"[{motion}] inversion lands at once, no colour in between: {before} -> {after} ({len(set(seq))} distinct over 10 frames)")

        # Bay fold chevron.
        cd = css(pg, ".bayhead .chev", "transitionDuration")
        check(cd == ("0.215s" if spring else "0s"), f"[{motion}] chevron transition {cd}")

        # Result dialog: 8 px rise, reversible mid-entrance.
        end_shift(pg)
        t0 = pg.evaluate("""new Promise(r => requestAnimationFrame(() => { const d = document.getElementById('result');
            const s = getComputedStyle(d); r([d.open, s.transform, s.opacity]); }))""")
        pg.wait_for_timeout(400)
        t1 = [css(pg, "#result", "transform"), css(pg, "#result", "opacity")]
        if spring:
            ty = nums(t0[1])[-1] if t0[1] != "none" else 0
            check(t0[0] and 0 < ty <= 8 and float(t0[2]) < 1, f"[{motion}] review rising: translateY {ty:.2f} px, opacity {float(t0[2]):.2f}")
        else:
            check(t0[0] and t0[1] == "none" and t0[2] == "1", f"[{motion}] review lands at once: {t0}")
        check(t1 == ["none", "1"], f"[{motion}] review at rest after 400 ms: {t1}")
        # Close during the entrance. At the real 215 ms the reversal from
        # ~0.3 lasts about 3 frames, and a sample started after Esc came back
        # through Playwright missed it 3 runs in 10 (2026-11-11), reading
        # 0 or 0.04 on a page that does turn back. So this check alone runs
        # the dialog at 10x its duration (the inline custom property; the
        # 215 ms itself is checked above), and the sampler starts before Esc.
        pg.keyboard.press("Escape"); pg.wait_for_timeout(400)
        pg.evaluate("""(() => { const d = document.getElementById('result');
            d.style.setProperty('--spring-state-ms', '2150ms'); d.showModal(); })()""")
        pg.wait_for_timeout(600)
        op_in = float(css(pg, "#result", "opacity"))
        # every frame for 2.5 s (reduced motion: 24 frames, ~400 ms, so the
        # snap is still read early): a reversal peaks where the entrance was and
        # falls through values in between; a jump goes straight to 0
        frames = 150 if spring else 24
        pg.evaluate("""n => { const d = document.getElementById('result'), out = window.__seq = [];
            (function f() { out.push(+getComputedStyle(d).opacity); if (out.length < n) requestAnimationFrame(f); })(); }""", frames)
        pg.keyboard.press("Escape")
        pg.wait_for_function("n => window.__seq.length >= n", arg=frames, timeout=10000)
        seq = pg.evaluate("window.__seq")
        gone = pg.evaluate("!document.getElementById('result').open && getComputedStyle(document.getElementById('result')).display")
        pg.evaluate("document.getElementById('result').style.removeProperty('--spring-state-ms')")
        if spring:
            k = seq.index(max(seq))
            fall = seq[k:]
            mids = [x for x in fall[1:] if 0.005 < x < fall[0] - 0.005]
            check(0 < op_in < 1 and 0.05 < fall[0] < 0.95 and len(mids) >= 3 and fall[-1] == 0 and all(a >= b for a, b in zip(fall, fall[1:])),
                  f"[{motion}] Esc mid-entrance turns back from {fall[0]:.2f} (at Esc-read {op_in:.2f}), {len(mids)} frames in between: {[round(x, 2) for x in fall[:6]]} ... {round(fall[-1], 2)}")
        check(gone == "none", f"[{motion}] closed after the exit: display {gone}")
        check(not errs, f"[{motion}] zero page errors {errs[:2]}")
        ctx.close()
    b.close()
print(f"{sum(res)} PASS, {len(res) - sum(res)} FAIL")
