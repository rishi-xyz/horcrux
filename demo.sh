#!/usr/bin/env bash
# Phase 1 demo: split a disposable key into encrypted shards, reconstruct it
# from any threshold subset, and show a wrong password failing cleanly.
set -euo pipefail
cd "$(dirname "$0")"

OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
PW="guardian-demo-password"

echo "Building horcrux..."
cargo build -q
BIN=./target/debug/horcrux

echo
echo ">> 1. Split: generate a disposable test key into 3 encrypted shards (2-of-3)"
INIT="$("$BIN" init --generate --threshold 2 --shares 3 --out-dir "$OUT" --password "$PW")"
echo "$INIT"
KEY="$(printf '%s\n' "$INIT" | sed -n 's/^Generated test key: 0x//p')"

echo
echo ">> 2. Reconstruct with shards 1 and 2"
R1="$("$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" | sed -n 's/^Reconstructed key: 0x//p')"
echo "reconstructed: 0x$R1"

echo
echo ">> 3. Reconstruct with a different pair (shards 2 and 3)"
R2="$("$BIN" reconstruct "$OUT/shard-2.hx" "$OUT/shard-3.hx" --password "$PW" | sed -n 's/^Reconstructed key: 0x//p')"
echo "reconstructed: 0x$R2"

echo
echo ">> 4. Verify both reconstructions match the original key"
if [ "$R1" != "$KEY" ] || [ "$R2" != "$KEY" ]; then
    echo "FAIL: reconstructed keys do not match the original" >&2
    exit 1
fi
echo "OK: 0x$R1 == original"

echo
echo ">> 5. Feed a wrong password and watch decryption fail cleanly"
if "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "not-the-password" 2>"$OUT/err.txt"; then
    echo "FAIL: wrong password should have been rejected" >&2
    exit 1
fi
if grep -q "wrong password or tampered shard" "$OUT/err.txt"; then
    echo "OK: rejected with an AES-GCM authentication-tag error"
else
    echo "FAIL: unexpected error output" >&2
    cat "$OUT/err.txt" >&2
    exit 1
fi

echo
echo "ALL DEMO CHECKS PASSED"
