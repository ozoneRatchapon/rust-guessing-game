# 002: Four Demos Milestone — All Working

## Status: Complete

## What Happened

Built and deployed all 4 demos for the Solana Guessing Game presentation. Every demo is live and verified.

### Architecture Decision: Separate Programs

Phase 2 is a **separate Anchor program** (`phase2-vrf`), not a replacement of Phase 1 (`on-chain`). Both coexist on devnet with different Program IDs. Reasons:

1. Phase 1 stays demo-able forever (already deployed)
2. Different instructions need different IDLs (Phase 2 has no `reveal`, adds `settle_random`)
3. Side-by-side comparison is better for teaching
4. Git tag `phase1-demo` preserves Phase 1 snapshot on `main`

### broken-rand: Standalone Workspace

`broken-rand` lives in `on-chain/demos/` (not `programs/`) with its own `[workspace]` table. This prevents the parent workspace's `getrandom = { features = ["custom"] }` from leaking into it — that feature would bypass the compile error we want to demonstrate.

## Where Is the Code

### Programs

| Program | Path | Program ID |
|---------|------|------------|
| Phase 1 (commit-reveal) | `on-chain/programs/on-chain/` | `3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT` |
| Phase 2 (Switchboard VRF) | `on-chain/programs/phase2-vrf/` | `CHXkyr3GrLvWRXdbnYgPMKhwU1dYF6gW9aUpV8S3oTJw` |
| Broken rand | `on-chain/demos/broken-rand/` | N/A (doesn't compile for BPF) |

### Scripts

| Script | Purpose |
|--------|---------|
| `on-chain/scripts/play-devnet.ts` | Demo 2: Phase 1 interactive (commit-reveal) |
| `on-chain/scripts/play-phase2-devnet.ts` | Demo 3: Phase 2 interactive (VRF) |
| `on-chain/scripts/build-broken-rand.ts` | Demo 4: Proves `rand` fails on BPF |
| `on-chain/scripts/demo.sh` | Menu launcher for all 4 demos |

### Docs

| Doc | Content |
|-----|---------|
| `README.md` | 4-demo table, multi-program structure, architecture decision |
| `docs/phase1-commit-reveal.md` | Phase 1 walkthrough |
| `docs/phase2-switchboard-vrf.md` | Phase 2 architecture (560 lines) |
| `docs/on-chain-randomness-lesson.md` | Teaching guide |
| `.handovers/001_phase2_switchboard_vrf.md` | Phase 2 planning (updated) |
| `.issues/001_broken_rand_demo.md` | Broken rand plan (updated) |

### Tests

- Phase 1: 8 LiteSVM tests (all passing)
- Phase 2: 16 LiteSVM tests (all passing, mocked Switchboard randomness)

Phase 2 tests mock the Switchboard `RandomnessAccountData` by constructing fake account data
with the correct discriminator, a known `value` byte, and `reveal_slot` synced to `svm.warp_to_slot()`.

## The Plan / How to Run

```bash
# Demo launcher (interactive menu)
cd on-chain && bash scripts/demo.sh

# Or run individually:
cargo run                                          # Demo 1: Pure Rust
cd on-chain && npx tsx scripts/play-devnet.ts       # Demo 2: Phase 1
cd on-chain && npx tsx scripts/play-phase2-devnet.ts # Demo 3: Phase 2
cd on-chain && npx tsx scripts/build-broken-rand.ts  # Demo 4: Broken rand
```

### Demo Flow for Presentation

1. **Demo 1** — `cargo run` — Show the original Rust Book game works locally
2. **Demo 4** — `build-broken-rand.ts` — Prove `rand` fails on Solana (the "why")
3. **Demo 2** — `play-devnet.ts` — Phase 1 solution: admin commit-reveal
4. **Demo 3** — `play-phase2-devnet.ts` — Phase 2 solution: trustless VRF

This order tells a story: "here's the problem → here's why → here's the simple fix → here's the production fix"

## Reflection

### Struggled With

1. **getrandom workspace leak** — `broken-rand` was in `programs/` initially, but the workspace `getrandom = { features = ["custom"] }` (needed for `phase2-vrf`) leaked into it and made `rand` compile for BPF. Fixed by moving `broken-rand` to `demos/` with its own `[workspace]` table.

2. **Switchboard API differences** — The `switchboard-on-demand` v0.12.1 API differed from docs:
   - `get_value()` takes `u64` slot, not `&Clock`
   - Returns `Result<[u8; 32]>`, not `Option<[u8; 32]>`
   - `anchor` feature flag causes borsh v1 conflict with anchor-lang 1.0.1 — removed it
   - `AccountInfo` deprecated in struct bindings, use `UncheckedAccount`

3. **anchor build includes all programs** — Can't exclude `broken-rand` from `anchor build` when it's in `programs/`. Moving to `demos/` was the clean fix.

### Solved

- Full VRF flow working on devnet: init → wait 3s → settle → guess
- All 4 demos verified end-to-end
- Smart resume logic in play scripts (detects existing game state)

## Remain Work

- [ ] Demo dry-run / rehearsal
- [ ] Consider adding a `play:phase2:local` feature for offline Phase 2 testing
- [ ] Optional: add cost analysis for Phase 2 transactions

## Issues Ref

- `.issues/001_broken_rand_demo.md` — complete and working

## How to Dev/Test

```bash
# Build all programs
cd on-chain && anchor build --skip-lint --ignore-keys

# Test Phase 1 (8 tests)
cargo test --manifest-path on-chain/programs/on-chain/Cargo.toml

# Build broken-rand separately (intentionally fails)
cargo build-sbf --manifest-path on-chain/demos/broken-rand/Cargo.toml

# Run broken-rand demo
cd on-chain && npx tsx scripts/build-broken-rand.ts

# Deploy Phase 2 to devnet
solana program deploy --url devnet on-chain/target/deploy/phase2_vrf.so \
  --program-id on-chain/target/deploy/phase2_vrf-keypair.json

# Play Phase 2 on devnet
cd on-chain && npx tsx scripts/play-phase2-devnet.ts
```

## Git Log (develop branch)

```
04e8868 feat: add Phase 2 devnet script, broken-rand demo, and demo launcher
87d82a7 feat: add phase2-vrf (Switchboard VRF) and broken-rand programs
3507502 docs: update architecture to 4-demo structure, add Phase 2 separate program plan
```

## Branch Structure

```
main (tagged phase1-demo) → Phase 1 snapshot, never touched
develop (3 commits ahead) → All 4 demos working
```
