#!/bin/bash
#
# Solana Guessing Game — Demo Launcher
#
# Run:  bash scripts/demo.sh [1-4]
#

set -euo pipefail

# Colors
CYAN='\033[36m'
GREEN='\033[32m'
YELLOW='\033[33m'
RED='\033[31m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'
BG_GREEN='\033[42m'
BG_RED='\033[41m'
WHITE='\033[37m'

banner() { echo -e "${CYAN}$1${RESET}"; }
ok()    { echo -e "${GREEN}✓${RESET} $1"; }
warn()  { echo -e "${YELLOW}⚠${RESET} $1"; }
err()   { echo -e "${RED}✗${RESET} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(cd "$ROOT_DIR/.." && pwd)"

show_menu() {
    echo
    banner "============================================"
    banner "  Solana Guessing Game — Demo Launcher"
    banner "============================================"
    echo
    echo -e "  ${BOLD}1${RESET}) Pure Rust CLI           ${DIM}(cargo run)${RESET}"
    echo -e "  ${BOLD}2${RESET}) Phase 1: Commit-Reveal   ${DIM}(devnet, admin secret)${RESET}"
    echo -e "  ${BOLD}3${RESET}) Phase 2: Switchboard VRF  ${DIM}(devnet, trustless VRF)${RESET}"
    echo -e "  ${BOLD}4${RESET}) Broken \`rand\` Proof       ${DIM}(cargo build-sbf fails)${RESET}"
    echo -e "  ${BOLD}5${RESET}) Phase 3: MagicBlock VRF  ${DIM}(devnet, free fast VRF)${RESET}"
    echo
}

run_demo() {
    local choice="$1"

    case "$choice" in
        1)
            echo
            banner "━━━ Demo 1: Pure Rust CLI ━━━"
            echo
            echo -e "  ${DIM}The original guessing game from The Rust Book Ch.2${RESET}"
            echo -e "  ${DIM}Uses \`rand\` crate — works on your laptop, not on Solana${RESET}"
            echo
            cd "$PROJECT_ROOT"
            cargo run
            ;;
        2)
            echo
            banner "━━━ Demo 2: Phase 1 — Commit-Reveal ━━━"
            echo
            echo -e "  ${DIM}Anchor program on devnet with admin commit-reveal${RESET}"
            echo -e "  ${DIM}Program: KXXhoaNpoXNNHCqB2YYjEBSXoUikpa2tou4haVJgvEU${RESET}"
            echo
            cd "$ROOT_DIR"
            npx tsx scripts/play-devnet.ts
            ;;
        3)
            echo
            banner "━━━ Demo 3: Phase 2 — Switchboard VRF ━━━"
            echo
            echo -e "  ${DIM}Anchor program on devnet with trustless VRF randomness${RESET}"
            echo -e "  ${DIM}Program: 94g894DkqpuewD8mKHimaBsuzFT7Qz2E9Wb8QPWUBsZ2${RESET}"
            echo
            cd "$ROOT_DIR"
            npx tsx scripts/play-phase2-devnet.ts
            ;;
        4)
            echo
            banner "━━━ Demo 4: Broken \`rand\` Proof ━━━"
            echo
            echo -e "  ${DIM}Proves \`rand\` cannot compile for Solana BPF target${RESET}"
            echo
            cd "$ROOT_DIR"
            npx tsx scripts/build-broken-rand.ts
            ;;
        5)
            echo
            banner "━━━ Demo 5: Phase 3 — MagicBlock VRF ━━━"
            echo
            echo -e "  ${DIM}Anchor program on devnet with MagicBlock VRF randomness${RESET}"
            echo -e "  ${DIM}Program: DnrNKTTspzjip8CAFXzCNkbMbQKXjNbZGnx6gNGtCEAH${RESET}"
            echo
            cd "$ROOT_DIR"
            npx tsx scripts/play-phase3-devnet.ts
            ;;
        *)
            err "Invalid choice: $choice"
            show_menu
            return 1
            ;;
    esac
}

# Main
if [ $# -eq 1 ]; then
    run_demo "$1"
else
    show_menu
    echo -en "  ${BOLD}Pick a demo [1-5]:${RESET} "
    read -r choice
    run_demo "$choice"
fi
