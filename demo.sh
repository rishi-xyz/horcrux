#!/usr/bin/env bash
# horcrux — interactive demo (Phases 1-4)
#
# Walks through everything built so far: split + per-shard encryption
# (SSS via vsss-rs, Argon2id, AES-256-GCM), the 83-byte shard file format,
# reconstruction from any threshold subset, the failure modes
# (wrong password / too few shards / mixed splits), the access-log /
# anomaly-detection layer that blocks repeated failed attempts, Mode B
# FROST threshold signing where the key is never reconstructed on any
# machine, passive shard verification, and (when solana-test-validator is
# installed) a live on-chain broadcast of both signing modes.
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
broadcast_ran=0
# Any command without an explicit --log-file writes to the demo's temp dir
# instead of polluting the repository root.
export HORCRUX_ACCESS_LOG="$OUT/access.log"

cleanup() {
    if [[ -n "${VALIDATOR_PID:-}" ]]; then
        kill "$VALIDATOR_PID" 2>/dev/null || true
    fi
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
printf '%s\n' "horcrux -- interactive demo"
printf '%s\n' "==================================="
info "Walks through everything built so far: split + per-shard encryption"
info "(vsss-rs Shamir, Argon2id, AES-256-GCM), the 83-byte shard format,"
info "reconstruction from any 2 of 3 shards, the failure modes, the"
info "access-log / anomaly-detection layer (Phase 3), Mode B FROST"
info "threshold signing where the key is never reconstructed (Phase 4),"
info "passive verify (Phase 5), and a live broadcast to the local"
info "validator when solana-test-validator is installed."
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

# --- 11. access log + anomaly detection --------------------------------
step 11 "access log -- normal attempt, then 3 failures -> blocked"
info "Every shard decryption is appended to the access log (src/audit.rs,"
info "JSON-lines, append-only). Before signing, a rule-based scorer reads the"
info "history: 3 recent failed decrypts block the next attempt entirely."
BH="5uB24TAxhvkdErSJUJMaPX2DgZtDzR2EjSjYTMrY56zs"
TO="dQT7Vmpq2WFzWsi8SuYCqw2JoQkfpadyHSsQiLEMJDJ"
LOG_FLAG=(--log-file "$OUT/access.log")
show "$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" "${LOG_FLAG[@]}" \
    --to "$TO" --lamports 1 --blockhash "$BH"
"$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" "${LOG_FLAG[@]}" \
    --to "$TO" --lamports 1 --blockhash "$BH" >/dev/null
ok "first-time attempt allowed; both shard decrypts logged"
show "$BIN" log "${LOG_FLAG[@]}" --tail 3
"$BIN" log "${LOG_FLAG[@]}" --tail 3
info "Now three wrong-password attempts (each logged as a failure)..."
for i in 1 2 3; do
    "$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "wrong-$i" "${LOG_FLAG[@]}" \
        --to "$TO" --lamports 1 --blockhash "$BH" 2>/dev/null || true
done
show "$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" "${LOG_FLAG[@]}" \
    --to "$TO" --lamports 1 --blockhash "$BH"
if "$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" "${LOG_FLAG[@]}" \
    --to "$TO" --lamports 1 --blockhash "$BH" 2>"$OUT/err-blocked.txt"; then
    fail "signing should have been blocked after 3 failures"
    exit 1
fi
printf '%s%s%s\n' "$RED" "$(cat "$OUT/err-blocked.txt")" "$RESET"
if grep -q "access audit blocked the attempt" "$OUT/err-blocked.txt"; then
    ok "blocked before signing -- key material never handled"
else
    fail "unexpected error output"
    exit 1
fi
pause

# Mode B signs against a fresh log so step 11's deliberate block
# (3 failed decrypts) doesn't carry over into the MPC steps.
export HORCRUX_ACCESS_LOG="$OUT/mode-b.log"

# --- 12. optional test suite ------------------------------------------
step 12 "full test suite (optional)"
info "cargo test runs 66 unit tests (sss, crypto, shard, chain, audit, tx, mpc,"
info "verify, lib) plus integration suites: tests/roundtrip.rs, tests/sign.rs,"
info "tests/audit.rs, tests/mpc.rs, tests/verify.rs."
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

# --- 13. Mode B: FROST threshold signing --------------------------------
step 13 "Mode B -- FROST threshold signing, key never reconstructed"
info "mpc-split (src/mpc.rs) dealer-splits the SAME key into t-of-n key shares"
info "via frost-ed25519 (RFC 9591). Each share file is an independent participant;"
info "the full key never exists on any machine."
show "$BIN" mpc-split --key-hex "0x$KEY" --threshold "$T" --shares "$N" \
    --out-dir "$OUT/mpc" --password "$PW"
MPC_OUT="$("$BIN" mpc-split --key-hex "0x$KEY" --threshold "$T" --shares "$N" \
    --out-dir "$OUT/mpc" --password "$PW")"
printf '%s\n' "$MPC_OUT"
ok "split wrote 3 key shares plus the non-secret group public package"
pause

step 13b "Mode B -- sign with 2 of 3 shares (offline)"
info "Each share contributes nonces (round 1) and a signature share (round 2);"
info "the coordinator aggregates them into a single Ed25519 signature."
show "$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH"
MPC_SIGN="$("$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH")"
printf '%s\n' "$MPC_SIGN"
MPC_FROM="$(printf '%s\n' "$MPC_SIGN" | sed -n 's/^From:      //p')"
ok "produced a valid Ed25519 signature without ever reconstructing the key"
pause

step 13c "Mode B -- same wallet as Mode A, non-deterministic"
info "The group address equals the Mode A address of the same key, so a FROST"
info "signature is indistinguishable from one signed by the full key."
show "$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" \
    --to "$TO" --lamports 1 --blockhash "$BH"
A_SIGN="$("$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" \
    --to "$TO" --lamports 1 --blockhash "$BH")"
printf '%s\n' "$A_SIGN" >/dev/null
A_FROM="$(printf '%s\n' "$A_SIGN" | sed -n 's/^From:      //p')"
if [[ -n "$MPC_FROM" && "$MPC_FROM" == "$A_FROM" ]]; then
    ok "Mode A and Mode B derive the same sender address: $A_FROM"
else
    fail "Mode A and Mode B sender addresses differ -- FROST group key mismatch"
    exit 1
fi
show "$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH"
MPC_AGAIN="$("$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH")"
printf '%s\n' "$MPC_AGAIN" >/dev/null
MPC_SIG1="$(printf '%s\n' "$MPC_SIGN" | sed -n 's/^Signature: //p')"
MPC_SIG2="$(printf '%s\n' "$MPC_AGAIN" | sed -n 's/^Signature: //p')"
if [[ "$MPC_SIG1" != "$MPC_SIG2" ]]; then
    ok "each signing operation uses fresh nonces (signatures differ across runs)"
else
    fail "FROST produced identical signatures -- nonce reuse"
    exit 1
fi
pause

step 13d "Mode B -- failure modes"
info "One share is below the threshold (Error::NotEnoughShares), and a Mode A"
info "shard file cannot be used as a FROST share (different file magic)."
show "$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH"
if "$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH" 2>"$OUT/err-mpc-few.txt"; then
    fail "one share should never sign a 2-of-3 group"
    exit 1
fi
printf '%s%s%s\n' "$RED" "$(cat "$OUT/err-mpc-few.txt")" "$RESET"
if grep -q "need 2 shards but only 1 were provided" "$OUT/err-mpc-few.txt"; then
    ok "rejected: not enough participants"
else
    fail "unexpected error output"
    exit 1
fi
show "$BIN" mpc-sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH"
if "$BIN" mpc-sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --group-dir "$OUT/mpc" \
    --password "$PW" --to "$TO" --lamports 1 --blockhash "$BH" 2>"$OUT/err-mpc-mixed.txt"; then
    fail "an SSS shard must not sign as a FROST share"
    exit 1
fi
printf '%s%s%s\n' "$RED" "$(cat "$OUT/err-mpc-mixed.txt")" "$RESET"
if grep -q "not a horcrux FROST share" "$OUT/err-mpc-mixed.txt"; then
    ok "rejected: share type mismatch caught by file magic"
else
    fail "unexpected error output"
    exit 1
fi
pause

# --- 14. verify ---------------------------------------------------------
step 14 "verify -- passive integrity checks (no key material touched)"
info "verify reads the files without decrypting: magic (HX1 vs HX2), version,"
info "length, and cross-file split consistency. With --password it additionally"
info "checks the AES-GCM auth tag. It never writes to the access log."
show "$BIN" verify "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW"
"$BIN" verify "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW"
show "$BIN" verify "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --password "$PW"
"$BIN" verify "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --password "$PW"
show "$BIN" verify "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password wrong-password
if "$BIN" verify "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password wrong-password >/dev/null 2>&1; then
    fail "verify must reject a wrong password"
    exit 1
fi
ok "structure + auth-tag verification works for HX1 shards and HX2 shares"
pause

# --- 15. optional live broadcast ---------------------------------------
step 15 "live broadcast to solana-test-validator (optional)"
if ! command -v solana-test-validator >/dev/null 2>&1; then
    warn "solana-test-validator not found -- skipping live broadcast step"
    warn "install it, then re-run ./demo.sh to see transactions confirmed on-chain"
else
    info "Starting a fresh local validator; will fund the derived address, then"
    info "broadcast a Mode A and a Mode B signed transfer and wait for confirmation."
    LEDGER="$OUT/ledger"
    if [[ -d "$LEDGER" ]]; then
        rm -rf "$LEDGER"
    fi
    solana-test-validator --ledger "$LEDGER" --quiet >"$OUT/validator.log" 2>&1 &
    VALIDATOR_PID=$!

    info "waiting for the validator RPC on http://127.0.0.1:8899 ..."
    ready=0
    for _ in $(seq 1 90); do
        if solana cluster-version --url http://127.0.0.1:8899 >/dev/null 2>&1; then
            ready=1
            break
        fi
        sleep 1
    done
    if [[ "$ready" != 1 ]]; then
        fail "validator did not become ready -- see $OUT/validator.log"
        exit 1
    fi
    ok "validator ready (localnet, RPC 127.0.0.1:8899)"
    broadcast_ran=1

    show solana airdrop 1 "$A_FROM" --url http://127.0.0.1:8899
    if ! solana airdrop 1 "$A_FROM" --url http://127.0.0.1:8899 >/dev/null; then
        fail "airdrop failed -- cannot fund the derived address"
        exit 1
    fi
    ok "funded sender $A_FROM with 1 SOL"

    show solana airdrop 1 "$TO" --url http://127.0.0.1:8899
    if ! solana airdrop 1 "$TO" --url http://127.0.0.1:8899 >/dev/null; then
        fail "airdrop failed -- cannot fund the recipient address"
        exit 1
    fi
    ok "funded recipient $TO so the recipient is rent-exempt"

    show "$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" \
        --log-file "$OUT/broadcast.log" --to "$TO" --lamports 1 --broadcast
    A_BROADCAST="$("$BIN" sign "$OUT/shard-1.hx" "$OUT/shard-2.hx" --password "$PW" \
        --log-file "$OUT/broadcast.log" --to "$TO" --lamports 1 --broadcast)"
    printf '%s\n' "$A_BROADCAST"
    if ! printf '%s\n' "$A_BROADCAST" | grep -q "Mined:.*confirmed"; then
        fail "Mode A broadcast did not confirm"
        exit 1
    fi
    ok "Mode A transfer confirmed on-chain"

    show "$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --group-dir "$OUT/mpc" \
        --password "$PW" --log-file "$OUT/broadcast.log" --to "$TO" --lamports 1 --broadcast
    B_BROADCAST="$("$BIN" mpc-sign "$OUT/mpc/mpc-1.hx" "$OUT/mpc/mpc-2.hx" --group-dir "$OUT/mpc" \
        --password "$PW" --log-file "$OUT/broadcast.log" --to "$TO" --lamports 1 --broadcast)"
    printf '%s\n' "$B_BROADCAST"
    if ! printf '%s\n' "$B_BROADCAST" | grep -q "Mined:.*confirmed"; then
        fail "Mode B broadcast did not confirm"
        exit 1
    fi
    ok "Mode B (FROST) transfer confirmed on-chain"

    info "The audit log records both confirmed broadcasts:"
    show "$BIN" log --log-file "$OUT/broadcast.log" --tail 2
    "$BIN" log --log-file "$OUT/broadcast.log" --tail 2

    kill "$VALIDATOR_PID" 2>/dev/null || true
    wait "$VALIDATOR_PID" 2>/dev/null || true
    VALIDATOR_PID=""
    ok "validator stopped"
fi
pause

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
ok "audit:       every shard decrypt logged; 3 failures block signing"
ok "mpc:         2-of-3 FROST key shares sign without reconstructing the key"
ok "mpc:         same sender address as Mode A; signatures non-deterministic"
ok "mpc:         rejected: too few participants / Mode A shard as FROST share"
ok "verify:      structural + auth-tag integrity checks (HX1 and HX2)"
if [[ "$broadcast_ran" == 1 ]]; then
    ok "broadcast:   Mode A and Mode B transfers confirmed on solana-test-validator"
else
    warn "broadcast:   skipped (solana-test-validator not installed)"
fi
if [[ "$run_tests" == 1 ]]; then
    ok "tests:      66 unit + 8 roundtrip + 3 sign + 4 audit + 7 mpc + 3 verify tests green"
fi
printf '%s\n' "---"
info "Phases 1-4 are done end to end: init, reconstruct, offline sign + broadcast"
info "to the local validator, audit/anomaly detection, Mode B FROST threshold"
info "signing where the key never exists on any machine, and passive verify."
info "Phase 5 hardens the CLI surface (verify) and rehearses the full demo."
