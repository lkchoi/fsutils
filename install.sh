#!/bin/bash
set -e

REPO="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$HOME/bin"
mkdir -p "$BIN_DIR"

cargo build --release --manifest-path "$REPO/Cargo.toml"

for tool in fsutils mvsum ddup dss ssim fix-ext; do
    src="$REPO/target/release/$tool"
    dst="$BIN_DIR/$tool"
    ln -sf "$src" "$dst"
    echo "linked $dst -> $src"
done
