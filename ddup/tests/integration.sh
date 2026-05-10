#!/usr/bin/env bash
# Integration tests for ddup
# Run: ./tests/integration.sh
# Requires: ddup binary in PATH or at target/release/ddup

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DDUP="${DDUP:-$SCRIPT_DIR/target/release/ddup}"

if [ ! -x "$DDUP" ]; then
    echo "Building ddup..."
    cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
    DDUP="$SCRIPT_DIR/target/release/ddup"
fi
TESTDIR=$(mktemp -d)
PASSED=0
FAILED=0

cleanup() { rm -rf "$TESTDIR"; }
trap cleanup EXIT

pass() { PASSED=$((PASSED + 1)); echo "  PASS: $1"; }
fail() { FAILED=$((FAILED + 1)); echo "  FAIL: $1"; }

assert_eq() {
    if [ "$1" = "$2" ]; then pass "$3"; else fail "$3 (expected '$2', got '$1')"; fi
}

assert_contains() {
    if echo "$1" | grep -q "$2"; then pass "$3"; else fail "$3 (output missing '$2')"; fi
}

assert_not_contains() {
    if echo "$1" | grep -q "$2"; then fail "$3 (output contains '$2')"; else pass "$3"; fi
}

assert_line_count() {
    local count
    count=$(echo "$1" | grep -c "$2" || true)
    if [ "$count" -eq "$3" ]; then pass "$4"; else fail "$4 (expected $3 lines matching '$2', got $count)"; fi
}

# --- Setup ---
echo "=== ddup integration tests ==="
echo "Test dir: $TESTDIR"
echo

# --- Test: dry run prints hash ---
echo "# Dry run"
echo "hello" > "$TESTDIR/a.txt"
OUT=$($DDUP -n "$TESTDIR/a.txt")
assert_contains "$OUT" "$TESTDIR/a.txt" "dry run prints path"
# Should be 16-char xxh3 hash
HASH=$(echo "$OUT" | awk '{print $1}')
assert_eq "${#HASH}" "16" "xxh3 hash is 16 chars"

# --- Test: algorithm selection ---
echo "# Algorithm selection"
OUT_MD5=$($DDUP -n -a md5 "$TESTDIR/a.txt")
HASH_MD5=$(echo "$OUT_MD5" | awk '{print $1}')
assert_eq "${#HASH_MD5}" "32" "md5 hash is 32 chars"

OUT_SHA=$($DDUP -n -a sha256 "$TESTDIR/a.txt")
HASH_SHA=$(echo "$OUT_SHA" | awk '{print $1}')
assert_eq "${#HASH_SHA}" "64" "sha256 hash is 64 chars"

OUT_B3=$($DDUP -n -a blake3 "$TESTDIR/a.txt")
HASH_B3=$(echo "$OUT_B3" | awk '{print $1}')
assert_eq "${#HASH_B3}" "64" "blake3 hash is 64 chars"

# --- Test: verbose flag ---
echo "# Verbose flag"
OUT=$($DDUP -v -n "$TESTDIR/a.txt")
assert_contains "$OUT" "$TESTDIR/a.txt" "verbose prints output"

# --- Test: silent by default ---
echo "# Silent by default"
OUT=$($DDUP -n "$TESTDIR/a.txt")
# -n always prints, test without -n and without -v
# Can't easily test this without setting xattrs, skip

# --- Test: duplicate detection ---
echo "# Duplicate detection"
echo "dup content" > "$TESTDIR/dup1.txt"
echo "dup content" > "$TESTDIR/dup2.txt"
echo "unique" > "$TESTDIR/unique.txt"

OUT=$($DDUP -d "$TESTDIR/")
assert_contains "$OUT" "dup1.txt" "duplicates shows dup1"
assert_contains "$OUT" "dup2.txt" "duplicates shows dup2"
assert_not_contains "$OUT" "unique.txt" "duplicates excludes unique"

