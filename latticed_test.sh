#!/usr/bin/env bash
# latticed_test.sh — integration test for latticed

set -euo pipefail
set -x

BINARY="./target/debug/latticed"
LOG="/tmp/latticed_test.log"
PASS=0
FAIL=0

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}[PASS]${NC} $1"; PASS=$((PASS+1)); }
fail() { echo -e "${RED}[FAIL]${NC} $1"; FAIL=$((FAIL+1)); }
info() { echo -e "${YELLOW}[INFO]${NC} $1"; }

# ── sanity check ────────────────────────────────────────────────
info "Building latticed..."
if cargo build 2>/dev/null; then
    pass "cargo build succeeded"
else
    fail "build failed"
    exit 1
fi

# ── start daemon (binary already built, just run it as root) ────
info "Starting latticed daemon..."
sudo "$BINARY" > "$LOG" 2>&1 &
DAEMON_PID=$!
sleep 1

if sudo kill -0 "$DAEMON_PID" 2>/dev/null; then
    pass "Daemon started (PID $DAEMON_PID)"
else
    fail "Daemon failed to start"
    cat "$LOG"
    exit 1
fi

# ── trigger filesystem events ───────────────────────────────────
info "Triggering filesystem events..."
sleep 1

sudo touch /etc/latticed_test_file
sleep 0.5
echo "latticed probe" | sudo tee /etc/latticed_test_file > /dev/null
sleep 0.5
sudo rm /etc/latticed_test_file
sleep 1

# ── check daemon is still alive ─────────────────────────────────
if sudo kill -0 "$DAEMON_PID" 2>/dev/null; then
    pass "Daemon still running after events"
else
    fail "Daemon crashed during event handling"
fi

# ── check blocks were created ───────────────────────────────────
BLOCK_COUNT=$(grep -c "Block #" "$LOG" || true)
if [ "$BLOCK_COUNT" -gt 0 ]; then
    pass "Detected $BLOCK_COUNT block(s) appended"
else
    fail "No blocks were appended — watcher may not be firing"
fi

# ── check hash format (64 char hex) ─────────────────────────────
HASH_CHECK=$(grep -oP '(?<=\| )[a-f0-9]{64}' "$LOG" | head -1)
if [ -n "$HASH_CHECK" ]; then
    pass "Hash format valid: ${HASH_CHECK:0:16}..."
else
    fail "No valid SHA-256 hash found in output"
fi

# ── check each block has unique hash ────────────────────────────
TOTAL_HASHES=$(grep -oP '(?<=\| )[a-f0-9]{64}' "$LOG" | wc -l)
UNIQUE_HASHES=$(grep -oP '(?<=\| )[a-f0-9]{64}' "$LOG" | sort -u | wc -l)
if [ "$TOTAL_HASHES" -eq "$UNIQUE_HASHES" ] && [ "$TOTAL_HASHES" -gt 0 ]; then
    pass "All $TOTAL_HASHES hashes are unique"
else
    fail "Duplicate hashes detected — collision or hashing bug"
fi

# ── cleanup ─────────────────────────────────────────────────────
info "Stopping daemon..."
if sudo kill "$DAEMON_PID" 2>/dev/null; then
    pass "Daemon stopped cleanly"
fi
sleep 0.5

# ── summary ─────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "  Results: ${GREEN}$PASS passed${NC} | ${RED}$FAIL failed${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

[ "$FAIL" -eq 0 ] && exit 0 || exit 1
