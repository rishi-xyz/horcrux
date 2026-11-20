#!/usr/bin/env bash
# Start (or reuse) a local Solana test validator and a funded demo keypair,
# for exercising Init -> Sign -> Broadcast end to end in the web UI/TUI/CLI
# without touching real funds or a public cluster.
#
# Usage:
#   scripts/local-devnet.sh                 # start validator if needed, fund the demo recipient
#   scripts/local-devnet.sh --fund <addr>    # airdrop to any address (e.g. your split key's
#                                             # "From" address, once Init/Sign has shown it to you)
#   scripts/local-devnet.sh --reset          # wipe validator ledger + demo keypair, start fresh
#   scripts/local-devnet.sh --stop           # stop a validator this script started
#
# The printed "demo recipient" address is funded and ready to use as the
# Sign form's recipient right away. The *sender* is derived from whatever
# key you split with Init, and only exists once you've done that — fund it
# with --fund after your first Sign attempt shows you its "From" address.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="$ROOT_DIR/.local-devnet"
LEDGER_DIR="$STATE_DIR/ledger"
KEYPAIR="$STATE_DIR/demo-keypair.json"
PID_FILE="$STATE_DIR/validator.pid"
LOG_FILE="$STATE_DIR/validator.log"
RPC_URL="http://127.0.0.1:8899"
AIRDROP_SOL=10

mkdir -p "$STATE_DIR"

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: '$1' not found in PATH." >&2
    echo "Install the Solana CLI tools: https://docs.solanalabs.com/cli/install" >&2
    exit 1
  }
}

stop_validator() {
  if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
    local pid
    pid="$(cat "$PID_FILE")"
    echo "Stopping validator (pid $pid)..."
    kill "$pid"
    for _ in $(seq 1 10); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 1
    done
    kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null
    rm -f "$PID_FILE"
  else
    echo "No validator running (per $PID_FILE)."
  fi
}

if [ "${1:-}" = "--stop" ]; then
  stop_validator
  exit 0
fi

require solana
require solana-keygen
require solana-test-validator

if [ "${1:-}" = "--fund" ]; then
  TARGET="${2:-}"
  [ -n "$TARGET" ] || { echo "usage: $0 --fund <address>" >&2; exit 1; }
  if ! curl -s -o /dev/null -m 1 "$RPC_URL" 2>/dev/null; then
    echo "error: no validator answering at $RPC_URL — run '$0' first to start one." >&2
    exit 1
  fi
  echo "Airdropping $AIRDROP_SOL SOL to $TARGET ..."
  solana airdrop "$AIRDROP_SOL" "$TARGET" --url "$RPC_URL"
  exit 0
fi

if [ "${1:-}" = "--reset" ]; then
  stop_validator
  rm -rf "$LEDGER_DIR" "$KEYPAIR"
fi

# --- Demo keypair -----------------------------------------------------
if [ ! -f "$KEYPAIR" ]; then
  echo "Generating a demo keypair at $KEYPAIR ..."
  solana-keygen new --no-bip39-passphrase --silent --outfile "$KEYPAIR"
fi
ADDRESS="$(solana-keygen pubkey "$KEYPAIR")"

# --- Validator ----------------------------------------------------------
if curl -s -o /dev/null -m 1 "$RPC_URL" 2>/dev/null; then
  echo "A validator is already answering at $RPC_URL — reusing it."
else
  echo "Starting solana-test-validator (logs: $LOG_FILE) ..."
  mkdir -p "$LEDGER_DIR"
  nohup solana-test-validator --ledger "$LEDGER_DIR" --reset >"$LOG_FILE" 2>&1 &
  echo $! >"$PID_FILE"
  echo "Waiting for it to come up..."
  for _ in $(seq 1 60); do
    if curl -s -o /dev/null -m 1 "$RPC_URL" 2>/dev/null; then
      break
    fi
    sleep 1
  done
  if ! curl -s -o /dev/null -m 1 "$RPC_URL" 2>/dev/null; then
    echo "error: validator didn't come up in time — check $LOG_FILE" >&2
    exit 1
  fi
fi

# --- Fund the demo address ----------------------------------------------
BALANCE="$(solana balance "$ADDRESS" --url "$RPC_URL" 2>/dev/null | awk '{print $1}')"
BALANCE="${BALANCE:-0}"
NEEDS_FUNDING=$(awk -v b="$BALANCE" 'BEGIN { print (b+0 < 1) ? "1" : "0" }')
if [ "$NEEDS_FUNDING" = "1" ]; then
  echo "Airdropping $AIRDROP_SOL SOL to $ADDRESS ..."
  solana airdrop "$AIRDROP_SOL" "$ADDRESS" --url "$RPC_URL"
else
  echo "$ADDRESS already has $BALANCE SOL — skipping airdrop."
fi

cat <<EOF

Local devnet ready.

  RPC URL:  $RPC_URL   (this is already the web UI's/CLI's default)
  Address:  $ADDRESS
  Keypair:  $KEYPAIR   (solana CLI keypair — NOT a horcrux shard; useful for
                        funding other addresses or checking balances, e.g.
                        'solana balance <addr> --url $RPC_URL')

For a demo: split a key (Init), then in Sign set the recipient to
$ADDRESS (or any address), turn on Broadcast, and leave RPC URL as the
default — it already points here. To fund the address horcrux derives
from your *split* key so it can pay a transaction fee, airdrop to it too:

  solana airdrop 5 <the "From" address a split/reconstruct shows you> --url $RPC_URL

Stop the validator this script started with: scripts/local-devnet.sh --stop
EOF
