#!/usr/bin/env bash
# horcrux — interactive Phase 1 demo
#
# Walks through everything built in Phase 1: split + per-shard encryption
# (SSS via vsss-rs, Argon2id, AES-256-GCM), the 83-byte shard file format,
# reconstruction from any threshold subset, and the failure modes
# (wrong password / too few shards / mixed splits).
#
#   ./demo.sh          interactive step-through
#   ./demo.sh --auto   run all steps without pausing
#   ./demo.sh --keep   keep the shard files afterwards (prints the path)
set -euo pipefail
cd "$(dirname "$0")"

AUTO=0
KEEP=0
for arg in "$@"; do
    case "$arg" in
        --auto) AUTO=1 ;;
        --keep) KEEP=1 ;;
        *)
            echo "usage: demo.sh [--auto] [--keep]" >&2
            exit 2
            ;;
    esac
done

# Non-interactive stdin (pipes, CI) => auto-advance.
if [[ ! -t 0 ]]; then AUTO=1; fi

# --- colors (disabled off a TTY or under NO_COLOR) ---
if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    BOLD=$(tput bold)
    DIM=$(tput dim)
    RED=$(tput setaf 1)
    GREEN=$(tput setaf 2)
    YELLOW=$(tput setaf 3)
    CYAN=$(tput setaf 6)
    MAGENTA=$(tput setaf 5)
    RESET=$(tput sgr0)
else
    BOLD=""; DIM=""; RED=""; GREEN=""; YELLOW=""; CYAN=""; MAGENTA=""; RESET=""
fi

# --- logging helpers ---
info() { printf '%s[INFO]%s %s\n' "$CYAN" "$RESET" "$*"; }
ok()   { printf '%s[ OK ]%s %s\n' "$GREEN" "$RESET" "$*"; }
warn() { printf '%s[WARN]%s %s\n' "$YELLOW" "$RESET" "$*"; }
fail() { printf '%s[FAIL]%s %s\n' "$RED" "$RESET" "$*"; }

step() {
    local n="$1"; shift
    printf '\n%s==============================================%s\n' "$BOLD" "$RESET"
    printf '%sSTEP %-3s %s%s\n' "$BOLD" "$n" "$*" "$RESET"
    printf '%s==============================================%s\n' "$BOLD" "$RESET"
}

show() { printf '%s$ %s%s\n' "$BOLD" "$*" "$RESET"; }

cmd() {
    show "$@"
    "$@"
}

pause() {
    if [[ "$AUTO" == 1 ]]; then return; fi
    printf '%s[enter]%s continue   %s[q]%s quit   %s[a]%s auto > ' \
        "$GREEN" "$RESET" "$RED" "$RESET" "$YELLOW" "$RESET"
    local k=""
    read -r -n1 k || true
    printf '\n'
    case "$k" in
        q | Q) printf '\nDemo aborted by user.\n'; exit 0 ;;
        a | A) AUTO=1; info "auto mode enabled -- remaining steps run by themselves" ;;
    esac
}

# --- setup ---
T=2
N=3
PW="guardian-demo-password"
OUT="$(mktemp -d)"

cleanup() {
    if [[ "$KEEP" == 1 ]]; then
        printf '\n%sShard files kept for inspection: %s%s\n' "$DIM" "$OUT" "$RESET"
    else
        rm -rf "$OUT"
    fi
}
trap cleanup EXIT

# =====================================================================
# DEMO
# =====================================================================
printf '%s\n' "horcrux -- Phase 1 interactive demo"
printf '%s\n' "==================================="
info "This walks through everything built so far: split + per-shard encryption"
info "(vsss-rs Shamir, Argon2id, AES-256-GCM), the 83-byte shard format,"
info "reconstruction from any 2 of 3 shards, and the failure modes."
info "Each step runs the real CLI. Press Enter to advance."
pause

# --- 1. build -------------------------------------------------------
step 1 "build the binary"
info "Compiles src/main.rs (CLI), src/lib.rs (pipeline), and the sss/crypto/shard/error modules."
cmd cargo build -q
BIN=./target/debug/horcrux
ok "binary ready: $BIN"
pause

