# 003: Phase 2 LiteSVM Tests + Cost Analysis + Devnet Redeploy

## Status: Complete

## What Happened

Session continued from the four demos milestone. Three things accomplished:

### 1. Committed Phase 2 LiteSVM Tests (16 tests)

The 16 Phase 2 LiteSVM tests (from prior session) were committed and merged:

- Branch: `feature/03_phase2_litesvm_tests` → merged into `develop` → merged into `main`
- All 24 tests verified green (8 Phase 1 + 16 Phase 2)
- Key refactoring: replaced `RandomnessAccountData::parse()` with manual byte-level functions
  to avoid bytemuck alignment issues in LiteSVM

### 2. Merged develop → main

Branches had diverged (different SHAs, same content). Used `git merge -X theirs` to resolve
add/add conflicts, taking develop's canonical versions. Both branches now in sync.

### 3. Phase 2 Cost Analysis

Redeployed Phase 2 program with refactored code:
```
solana program deploy --url devnet phase2_vrf.so --program-id phase2_vrf-keypair.json
Signature: 5B7hMoJxvxE2ix1sSVuwRMcCNhoHDeKn6ydad84cW3eYcKdid9fxMhpHV9bKAdroGEQ3VpGGUBGfVhrY7TSLwCfS
```

Created `scripts/cost-analysis-phase2.ts` to extract CU/fee data from recent devnet transactions.

Results:

| Instruction | Fee | Compute Units | Notes |
|-------------|----:|--------------:|-------|
| initialize | 10,000 lamports | ~121,148 | Multi-ix: Switchboard + our init |
| settle_random | 5,000 lamports | ~49,836 | VRF reveal + secret derivation |
| guess | 5,000 lamports | ~2,992 | Per guess |
| close_game | 5,000 lamports | ~5,396 | Rent recovery |

### 4. Demo Rehearsal Status

- ✅ Demo 1 (Pure Rust CLI) — Works (interactive)
- ✅ Demo 2 (Phase 1 devnet) — IDL loaded, program live, wallet funded (8.9 SOL)
- ✅ Demo 3 (Phase 2 devnet) — Redeployed with refactored code, program live
- ✅ Demo 4 (Broken rand) — Verified working, clean output

All 4 programs confirmed live on devnet:
- Phase 1: `3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT` — 199,192 bytes
- Phase 2: `CHXkyr3GrLvWRXdbnYgPMKhwU1dYF6gW9aUpV8S3oTJw` — 207,488 bytes

## Where Is the Code

| File | Action | Description |
|------|--------|-------------|
| `on-chain/programs/phase2-vrf/tests/test_phase2.rs` | From prior session | 660-line test file, 16 tests |
| `on-chain/programs/phase2-vrf/src/instructions/initialize.rs` | From prior session | Manual `validate_randomness_account()` |
| `on-chain/programs/phase2-vrf/src/instructions/settle_random.rs` | From prior session | Manual `read_randomness_value()` |
| `on-chain/scripts/cost-analysis-phase2.ts` | **New** | Cost analysis tool (158 lines) |
| `README.md` | **Modified** | Added Phase 2 cost table |

## The Plan / How to Run

```bash
# Run all tests
cargo test --manifest-path on-chain/programs/phase2-vrf/Cargo.toml   # 16 tests
cargo test --manifest-path on-chain/programs/on-chain/Cargo.toml     # 8 tests

# Run cost analysis
cd on-chain && npx tsx scripts/cost-analysis-phase2.ts

# Run demos
cargo run                                              # Demo 1: Pure Rust
cd on-chain && npx tsx scripts/build-broken-rand.ts    # Demo 4: Broken rand
cd on-chain && npx tsx scripts/play-devnet.ts           # Demo 2: Phase 1 (interactive)
cd on-chain && npx tsx scripts/play-phase2-devnet.ts    # Demo 3: Phase 2 (interactive)
```

## Reflection

### Solved

1. **Branch divergence** — `main` and `develop` had same content but different SHAs. Used `git merge -X theirs` to take develop's versions cleanly.

2. **Cost analysis without re-running game** — Created a script that reads recent devnet transactions instead of needing a fresh game session. Much faster and captures real-world data.

3. **Phase 2 redeploy** — Refactored program (manual byte extraction) deployed successfully. Functionally equivalent to old version, no behavior change.

### Struggled With

- Clippy timing out on Phase 2 program (memory pressure from compiling all deps). Tests pass, which is the main gate.

## Remain Work

- [ ] **Interactive demo rehearsal** — Run Demos 2 & 3 manually with real guesses to verify end-to-end
- [ ] **Optional: `play:phase2:local`** — Offline Phase 2 play using LiteSVM + mocked randomness
- [ ] **Optional: CI pipeline** — Auto-run 24 tests on push

## Issues Ref

- `.issues/001_broken_rand_demo.md` — complete

## Git Log

```
375596b (main) Merge develop: add 16 Phase 2 LiteSVM tests with mocked Switchboard VRF
e132148 (develop) Merge feature/03_phase2_litesvm_tests into develop
89a57e7 feat: add 16 Phase 2 LiteSVM tests with mocked Switchboard VRF
```

## Branch Structure

```
main (375596b) ← up to date with develop
develop (e132148) ← Phase 2 tests merged
  tag: phase1-demo → Phase 1 snapshot (on develop)
```
