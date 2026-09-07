#!/usr/bin/env bash
set -euo pipefail
binary="$(cd "$(dirname "$0")" && pwd)/target/debug/latticed"
fixture=$(mktemp -d)
daemon_pid=
cleanup() {
    if [[ -n "$daemon_pid" ]]; then
        kill -TERM "$daemon_pid" 2>/dev/null || true
        for _ in {1..30}; do
            kill -0 "$daemon_pid" 2>/dev/null || break
            sleep 0.1
        done
        kill -KILL "$daemon_pid" 2>/dev/null || true
        wait "$daemon_pid" 2>/dev/null || true
    fi
    rm -rf "$fixture"
}
trap cleanup EXIT
mkdir "$fixture/watched"
"$binary" --help >/dev/null
"$binary" --storage-dir "$fixture/store" keygen >"$fixture/keygen.log"
"$binary" --storage-dir "$fixture/store" start --watch "$fixture/watched" >"$fixture/daemon.log" 2>&1 &
daemon_pid=$!
wait_for() {
    for _ in {1..100}; do
        kill -0 "$daemon_pid" 2>/dev/null || { cat "$fixture/daemon.log"; return 1; }
        if grep -q "$1" "$fixture/daemon.log"; then return 0; fi
        sleep 0.1
    done
    cat "$fixture/daemon.log"
    return 1
}
wait_for 'Watching configured paths'
echo probe >"$fixture/watched/probe"
wait_for 'Block #'
kill -TERM "$daemon_pid"
for _ in {1..100}; do
    kill -0 "$daemon_pid" 2>/dev/null || break
    sleep 0.1
done
if kill -0 "$daemon_pid" 2>/dev/null; then
    echo 'Daemon failed to exit after SIGTERM' >&2
    exit 1
fi
wait "$daemon_pid"
daemon_pid=
grep -q 'Final checkpoint written' "$fixture/daemon.log"
"$binary" --storage-dir "$fixture/store" verify >"$fixture/verify.log"
grep -q 'Chain OK' "$fixture/verify.log"
# Changing a recorded filename must invalidate its signed chain head.
sed -i 's/probe/tampered/g' "$fixture/store/chain.jsonl"
if "$binary" --storage-dir "$fixture/store" verify >"$fixture/tamper.log" 2>&1; then
    echo 'Tampered chain was accepted' >&2
    exit 1
fi
grep -q 'TAMPER DETECTED' "$fixture/tamper.log"
echo 'Event recording, signed shutdown, verification and tamper rejection passed.'
