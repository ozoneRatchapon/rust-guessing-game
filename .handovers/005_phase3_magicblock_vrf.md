# 005: Phase 3 — MagicBlock VRF

## Status: Complete

## What Happened

Phase 3 implementation of the Rust Guessing Game using MagicBlock VRF as a third randomness source. This adds a callback-based VRF flow where the program requests randomness via CPI, and the MagicBlock oracle calls back with the result.

## What Was Built

- **5-instruction Anchor program**: `initialize`, `request_randomness`, `consume_randomness`, `guess`, `close_game`
- **Inlined VRF constants** — `ephemeral-vrf-sdk` is incompatible with Anchor 1.0.1 Pubkey types, so all VRF program IDs, instruction discriminators, and account layouts were inlined directly
- **Rejection sampling** (`random_u8_with_range`) — unbiased `[1,100]` secret derivation from raw VRF bytes
- **blake3 hash audit trail** — secret verification via on-chain hash commitment
- **VRF identity signer constraint** — `consume_randomness` callback restricted to MagicBlock oracle only
- **15 LiteSVM tests** — init, guessing, game over, close, boundary values, wrong identity, double consume, full session
- **devnet client script** — `scripts/play-phase3-devnet.ts`

## Where Is the Code

| File | Description |
|------|-------------|
| `on-chain/programs/phase3-magicblock-vrf/src/lib.rs` | Program entry (5 instructions) |
| `on-chain/programs/phase3-magicblock-vrf/src/constants.rs` | Game + inlined VRF constants |
| `on-chain/programs/phase3-magicblock-vrf/src/state.rs` | `GameV3` account + 6 events |
| `on-chain/programs/phase3-magicblock-vrf/src/error.rs` | 9 error codes |
| `on-chain/programs/phase3-magicblock-vrf/src/instructions/request_randomness.rs` | Manual VRF CPI (most complex) |
| `on-chain/programs/phase3-magicblock-vrf/src/instructions/consume_randomness.rs` | VRF callback handler + `random_u8_with_range` |
| `on-chain/programs/phase3-magicblock-vrf/tests/test_phase3.rs` | 15 LiteSVM tests |
| `docs/phase3-magicblock-vrf.md` | Full documentation |
| `on-chain/scripts/play-phase3-devnet.ts` | Devnet client script |

## Architecture Decisions

| Decision | Rationale |
|----------|-----------|
| Callback pattern: `request_randomness` (CPI to VRF) → `consume_randomness` (VRF calls back) | Matches MagicBlock's async oracle design — program requests randomness, oracle delivers it |
| Inlined all VRF constants instead of using `ephemeral-vrf-sdk` | SDK v0.2.3 requires `solana-program >=1.18.26,<3` but Anchor 1.0.1 uses `solana-pubkey 3.x/4.x` — type mismatch |
| `request_randomness` manually builds `Instruction` with `AccountMeta`, serializes with borsh | Bypasses SDK dependency entirely — full control over CPI construction |
| `consume_randomness` requires `VRF_PROGRAM_IDENTITY` as signer | Only the MagicBlock oracle can invoke this callback — prevents anyone from injecting fake randomness |
| Slot hashes sysvar ID hardcoded as string | Anchor 1.0.1 doesn't re-export `slot_hashes` sysvar |
| blake3 hash for audit trail | Anchor 1.0.1 doesn't re-export `solana_program::hash`; blake3 is already a project dependency |

## Struggling / Solved

| Problem | Solution |
|---------|----------|
| `ephemeral-vrf-sdk` incompatibility with Anchor 1.0.1 Pubkey types | Inlined all constants and built CPI manually — no SDK dependency needed |
| Anchor 1.0.1 doesn't re-export `solana_program::hash` | Used `blake3` crate directly for audit trail hashing |
| Anchor 1.0.1 doesn't re-export `slot_hashes` sysvar | Hardcoded the sysvar ID as a string constant |
| `borsh::BorshSerialize` derive macro requires borsh as direct dependency | Added `borsh` to `Cargo.toml` directly |
| `AccountInfo` deprecated in Anchor 1.0.1 | Used `UncheckedAccount` instead |

## Test Results

All 15 Phase 3 tests pass. **Total project: 40 tests** across 3 phases + broken-rand proof.

```
Phase 1 (commit-reveal): 8 tests
Phase 2 (Switchboard VRF): 16 tests
Phase 3 (MagicBlock VRF): 15 tests
Broken-rand proof: 1 test
─────────────────────────────
Total: 40 tests (all passing)
```

## Remaining Work

- [ ] Deploy to devnet when ready
- [ ] Monitor `ephemeral-vrf-sdk` for Anchor 1.0.1 compatibility — switch from inlined constants back to SDK once compatible
- [ ] Phase 4: Potential Ephemeral Rollup multiplayer (1ms block time, zero fees)

## Issues Ref

- `.issues/001_broken_rand_demo.md`

## How to Dev/Test

```bash
# Build
anchor build --program-name phase3_magicblock_vrf --ignore-keys

# Test (Phase 3 only)
cargo test --manifest-path on-chain/programs/phase3-magicblock-vrf/Cargo.toml -- --nocapture

# All tests
bash test.sh

# Play on devnet
npx tsx scripts/play-phase3-devnet.ts
```