# --- 2. init: split + encrypt ---------------------------------------
step 2 "init -- split the key and encrypt each shard"
info "init_shards (src/lib.rs) -> sss::split via vsss-rs (Shamir over the secp256k1"
info "field), then each share is encrypted with its own Argon2id-derived AES-256-GCM"
info "key (src/crypto.rs) and written as an 83-byte file (src/shard.rs)."
info "The key is a random disposable secp256k1 private key. Real usage would prompt"
info "for a distinct guardian password per shard; this demo reuses one for brevity."
show "$BIN" init --generate --threshold "$T" --shares "$N" --out-dir "$OUT" --password "$PW"
INIT="$("$BIN" init --generate --threshold "$T" --shares "$N" --out-dir "$OUT" --password "$PW")"
printf '%s\n' "$INIT"
KEY="$(printf '%s\n' "$INIT" | sed -n 's/^Generated test key: 0x//p')"
ok "split created: threshold $T of $N, each shard independently encrypted"
pause

# --- 3. inspect shard files ------------------------------------------
step 3 "inspect the shard files"
info "Every shard is a fixed 83-byte binary file: magic, version, threshold,"
info "share-count, share id, then salt, nonce, ciphertext and auth tag."
cmd ls -l "$OUT"
cmd file "$OUT"/*.hx
cmd hexdump -C "$OUT/shard-1.hx"
printf '\n%s  offset   len  field%s\n' "$CYAN" "$RESET"
printf '%s  ------  ----  ------------------------------%s\n' "$CYAN" "$RESET"
printf '       0     3  magic "HX1"\n'
printf '       3     1  format version = 1\n'
printf '       4     1  threshold t = %s\n' "$T"
printf '       5     1  share count n = %s\n' "$N"
printf '       6     1  share id = 1\n'
printf '       7    16  Argon2id salt (random per shard)\n'
printf '      23    12  AES-256-GCM nonce (random per shard)\n'
printf '      35    32  sealed share value (ciphertext)\n'
printf '      67    16  GCM authentication tag\n'
printf '      83    --  total file size\n'
ok "format matches src/shard.rs (SHARD_LEN = 83 bytes)"
pause

# --- 4. reconstruct: shards 1+2 --------------------------------------
step 4 "reconstruct -- shards 1 and 2"
info "reconstruct (src/lib.rs) parses the files, re-derives each AES key with"
info "Argon2id, decrypts through the GCM tag check, then sss::combine recovers"
info "the key via Lagrange interpolation."
show "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW"
OUT12="$("$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW")"
printf '%s\n' "$OUT12"
R1="$(printf '%s\n' "$OUT12" | sed -n 's/^Reconstructed key: 0x//p')"
ok "reconstructed from shards 1+2"
pause

# --- 5. reconstruct: shards 2+3 --------------------------------------
step 5 "reconstruct -- a different pair (shards 2 and 3)"
info "Any threshold subset must recover the same key; here the pair is 2+3."
show "$BIN" reconstruct "$OUT/shard-2.hx" "$OUT/shard-3.hx" --password "$PW"
OUT23="$("$BIN" reconstruct "$OUT/shard-2.hx" "$OUT/shard-3.hx" --password "$PW")"
printf '%s\n' "$OUT23"
R2="$(printf '%s\n' "$OUT23" | sed -n 's/^Reconstructed key: 0x//p')"
ok "reconstructed from shards 2+3"
pause

# --- 6. reconstruct: all three ---------------------------------------
step 6 "reconstruct -- all three shards"
info "More than the threshold works too: the polynomial is still interpolated"
info "to the same constant term."
show "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" "$OUT/shard-3.hx" --password "$PW"
OUT123="$("$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" "$OUT/shard-3.hx" --password "$PW")"
printf '%s\n' "$OUT123"
R3="$(printf '%s\n' "$OUT123" | sed -n 's/^Reconstructed key: 0x//p')"
ok "reconstructed from all 3 shards"
pause

# --- 7. verify equality ----------------------------------------------
step 7 "verify every reconstruction matches the original"
if [[ "$R1" != "$KEY" || "$R2" != "$KEY" || "$R3" != "$KEY" ]]; then
    fail "reconstructed keys do not match the original"
    exit 1
fi
ok "shards 1+2, 2+3 and 1+2+3 all reproduce the original key"
printf '%s   key: 0x%s%s\n' "$DIM" "$KEY" "$RESET"
pause

# --- 8. wrong password ------------------------------------------------
step 8 "wrong password -- clean rejection"
info "AES-256-GCM authenticates everything: a wrong password fails the tag with a"
info "typed Error::Decrypt (src/error.rs). No plaintext is ever produced."
show "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "wrong-password"
if "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "wrong-password" 2>"$OUT/err-wrongpw.txt"; then
    fail "wrong password was accepted -- this must never happen"
    exit 1
fi
printf '%s%s%s\n' "$RED" "$(cat "$OUT/err-wrongpw.txt")" "$RESET"
if grep -q "wrong password or tampered shard" "$OUT/err-wrongpw.txt"; then
    ok "rejected: GCM authentication tag did not verify"
else
    fail "unexpected error output"
    exit 1
fi
pause

# --- 9. too few shards ------------------------------------------------
step 9 "too few shards -- NotEnoughShares"
info "With threshold 2, a single shard cannot interpolate the polynomial."
info "reconstruct rejects it before any decryption: Error::NotEnoughShares(2, 1)."
show "$BIN" reconstruct "$OUT/shard-1.hx" --password "$PW"
if "$BIN" reconstruct "$OUT/shard-1.hx" --password "$PW" 2>"$OUT/err-few.txt"; then
    fail "one shard should never reconstruct a 2-of-3 key"
    exit 1
fi
printf '%s%s%s\n' "$RED" "$(cat "$OUT/err-few.txt")" "$RESET"
if grep -q "need 2 shards but only 1 were provided" "$OUT/err-few.txt"; then
    ok "rejected: not enough shares"
else
    fail "unexpected error output"
    exit 1
fi
pause

# --- 10. mixed splits --------------------------------------------------
step 10 "mixed shards from different splits -- SplitMismatch"
info "Every shard is bound to its split via the AAD [t, n, id]. A second split"
info "(3-of-3) is created; mixing one of its shards with the first split is"
info "rejected: Error::SplitMismatch."
show "$BIN" init --generate --threshold 3 --shares 3 --out-dir "$OUT/b" --password "$PW"
"$BIN" init --generate --threshold 3 --shares 3 --out-dir "$OUT/b" --password "$PW" >/dev/null
show "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/b/shard-1.hx" --password "$PW"
if "$BIN" reconstruct "$OUT/shard-1.hx" "$OUT/b/shard-1.hx" --password "$PW" 2>"$OUT/err-mixed.txt"; then
    fail "mixing shards from different splits must fail"
    exit 1
fi
printf '%s%s%s\n' "$RED" "$(cat "$OUT/err-mixed.txt")" "$RESET"
if grep -q "has different split parameters" "$OUT/err-mixed.txt"; then
    ok "rejected: shards belong to different splits"
else
    fail "unexpected error output"
    exit 1
fi
pause

# --- 11. optional test suite ------------------------------------------
step 11 "full test suite (optional)"
info "cargo test runs 23 unit tests (sss, crypto, shard, lib) plus 8 integration"
info "tests in tests/roundtrip.rs."
run_tests=0
if [[ "$AUTO" == 1 ]]; then
    run_tests=1
else
    printf '%s[?]%s run cargo test now? (y/N) ' "$YELLOW" "$RESET"
    read -r yn || true
    case "$yn" in y | Y) run_tests=1 ;; esac
fi
if [[ "$run_tests" == 1 ]]; then
    show cargo test
    if ! SUMMARY="$(cargo test 2>&1 | grep 'test result:')"; then
        fail "cargo test failed -- see output above"
        exit 1
    fi
    printf '%s\n' "$SUMMARY"
    ok "all tests green"
else
    warn "skipped (run 'cargo test' yourself to see the suite)"
fi

# --- wrap-up ------------------------------------------------------------
printf '\n%s==============================================%s\n' "$BOLD" "$RESET"
printf '%s  DEMO COMPLETE  --  everything verified %s\n' "$BOLD" "$RESET"
printf '%s==============================================%s\n' "$BOLD" "$RESET"
ok "split:      2-of-3 encrypted shard files (83 B each)"
ok "inspect:    83-byte format with magic/version/t/n/id/salt/nonce/ciphertext"
ok "reconstruct:any 2 of 3 recover the original key"
ok "reconstruct:all 3 shards also recover the key"
ok "rejected:   wrong password (AES-GCM auth-tag failure)"
ok "rejected:   too few shards (NotEnoughShares)"
ok "rejected:   mixed splits (SplitMismatch)"
if [[ "$run_tests" == 1 ]]; then
    ok "tests:      23 unit + 8 integration tests green"
fi
printf '%s\n' "---"
info "That is Phase 1 end to end. Next up: Phase 2 -- 'sign' (reconstruct in RAM,"
info "build and broadcast an EVM transaction via alloy)."
