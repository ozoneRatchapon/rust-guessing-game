#!/usr/bin/env bash
#
# Run all Rust Guessing Game tests (LiteSVM — no network needed)
#
# Usage:
#   bash test.sh              # Run all tests
#   bash test.sh phase1       # Phase 1 only (8 tests)
#   bash test.sh phase2       # Phase 2 only (16 tests)
#   bash test.sh phase3       # Phase 3 only (15 tests)
#   bash test.sh phase4       # Phase 4 only (20 tests)
#   bash test.sh broken-rand  # Verify broken-rand fails for BPF (Demo 4)
#

set -euo pipefail

BOLD="\033[1m"
GREEN="\033[32m"
RED="\033[31m"
YELLOW="\033[33m"
CYAN="\033[36m"
DIM="\033[2m"
RESET="\033[0m"

PHASE1_MANIFEST="on-chain/programs/on-chain/Cargo.toml"
PHASE2_MANIFEST="on-chain/programs/phase2-vrf/Cargo.toml"
PHASE3_MANIFEST="on-chain/programs/phase3-magicblock-vrf/Cargo.toml"
PHASE4_MANIFEST="on-chain/programs/phase4-tournament/Cargo.toml"
BROKEN_RAND_MANIFEST="on-chain/demos/broken-rand/Cargo.toml"

pass=0
fail=0

run_tests() {
    local label="$1"
    local manifest="$2"

    echo -e "\n${CYAN}${BOLD}━━━ ${label} ━━━${RESET}"
    echo -e "${DIM}  manifest: ${manifest}${RESET}\n"

    if cargo test --manifest-path "${manifest}" --quiet -- --nocapture 2>&1; then
        pass=$((pass + 1))
        echo -e "\n${GREEN}✓ ${label} passed${RESET}"
    else
        fail=$((fail + 1))
        echo -e "\n${RED}✗ ${label} failed${RESET}"
    fi
}

run_broken_rand() {
    echo -e "\n${CYAN}${BOLD}━━━ Broken rand (Demo 4) ━━━${RESET}"
    echo -e "${DIM}  This should FAIL for BPF target — that's the proof${RESET}\n"

    echo -e "${YELLOW}  Step 1: Host compilation (should pass)...${RESET}"
    if cargo check --manifest-path "${BROKEN_RAND_MANIFEST}" --quiet 2>&1; then
        echo -e "${GREEN}  ✓ Host build passed${RESET}"
    else
        echo -e "${RED}  ✗ Host build failed (unexpected)${RESET}"
        fail=$((fail + 1))
        return
    fi

    echo -e "\n${YELLOW}  Step 2: BPF compilation (should FAIL)...${RESET}"
    if cargo build-sbf --manifest-path "${BROKEN_RAND_MANIFEST}" 2>&1; then
        echo -e "${RED}  ✗ BPF build passed (should have failed!)${RESET}"
        fail=$((fail + 1))
    else
        echo -e "${GREEN}  ✓ BPF build failed as expected — rand cannot work on Solana${RESET}"
        pass=$((pass + 1))
    fi
}

print_summary() {
    echo -e "\n${BOLD}━━━ Summary ━━━${RESET}"
    if [ "${fail}" -eq 0 ]; then
        echo -e "${GREEN}${BOLD}  All ${pass} test suites passed${RESET}"
        echo -e "${DIM}  (59 LiteSVM tests + broken-rand proof)${RESET}\n"
        return 0
    else
        echo -e "${RED}${BOLD}  ${fail} suite(s) failed, ${pass} passed${RESET}\n"
        return 1
    fi
}

# ─── Main ──────────────────────────────────────────────────────────────────────

echo -e "${BOLD}Rust Guessing Game — Test Runner${RESET}"
echo -e "${DIM}  LiteSVM tests (no network, no TypeScript)${RESET}"

case "${1:-all}" in
    phase1)
        run_tests "Phase 1: Commit-Reveal (8 tests)" "${PHASE1_MANIFEST}"
        ;;
    phase2)
        run_tests "Phase 2: Switchboard VRF (16 tests)" "${PHASE2_MANIFEST}"
        ;;
    phase3)
        run_tests "Phase 3: MagicBlock VRF (15 tests)" "${PHASE3_MANIFEST}"
        ;;
    phase4)
        run_tests "Phase 4: Multi-Player Tournament (20 tests)" "${PHASE4_MANIFEST}"
        ;;
    broken-rand)
        run_broken_rand
        ;;
    all)
        run_tests "Phase 1: Commit-Reveal (8 tests)" "${PHASE1_MANIFEST}"
        run_tests "Phase 2: Switchboard VRF (16 tests)" "${PHASE2_MANIFEST}"
        run_tests "Phase 3: MagicBlock VRF (15 tests)" "${PHASE3_MANIFEST}"
        run_tests "Phase 4: Multi-Player Tournament (20 tests)" "${PHASE4_MANIFEST}"
        run_broken_rand
        ;;
    *)
        echo -e "${RED}Unknown argument: $1${RESET}"
        echo "Usage: bash test.sh [all|phase1|phase2|phase3|phase4|broken-rand]"
        exit 1
        ;;
esac

print_summary
