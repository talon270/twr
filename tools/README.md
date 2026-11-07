# twr verification scripts

Playwright scripts, one per slice, that drive `dist/index.html` with real
clicks and keys and print one PASS or FAIL line per check. Build the page
first with `./build.sh`. Each script takes a folder for its screenshots. Give
it a scratch folder, not the project:

```sh
python3 tools/verify_b6.py /tmp/twr-shots
```

| Script | Checks | Last run |
|---|---|---|
| `verify_b2.py` | map, pan and zoom, click-to-taxi; the preview equals the route | 29/29, 8 of 8 runs, 2026-11-07 |
| `verify_b3.py` | push, hold short, crossings, line-up, takeoff | 22/22, 2026-11-07 |
| `verify_b4.py` | each conflict type in both themes | 35/35, 2026-11-07 |
| `verify_b5.py` | arrival from final to stand, go-arounds, R5 | 40/40, 2026-11-07 |
| `verify_b6.py` | modes, seed replay, traffic, shut-down, bar layout | 34/34, 2026-11-07 (B9 run) |
| `verify_b7.py` | score panel, result dialog, bests, v1 -> v2 storage | 22/22, 2026-11-07 |
| `verify_b9.py` | wind and flow, east flow end to end, fresh and seeded profiles, both themes at 1500/1920, reduced motion, no alert/confirm | 21/21, 2026-11-07 |
| `perf_80.py` | frame time with 80+ aircraft, parked and with 42 taxiing, real GPU | see PLAN-twr.md § B9 "Built" |

`verify_b2.py` and `verify_b3.py` draw a fresh seed each load, so their check
counts vary a little from run to run.
