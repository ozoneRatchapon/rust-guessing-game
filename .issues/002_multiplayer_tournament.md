# 002: Phase 4 — Multi-Player Tournament Guessing Game

## Status: In Progress

## Problem

Phases 1-3 are single-player: one person guesses a secret number. Phase 4 introduces **multi-player tournament mode** where multiple players compete to guess the same secret number in the fewest attempts.

## Design

### Accounts

**Tournament** (PDA: `seeds = [b"tournament", admin.key()]`)
```
Tournament {
  admin: Pubkey,
  secret_hash: [u8; 32],       // blake3(secret) after settle
  secret_number: u8,            // revealed after VRF settle
  is_settled: bool,
  max_tries_per_player: u8,     // 10
  player_count: u8,
  max_players: u8,              // 16
  is_finished: bool,
  bump: u8,
  randomness_account: Pubkey,   // Switchboard randomness PDA
  commit_slot: u64,
}
```

**PlayerEntry** (PDA: `seeds = [b"player", tournament.key(), player.key()]`)
```
PlayerEntry {
  player: Pubkey,
  tournament: Pubkey,
  guess_count: u8,
  best_distance: u8,            // |guess - secret| (lower is better)
  found_exact: bool,            // guessed the exact number
  bump: u8,
}
```

### Instructions (5)

| # | Instruction | Who | What |
|---|------------|-----|------|
| 1 | `create_tournament` | Admin | Create tournament PDA + store Switchboard randomness account |
| 2 | `settle_tournament` | Admin | Settle VRF → derive and store secret number |
| 3 | `join_tournament` | Any player | Create PlayerEntry PDA (before or after settle) |
| 4 | `submit_guess` | Joined player | Submit a guess, track best distance |
| 5 | `close_tournament` | Admin | Close tournament, recover rent |

### Winner Logic
- Players who found the exact number: ranked by fewest guesses
- If no exact match: closest distance wins, ties broken by fewer guesses
- Leaderboard computed off-chain from PlayerEntry accounts

### Randomness
- Uses **Switchboard VRF** (proven to work on devnet from Phase 2)
- Same mock pattern for LiteSVM tests

## Scope

- [ ] Program source (`phase4-tournament/`)
- [ ] LiteSVM tests
- [ ] Devnet client script
- [ ] Documentation
