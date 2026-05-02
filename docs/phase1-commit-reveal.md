# Phase 1: Commit-Reveal Guessing Game

## Overview

This phase implements an **on-chain guessing game** on Solana using a commit-reveal scheme. It takes the original CLI guessing game and moves it on-chain, adding cryptographic guarantees so players can verify the admin cannot cheat.

**How it works:**

1. The admin **commits** a hash of the secret number (not the number itself).
2. When ready, the admin **reveals** the secret number, and the program verifies it matches the committed hash.
3. Players **guess** the number. The program responds with "too small", "too big", or "correct".

**Trust model:** The admin knows the answer -- this is a known limitation. Phase 2 will replace the admin-chosen number with a Verifiable Random Function (VRF) from an oracle, removing the need to trust the admin at all.

**Key difference from CLI version:** In the original CLI game, the computer picks a random number locally. Here, the secret is committed on-chain as a `blake3` hash, making the commitment public and immutable. Once revealed, anyone can verify the hash matches.

---

## Architecture

### Program Structure

```
on-chain/programs/on-chain/src/
  lib.rs              # Program entrypoint: initialize, reveal, guess
  state.rs            # Game account struct + event definitions
  error.rs            # GameError enum (7 custom error codes)
  constants.rs        # MAX_TRIES, MIN_NUMBER, MAX_NUMBER
  instructions/
    initialize.rs     # Create game, store blake3 hash of secret
    reveal.rs         # Admin reveals secret, program verifies hash
    guess.rs          # Player guesses, program compares and emits events
```

### Account Layout

The `Game` account stores all game state in a single PDA:

| Field         | Type      | Size   | Description                          |
|---------------|-----------|--------|--------------------------------------|
| discriminator |           | 8      | Anchor account discriminator         |
| admin         | Pubkey    | 32     | Admin who initialized the game       |
| secret_hash   | [u8; 32]  | 32     | blake3 hash of the secret number     |
| secret_number | u8        | 1      | Actual secret (0 until revealed)     |
| is_revealed   | bool      | 1      | Whether the secret has been revealed |
| attempts      | u8        | 1      | Number of guesses made               |
| max_tries     | u8        | 1      | Maximum allowed guesses (10)         |
| is_finished   | bool      | 1      | Whether the game is over             |
| bump          | u8        | 1      | PDA bump seed                        |
| **Total**     |           | **78** |                                      |

### PDA Derivation

The game account is a Program Derived Address (PDA) derived from:

```rust
seeds = [b"game", admin.key().as_ref()]
```

This means each admin wallet can have exactly one game at a time. The PDA is deterministic -- anyone can compute the game address from the admin's public key.

### Instruction Flow

```
initialize(secret_number)     reveal(secret_number)     guess(guess)
        |                            |                        |
        v                            v                        v
  Admin creates game          Admin reveals secret      Player guesses
  Stores blake3 hash          Program verifies hash     Program compares
  secret_number = 0           secret_number filled      Emits event
  is_revealed = false         is_revealed = true        Attempts tracked
  is_finished = false         is_finished unchanged     Auto-finish at 10
```

**Order matters:** `initialize` -> `reveal` -> `guess`. You cannot guess before the secret is revealed.

---

## Instructions Deep Dive

### initialize(secret_number)

**Purpose:** Admin creates a new game by committing a hash of the secret number.

**Account validation:**

- `game` -- initialized as a PDA with seeds `[b"game", admin.key()]`, funded by the admin.
- `admin` -- must be a signer (proves ownership) and must pay for account rent.
- `system_program` -- required for account creation.

**What gets stored:**

The program does **not** store the secret number. It stores the `blake3` hash:

```rust
let hash = blake3::hash(&secret_number.to_le_bytes());
game.secret_hash = *hash.as_bytes();
game.secret_number = 0;  // NOT the real number
```

This is the "commit" in commit-reveal. The admin locks themselves into a specific number without revealing it.

**Security check -- range validation:**

```rust
require!(
    (MIN_NUMBER..=MAX_NUMBER).contains(&secret_number),
    GameError::InvalidSecretRange
);
```

