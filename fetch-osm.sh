#!/bin/sh
# Fetch one airport's aeroways from OpenStreetMap into data/<icao>.osm.json.
#
# Run only to refresh a layout; the game itself never fetches. The raw answer
# is kept so every later extract is reproducible from it. Then rebuild:
#   cargo run -p twr-osm -- <ICAO>
#
# Aprons and terminals are not part of the route graph; they come along so the
# map can draw them later without a second fetch at a different OSM date.
set -eu
cd "$(dirname "$0")"
[ $# -eq 1 ] || { echo "usage: $0 <ICAO>   e.g. $0 VIDP" >&2; exit 2; }
icao=$(printf %s "$1" | tr a-z A-Z)
lower=$(printf %s "$icao" | tr A-Z a-z)
mkdir -p data
tmp="data/$lower.osm.json.tmp"
curl -sS --fail --max-time 180 -A 'twr-osm/0.1 (offline game, one fetch per airport)' \
  --data-urlencode data@- https://overpass-api.de/api/interpreter -o "$tmp" <<Q
[out:json][timeout:90];
area["icao"="$icao"]["aeroway"="aerodrome"]->.a;
(
  way(area.a)["aeroway"~"^(runway|taxiway|taxilane|parking_position|holding_position|apron|terminal)\$"];
  node(area.a)["aeroway"~"^(parking_position|holding_position|gate)\$"];
);
(._;>;);
out body qt;
Q
# Overpass answers a timeout with HTTP 200 and an empty element list. Keep
# the old file rather than replace a working layout with nothing.
if ! grep -q '"type": "way"' "$tmp"; then
  echo "Overpass returned no ways for $icao; kept the previous data/$lower.osm.json." >&2
  rm -f "$tmp"; exit 1
fi
mv "$tmp" "data/$lower.osm.json"
echo "wrote data/$lower.osm.json ($(wc -c < "data/$lower.osm.json") bytes)"
