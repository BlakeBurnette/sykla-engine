#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "Building sykla-godot..."
cargo build --release -p sykla-godot

echo "Symlinking library into godot/bin/..."
mkdir -p "$REPO_ROOT/godot/bin"
ln -sf "$REPO_ROOT/target/debug/libsykla_godot.dylib" "$REPO_ROOT/godot/bin/libsykla_godot.debug.dylib"
ln -sf "$REPO_ROOT/target/release/libsykla_godot.dylib" "$REPO_ROOT/godot/bin/libsykla_godot.release.dylib"

echo "Done. Open godot/ in Godot 4.3+."
