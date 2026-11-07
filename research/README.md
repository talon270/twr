# twr research

Five research briefs, each run by a headless Claude Code agent that searches the
web, reads primary documents, and writes sourced findings into `out/<topic>/`.
Every figure it writes carries a source and a grade (`primary` → `authored`), and
a gap is written as a gap rather than filled with a guess (`briefs/_rules.md`).

## Run

From the project root:

```sh
research/run.sh vidp        # IGI airfield: runways, taxiways, holds, stands, markings, operations
research/run.sh aircraft    # types at Delhi: dimensions, shapes, performance, operators, traffic
research/run.sh ui          # interface: real tower systems, ATC games, input speed, alerting
research/run.sh separation  # runway, wake and ground rules as codeable minima + B4 test cases
research/run.sh phraseology # real RT phrases, readbacks, and a grammar for the typed line
research/run.sh vidp --dry-run   # print the command without running it
```

Each run takes tens of minutes and uses your Claude Code plan. Re-running a topic
extends its earlier output instead of starting over. The runs are independent;
run them in separate terminals to go in parallel.

| Variable | Default | Effect |
|---|---|---|
| `TWR_RESEARCH_MODEL` | your Claude Code default | model alias or id |
| `TWR_RESEARCH_EFFORT` | `high` | `low` … `max` |
| `TWR_RESEARCH_BUDGET` | none | USD cap; only applies with API-key auth |

## What each run produces

| Topic | Files in `out/<topic>/` | Feeds |
|---|---|---|
| `vidp` | `runways.csv`, `taxiways.csv`, `holds.csv`, `stands.csv`, `overrides.proposed.toml`, `markings.md`, `operations.md` | `data/vidp.overrides.toml` (by hand, after you check it), map drawing, runway flows |
| `aircraft` | `types.csv`, `geometry/<icao>.json`, `discrepancies.csv`, `operators.csv`, `movements.md`, `sim-constants.md` | per-type sim values, aircraft silhouettes, traffic generator (B6) |
| `ui` | `real-systems.csv`, `games.csv`, `klm.md`, `wireframes.md` | `PLAN-twr.md` § Input and § Look |
| `separation` | `rules.csv`, `wake.csv`, `incidents.md`, `test-cases.md` | B4 conflict model and its tests, Surge end condition |
| `phraseology` | `phrases.csv`, `numbers-and-names.md`, `grammar.md`, `readback.md` | typed command line, autocomplete, radio log |

Every topic also writes `REPORT.md` and `sources.csv`.

## What the agent can and cannot touch

**Writes are fenced to `research/out/` and `research/work/`.** The run uses
`dontAsk` mode with an allowlist, so anything outside it is refused without a
prompt and the run can be left alone. Tested 2026-11-05: writes to `data/` and
`research/` itself, `rm`, and `node -e` were all refused; writes under `out/` and
`work/`, `node` on a script in `work/`, and `curl` downloads succeeded. MCP
servers are not loaded.

The fence guards against accidents, not malice: a node script or `curl -o` can
still reach any path your user can write.

**`overrides.proposed.toml` is a proposal.** The agent never edits
`data/vidp.overrides.toml`. Read the chart evidence comment on each entry, then
copy the ones you accept.

`sources/` (downloaded PDFs) and `work/` (scratch) are gitignored, as are run
logs.
