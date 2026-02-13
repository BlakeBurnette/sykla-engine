#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Building sykla-web for wasm32 ==="
cargo build --target wasm32-unknown-unknown --release -p sykla-web

echo "=== Running wasm-bindgen ==="
wasm-bindgen --target web --out-dir "$ROOT/web/dist" \
    "$ROOT/target/wasm32-unknown-unknown/release/sykla_web.wasm"

# Optional: optimize with wasm-opt if available
if command -v wasm-opt &> /dev/null; then
    echo "=== Optimizing with wasm-opt ==="
    WASM_FILE="$ROOT/web/dist/sykla_web_bg.wasm"
    wasm-opt -O3 "$WASM_FILE" -o "$WASM_FILE"
    echo "Optimized: $(du -h "$WASM_FILE" | cut -f1)"
else
    echo "wasm-opt not found — skipping optimization"
fi

echo "=== Build complete ==="
echo "Serve web/ with any static HTTP server, e.g.:"
echo "  python3 -m http.server 8080 -d $ROOT/web"
