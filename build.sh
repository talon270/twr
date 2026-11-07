#!/bin/sh
# Build twr into one self-contained page: dist/index.html.
#
# The wasm is inlined as base64 and started with initSync, because Chrome
# refuses to fetch() a .wasm file from file:// (PLAN-twr.md § A4). The page
# makes no network request of any kind.
#
# Needs: cargo with the wasm32-unknown-unknown target, wasm-bindgen-cli at the
# exact version in Cargo.lock, wasm-opt (binaryen). Run from anywhere.
set -eu
cd "$(dirname "$0")"
# cargo install puts wasm-bindgen here; not every shell has it on PATH.
PATH="$HOME/.cargo/bin:$PATH"

TARGET=wasm32-unknown-unknown
OUT=target/bindgen
SHELL_HTML=web/shell.html
MARKER='<!--BUNDLE-->'

# wasm-bindgen's CLI and library must match exactly, or the glue it writes
# silently disagrees with the module. Check before building, not after.
lib_ver=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[^0-9.]/, ""); print; exit }' Cargo.lock 2>/dev/null || true)
cli_ver=$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')
if [ -n "$lib_ver" ] && [ "$lib_ver" != "$cli_ver" ]; then
  echo "wasm-bindgen CLI is '${cli_ver:-missing}', Cargo.lock wants $lib_ver." >&2
  echo "Fix: cargo install wasm-bindgen-cli --version $lib_ver --locked" >&2
  exit 1
fi

cargo build --release --target "$TARGET" -p twr-web
rm -rf "$OUT" && mkdir -p "$OUT" dist
wasm-bindgen --target no-modules --no-typescript --out-dir "$OUT" \
  "target/$TARGET/release/twr_web.wasm"

# The feature flags name what current rustc emits by default for wasm32;
# wasm-opt rejects a module that uses a feature it was not told about.
wasm-opt -O3 \
  --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  "$OUT/twr_web_bg.wasm" -o "$OUT/twr_web_bg.opt.wasm"

glue="$OUT/twr_web.js"
# A literal </script> inside the glue would end the inline script early.
if grep -qi '</script' "$glue"; then
  echo "Glue contains </script; refusing to inline." >&2
  exit 1
fi
if [ "$(grep -c "$MARKER" "$SHELL_HTML")" -ne 1 ]; then
  echo "$SHELL_HTML must contain exactly one $MARKER line." >&2
  exit 1
fi

page=dist/index.html
tmp="$page.tmp"
{
  sed "/$MARKER/,\$d" "$SHELL_HTML"
  printf '<script>\n'
  cat "$glue"
  printf '</script>\n<script>TWR.boot("'
  base64 -w0 "$OUT/twr_web_bg.opt.wasm"
  printf '");</script>\n'
  sed "1,/$MARKER/d" "$SHELL_HTML"
} > "$tmp"
mv "$tmp" "$page"

raw=$(wc -c < "target/$TARGET/release/twr_web.wasm")
opt=$(wc -c < "$OUT/twr_web_bg.opt.wasm")
total=$(wc -c < "$page")
echo "built $page: $total bytes (wasm $raw -> $opt after wasm-opt)"