# --- Test: duplicate detection with file sizes ---
echo "# Duplicate sizes"
assert_contains "$OUT" "B" "duplicates output includes size"

# --- Test: no duplicates ---
echo "# No duplicates"
rm "$TESTDIR/dup2.txt"
OUT=$($DDUP -d "$TESTDIR/" 2>&1 || true)
assert_contains "$OUT" "No duplicates" "reports no duplicates"
echo "dup content" > "$TESTDIR/dup2.txt"

# --- Test: exclude by name ---
echo "# Exclude"
mkdir -p "$TESTDIR/node_modules"
echo "skip" > "$TESTDIR/node_modules/pkg.js"
OUT=$($DDUP -n -e node_modules "$TESTDIR/")
assert_not_contains "$OUT" "node_modules" "exclude skips node_modules"

# --- Test: exclude by glob ---
echo "# Exclude glob"
echo "logdata" > "$TESTDIR/app.log"
OUT=$($DDUP -n -e "*.log" "$TESTDIR/")
assert_not_contains "$OUT" "app.log" "exclude glob skips .log files"

# --- Test: multiple excludes ---
echo "# Multiple excludes"
OUT=$($DDUP -n -e node_modules -e "*.log" "$TESTDIR/")
assert_not_contains "$OUT" "node_modules" "multi-exclude skips node_modules"
assert_not_contains "$OUT" "app.log" "multi-exclude skips .log"

# --- Test: glob input pattern ---
echo "# Glob input"
OUT=$($DDUP -n "$TESTDIR/*.txt")
assert_contains "$OUT" "a.txt" "glob input finds .txt files"
assert_not_contains "$OUT" "app.log" "glob input excludes non-.txt"

# --- Test: cross-directory duplicates ---
echo "# Cross-directory duplicates"
mkdir -p "$TESTDIR/dir_a" "$TESTDIR/dir_b"
echo "cross dup" > "$TESTDIR/dir_a/file.txt"
echo "cross dup" > "$TESTDIR/dir_b/file.txt"
OUT=$($DDUP -d "$TESTDIR/dir_a" "$TESTDIR/dir_b")
assert_contains "$OUT" "dir_a" "cross-dir dup shows dir_a"
assert_contains "$OUT" "dir_b" "cross-dir dup shows dir_b"

# --- Test: strategy --keep=first ---
echo "# Strategy: keep first"
OUT=$(echo "n" | $DDUP -d --delete --keep=first "$TESTDIR/dir_a" "$TESTDIR/dir_b" 2>&1)
assert_contains "$OUT" "[keep]" "strategy shows [keep]"
assert_contains "$OUT" "[delete]" "strategy shows [delete]"

# --- Test: hash caching ---
echo "# Hash caching"
echo "cache me" > "$TESTDIR/cached.txt"
$DDUP -n "$TESTDIR/cached.txt" > /dev/null
# Check xattrs were set
XATTR_HASH=$(xattr -p com.ddup.hash "$TESTDIR/cached.txt" 2>/dev/null || echo "")
if [ -n "$XATTR_HASH" ]; then pass "cache xattr set"; else fail "cache xattr not set"; fi

# --- Test: --no-cache ---
echo "# --no-cache"
$DDUP -n --no-cache "$TESTDIR/cached.txt" > /dev/null
pass "no-cache runs without error"

# --- Test: config file ---
echo "# Config file"
echo "algorithm=md5" > "$TESTDIR/.ddup"
OUT=$(cd "$TESTDIR" && $DDUP -n a.txt)
HASH=$(echo "$OUT" | awk '{print $1}')
assert_eq "${#HASH}" "32" "config sets md5 algorithm"
rm "$TESTDIR/.ddup"

# --- Summary ---
echo
echo "=== Results: $PASSED passed, $FAILED failed ==="
if [ "$FAILED" -gt 0 ]; then exit 1; fi