The secret must be between 1 and 100. This prevents the admin from picking a number outside the game's range.

---

### reveal(secret_number)

**Purpose:** Admin reveals the secret number. The program cryptographically verifies it matches the committed hash.

**Account validation:**

- `game` -- mutable (we're writing to it).
- `admin` -- must be a signer.

**Security checks (in order):**

1. **Admin-only:** The signer must match the game's stored admin.

   ```rust
   require!(
       ctx.accounts.admin.key() == game.admin,
       GameError::Unauthorized
   );
   ```

2. **Range check:** The revealed number must still be in 1-100.

3. **Not already revealed:** Prevents calling reveal twice.

   ```rust
   require!(!game.is_revealed, GameError::AlreadyRevealed);
   ```

4. **Not finished:** Cannot reveal a finished game.

5. **Hash verification:** The critical check.

   ```rust
   let hash = blake3::hash(&secret_number.to_le_bytes());
   require!(
       hash.as_bytes() == &game.secret_hash,
       GameError::HashMismatch
   );
   ```

   If the admin tries to reveal a different number than what they committed, the hashes won't match, and the transaction fails.

**What changes:**

- `secret_number` is set to the actual number.
- `is_revealed` is set to `true`.

**Possible errors:** `Unauthorized`, `InvalidSecretRange`, `AlreadyRevealed`, `GameFinished`, `HashMismatch`.

---

### guess(guess)

**Purpose:** Any player can submit a guess. The program compares it to the secret and emits an event.

**Account validation:**

- `game` -- mutable (we're tracking attempts).
- `player` -- must be a signer (any wallet, no restriction).

**Security checks (in order):**

1. **Game not finished:** Cannot guess on a completed game.

2. **Secret revealed:** Cannot guess before the admin reveals.

3. **Attempts remaining:** Player has not exceeded `max_tries`.

**Comparison logic:**

```rust
match guess.cmp(&game.secret_number) {
    std::cmp::Ordering::Equal => {
        game.is_finished = true;
        emit!(GuessCorrect { guess, attempts: game.attempts });
    }
    std::cmp::Ordering::Less => {
        emit!(GuessTooSmall { guess, attempts: game.attempts });
    }
    std::cmp::Ordering::Greater => {
        emit!(GuessTooBig { guess, attempts: game.attempts });
    }
}
```

**Auto-finish:** After the guess, if `attempts >= max_tries` and the game isn't already finished, the game ends automatically:

```rust
if game.attempts >= game.max_tries && !game.is_finished {
    game.is_finished = true;
    emit!(GameOver { attempts, max_tries });
}
```

**Events emitted:**

| Event          | Condition                    |
|----------------|------------------------------|
| GuessTooSmall  | guess < secret_number        |
| GuessTooBig    | guess > secret_number        |
| GuessCorrect   | guess == secret_number       |
| GameOver       | attempts >= max_tries (miss) |

---

## Security Model

### What commit-reveal prevents

**Without commit-reveal:** The admin could see all guesses and silently change the secret number to always stay one step ahead. Since block data is public, the admin could change the number after seeing a correct guess.

**With commit-reveal:** The admin commits a `blake3` hash at initialization. The hash is one-way -- nobody can derive the secret from the hash. When the admin reveals, the program verifies `blake3(secret) == stored_hash`. If the admin changed the number, the hash won't match, and the transaction fails.

### Security properties

| Property | Mechanism |
|----------|-----------|
| Admin cannot change the number | blake3 hash is stored at init, verified at reveal |
| Secret is hidden until reveal | blake3 is a one-way function |
| Only admin can reveal | Signer check against stored admin pubkey |
| Limited guessing | Max 10 attempts, enforced on-chain |
| Deterministic game address | PDA seeds `[b"game", admin.key()]` |

### Known limitation: trust-on-admin

The admin **knows** the answer. They chose it. This is the fundamental limitation of Phase 1. If the admin is also playing, they have an unfair advantage.

**Phase 2 solution:** Replace the admin-chosen number with a VRF oracle (e.g., Switchboard or Pyth). The oracle generates a provably random number that nobody -- including the admin -- knows in advance.

---

## Account Space Calculation

Anchor accounts have an 8-byte discriminator prefix, then the serialized data fields:

```
 8  discriminator   (Anchor internal)
32  admin           (Pubkey)
32  secret_hash     ([u8; 32] -- blake3 output)
 1  secret_number   (u8)
 1  is_revealed     (bool)
 1  attempts        (u8)
 1  max_tries       (u8)
 1  is_finished     (bool)
 1  bump            (u8)
---
78  total bytes
```

The program uses `#[derive(InitSpace)]` on the `Game` struct, so Anchor computes `Game::INIT_SPACE` automatically. The account is allocated with `space = 8 + Game::INIT_SPACE`.

---

## Error Codes

| Code | Name | Message |
|------|------|---------|
| 6000 | Unauthorized | Only the game admin can perform this action |
| 6001 | InvalidSecretRange | Secret number must be between 1 and 100 |
| 6002 | HashMismatch | Invalid hash - secret does not match committed hash |
| 6003 | NotRevealed | Game has not been revealed yet |
| 6004 | AlreadyRevealed | Secret has already been revealed |
| 6005 | GameFinished | Game is already finished |
| 6006 | NoAttemptsRemaining | No more attempts remaining |

Anchor custom errors start at code `6000` (offset from the Anchor error base of `6000`).

---

## Testing

The test suite uses **LiteSVM** -- a lightweight Solana VM that runs the actual compiled BPF bytecode without needing a local validator.

**8 tests covering happy path, security, and edge cases:**

| Test | What it verifies |
|------|-----------------|
| `test_initialize` | Game created with correct hash, admin, defaults |
| `test_reveal` | Secret revealed, hash verified, fields updated |
| `test_guess_correct` | Correct guess finishes game, 1 attempt |
| `test_guess_too_small` | Guess too small, game continues |
| `test_guess_too_big` | Guess too big, game continues |
| `test_guess_game_over` | 10 wrong guesses triggers GameOver, 11th fails |
| `test_unauthorized_reveal` | Non-admin cannot reveal (signer check) |
| `test_hash_mismatch` | Wrong secret fails hash verification |

**Run tests:**

```bash
cargo test --manifest-path on-chain/programs/on-chain/Cargo.toml
```

All 8 tests should pass:

```
[PASS] test_initialize
[PASS] test_reveal
[PASS] test_guess_correct
[PASS] test_guess_too_small
[PASS] test_guess_too_big
[PASS] test_guess_game_over
[PASS] test_unauthorized_reveal
[PASS] test_hash_mismatch
```

---

## Playing

### Local (LiteSVM)

Play the game locally using the real compiled BPF program inside LiteSVM:

```bash
cargo run --manifest-path on-chain/programs/on-chain/Cargo.toml --features play --bin play
```

This launches an interactive terminal game:

1. Admin initializes the game (commits blake3 hash of secret).
2. Admin reveals the secret (program verifies hash).
3. You guess the number interactively (1-100, 10 attempts).

### How the play binary works

The `play.rs` binary:

- Loads the compiled `.so` file from `target/deploy/`.
- Creates an admin keypair and a player keypair.
- Airdrops SOL to both (LiteSVM, so it's instant).
- Sends real Anchor instructions through LiteSVM transactions.
- Parses transaction logs to show feedback (too small / too big / correct).
- Reads your guesses from stdin.

---

## What's Next (Phase 2)

Phase 1 proves the on-chain game architecture works: accounts, instructions, events, security checks. But the admin still **chooses** the number.

**Phase 2 will replace the admin-chosen number with a VRF oracle:**

- Use Switchboard or Pyth VRF to generate a provably random number on-chain.
- Nobody -- not even the admin -- knows the number in advance.
- The commit-reveal pattern stays, but the "commit" comes from the oracle's cryptographic commitment, not an admin transaction.
- Full trustless randomness: the oracle's math proof guarantees the number was not manipulated.

This mirrors the progression described in the [On-Chain Randomness Lesson](on-chain-randomness-lesson.md): from broken `rand` -> commit-reveal -> VRF oracle.
