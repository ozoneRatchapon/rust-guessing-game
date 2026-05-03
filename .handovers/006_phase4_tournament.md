# Handover 006 — Phase 4: Multi-Player Tournament

## Overview

Phase 4 adds multi-player tournament support to the on-chain Solana guessing game. Up to 16 players compete across 10 guesses each, with a secret number generated trustlessly via Switchboard VRF. Players are ranked by exact matches first, then closest distance to the secret. This is the first phase to support multi-player competition on-chain.

## What Happened

- Designed and implemented a full tournament lifecycle: **create → settle → join → guess → close**.
- Integrated **Switchboard VRF** for trustless secret number generation (1–100).
- Supports up to **16 players** with **10 guesses each**.
- Ranking: exact matches first, then closest absolute distance to the secret.
- Wrote **24 LiteSVM tests** covering the full tournament flow, boundary values, multi-player competition, and all error cases.
- Created an interactive **devnet play script** (`play-phase4-devnet.ts`) for manual end-to-end testing.
- Confirmed devnet lifecycle: CREATE, WAIT for oracle, SETTLE all succeeded. Interactive JOIN/GUESS phase requires a real terminal.

## Technical Details

| Field | Value |
|-------|-------|
| Program ID | `FKqXgQYFUgMifKoQTYbb5UzMLry6RDo9E6dWm6E4fKoL` |
| Program path | `programs/phase4-tournament/` |
| IDL on-chain metadata | `FgEWGLfUkWB2cHi62TPL2n593r4E19F4p48ZKQzfQD7M` |
| Data length | 245,088 bytes (~1.71 SOL rent) |
| Switchboard PID | `Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2` |
| Switchboard Queue | `EYiAmGSdsQTuCw413V5BzaruWuCCSDgTPtBGvLkXHbe7` |

### Instructions

| Instruction | Description |
|-------------|-------------|
| `create_tournament` | Admin creates tournament with VRF commitment |
| `settle_tournament` | Reveals VRF randomness to set secret number (1–100) |
| `join_tournament` | Player joins (PDA: `["player", tournament.key(), player.key()]`) |
| `submit_guess` | Player submits guess, gets feedback (too_high / too_low / correct) |
| `close_tournament` | Admin closes, rent recovered |

### Accounts (`state.rs`)

- **Tournament** — stores admin, VRF pubkey, secret number, player list, status, max players/guesses.
- **PlayerEntry** — stores player pubkey, guess history, exact match count, best distance.

### Error Types (`errors.rs`)

Custom errors for: tournament full, already joined, not joined, out of guesses, tournament not active, guess out of range, unauthorized, VRF not settled, etc.

## Struggles & Solutions

### Borrow Checker E0502 in `join_tournament`

**Problem:** `ctx.accounts.tournament` was mutably borrowed then accessed immutably within the same scope, triggering Rust E0502.

**Fix:** Capture all `Pubkey` values and bumps **before** taking any mutable borrows. Extract read-only data first, then perform writes.

### Program ID Journey

| Stage | Program ID | Symptom |
|-------|-----------|---------|
| Initial | `1111...` (system program) | `InstructionFallbackNotFound` on all tests |
| Updated from keypair | `FKqXgQYFUgMifKoQTYbb5UzMLry6RDo9E6dWm6E4fKoL` | Tests still failed |
| Rebuilt `.so` | Same as above | Fixed — `DeclaredProgramIdMismatch` was caused by stale `.so` |

**Lesson:** Changing `declare_id!` requires a full rebuild of the `.so` binary. The compiled bytecode embeds the program ID.

### Devnet Faucet Rate Limiting

**Problem:** Devnet airdrop returns 429 after 1–2 requests per keypair.

**Fix:** Transfer SOL from the admin keypair to each player (0.05 SOL per player) instead of requesting individual airdrops.

## Files Changed

| File | Purpose |
|------|---------|
| `programs/phase4-tournament/src/lib.rs` | Program entry point, `declare_id!` |
| `programs/phase4-tournament/src/instructions/` | All instruction handlers (create, settle, join, submit_guess, close) |
| `programs/phase4-tournament/src/state.rs` | `Tournament` and `PlayerEntry` account structs |
| `programs/phase4-tournament/src/errors.rs` | Custom error types |
| `tests/phase4-tournament.ts` | 24 LiteSVM tests |
| `scripts/play-phase4-devnet.ts` | Interactive devnet play script |

## Test Results

- **24/24 LiteSVM tests pass**
- Coverage: create, settle, join, submit_guess, close, boundary values (1 and 100), multi-player competition, full tournament session, and all error cases.

## Phase Comparison

| Phase | VRF | Devnet | Multi-player |
|-------|-----|--------|-------------|
| Phase 1 (Commit-Reveal) | None | ✅ | No |
| Phase 2 (Switchboard) | Switchboard | ✅ | No |
| Phase 3 (MagicBlock) | MagicBlock | ❌ | No |
| Phase 4 (Tournament) | Switchboard | ✅ | **Yes** |

## How to Dev/Test

```bash
cd on-chain

# Run LiteSVM tests (24 tests)
anchor test --skip-deploy

# Interactive devnet play
npm run play:phase4:devnet

# Local validator play
npm run play:phase4:local
```

## Remaining Work

- **Leaderboard:** Off-chain script ranking players across tournaments.
- **SOL wagering:** Players stake SOL, closest guess wins the pot.
- **Timed rounds:** Tournament auto-expires after N slots.
- **Frontend:** Web UI with wallet-connected players.
- **Hide secret:** Admin shouldn't see `secret_number` in real deployment.

## Issues Ref

- `.issues/002_multiplayer_tournament.md`
