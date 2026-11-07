#!/bin/sh
# Run one twr research brief through a headless Claude Code agent.
#
#   research/run.sh <topic> [--dry-run]       topic: vidp | aircraft | ui
#
# The agent gets the shared rules (briefs/_rules.md) plus the topic's brief,
# web search and fetch, read access to the project, node and curl, and write
# access only under research/out, research/work and research/sources. Anything
# else it tries is refused without a prompt (dontAsk), so a run can be left
# alone. It can't edit data/, crates/ or the plan.
#
# Re-running a topic extends its earlier output rather than starting over; the
# rules tell the agent to read what is already in research/out/<topic>/ first.
#
# Optional environment:
#   TWR_RESEARCH_MODEL   model alias or id (default: your Claude Code default)
#   TWR_RESEARCH_EFFORT  low | medium | high | xhigh | max (default: high)
#   TWR_RESEARCH_BUDGET  stop after this many USD; only applies to API-key auth
set -eu
cd "$(dirname "$0")/.."

usage() { echo "usage: research/run.sh <vidp|aircraft|ui> [--dry-run]" >&2; exit 2; }
[ $# -ge 1 ] || usage
topic=$1
dry=${2:-}
brief="research/briefs/$topic.md"
case $topic in _*) usage ;; esac
[ -f "$brief" ] || { echo "no brief at $brief" >&2; usage; }
command -v claude >/dev/null || { echo "claude CLI not on PATH" >&2; exit 1; }

out="research/out/$topic"
stamp=$(date +%Y%m%d-%H%M%S)
log="$out/logs/run-$stamp.log"

prompt=$(cat research/briefs/_rules.md; printf '\n\n---\n\n'; cat "$brief"; \
  printf '\n\nToday is %s. Begin.\n' "$(date +%Y-%m-%d)")

# One comma-separated line: the CLI does not split on newlines, and a merged
# rule silently stops matching (every Write was denied in the first test).
# Edit(path) rules cover Write too; paths are relative to the project root.
# Bash is limited to fetching and computing: no rm, no git, no installs.
# The fence is against accidents, not a sandbox: a node script or curl -o can
# still reach any path this user can write.
tools="WebSearch,WebFetch,Read,Glob,Grep,Edit(./research/out/**),Edit(./research/work/**),Bash(curl:*),Bash(node research/work/*),Bash(mkdir -p research/*),Bash(ls:*),Bash(wc:*),Bash(head:*),Bash(file:*)"

# --strict-mcp-config with no --mcp-config loads no MCP servers: the research
# writes files here, not into Drive or a hosted doc.
set -- -p --permission-mode dontAsk --allowedTools "$tools" --strict-mcp-config \
  --effort "${TWR_RESEARCH_EFFORT:-high}" --name "twr-research-$topic" \
  --output-format text
[ -n "${TWR_RESEARCH_MODEL:-}" ] && set -- "$@" --model "$TWR_RESEARCH_MODEL"
[ -n "${TWR_RESEARCH_BUDGET:-}" ] && set -- "$@" --max-budget-usd "$TWR_RESEARCH_BUDGET"

if [ "$dry" = "--dry-run" ]; then
  echo "would run: claude $*  <prompt: $(printf %s "$prompt" | wc -c) bytes>"
  echo "log: $log"
  exit 0
fi

mkdir -p "$out/logs" "research/work/$topic" "research/sources/$topic"
echo "research $topic started $(date '+%H:%M'); log: $log"
echo "Expect tens of minutes. Files appear in $out/ as the agent writes them."
printf %s "$prompt" | claude "$@" 2>&1 | tee "$log"
echo "research $topic finished $(date '+%H:%M'); files:"
ls -la "$out"
