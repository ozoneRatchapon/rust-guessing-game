# Rust Guessing Game

A guessing game that started as a CLI toy and is evolving into an **on-chain Solana program**.

---

## Demos

| # | Demo | Command | What |
|---|------|---------|------|
| 1 | Pure Rust CLI | `cargo run` | Original guessing game (Rust Book Ch.2) |
| 2 | Phase 1: Commit-Reveal | `cd on-chain && npx tsx scripts/play-devnet.ts` | Anchor program, admin commit-reveal on devnet |
| 3 | Phase 2: Switchboard VRF | `cd on-chain && npx tsx scripts/play-phase2-devnet.ts` | Trustless VRF randomness on devnet |
| 4 | Broken `rand` Proof | `cd on-chain && npx tsx scripts/build-broken-rand.ts` | `cargo build-sbf` fails = proof |

Or use the launcher: `cd on-chain && bash scripts/demo.sh`

---

## CLI Version

The original game, built from [The Rust Book Chapter 2](https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html).

### Objective

The game generates a secret random number between **1 and 100**. Your goal is to guess the number in as few attempts as possible. After each guess, the game tells you if your guess was too small, too big, or correct!

### How to Run

```bash
cargo run
```

**Prerequisites**: [Rust and Cargo](https://www.rust-lang.org/tools/install)

### What It Teaches

- Handling user input with `std::io`
- Variables and mutability (`let mut`)
- Using external crates via `Cargo.toml`
- Pattern matching with `match`
- Control flow using `loop` and `break`
- Error handling with `Result` and `Expect`

### How It Works

The CLI version uses the `rand` crate to pick a random number. That works fine on your laptop because your computer has access to a random source (like `/dev/urandom` on macOS/Linux). But this approach **does not work on Solana**.

---

## On-Chain Version

The same guessing game, rebuilt as a Solana program using [Anchor](https://www.anchor-lang.com/) with proper on-chain randomness.

### Why On-Chain Is Different

On Solana, hundreds of computers (validators) must all get the **same answer** for every transaction. Random numbers give different answers on different computers, which breaks the network. The `rand` crate cannot even compile for the Solana target because programs run inside a sandboxed VM ([rBPF](https://docs.rs/solana_rbpf/latest/solana_rbpf/)) with no OS access.

Instead, we use a **VRF oracle** (like Switchboard) that generates randomness off-chain and provides a cryptographic proof that can be verified on-chain. This is covered in detail in our [teaching guide](docs/on-chain-randomness-lesson.md).

### How It Works (Phase 1: Commit-Reveal)

1. **Admin** starts a game -- commits a `blake3` hash of a secret number (1-100) on-chain
2. **Admin** reveals the secret -- program verifies the hash matches, then stores the number
3. **Player** submits guesses -- the program responds with "too small", "too big", or "you win!"
4. Game tracks attempts and enforces a 10-try limit

> Phase 2 is a **separate program** (`phase2-vrf`) that uses a VRF oracle for trustless randomness. Phase 1 stays deployed and demo-able.

### Build Phases

| Phase | What | Status |
|-------|------|--------|
| **Phase 1** | Commit-reveal with Anchor | Done |
| **Phase 2** | Switchboard VRF (separate program) | Done |
| **Bonus** | Broken `rand` demo | Done |

### How to Build and Test

```bash
# Run all tests (Phase 1 + Phase 2 + broken-rand proof)
bash test.sh

# Or run individually:
bash test.sh phase1       # 8 LiteSVM tests
bash test.sh phase2       # 16 LiteSVM tests
bash test.sh broken-rand  # Proves rand fails on BPF

# Build the on-chain program (BPF)
cargo build-sbf --manifest-path on-chain/programs/on-chain/Cargo.toml

# Generate IDL (needed for devnet play scripts)
cd on-chain && anchor build --skip-lint
```

### How to Play

Each phase has its own playable experience:

| Mode | Command | Network | Explorer |
|------|---------|---------|----------|
| Local (LiteSVM) | `cargo run --manifest-path on-chain/programs/on-chain/Cargo.toml --features play --bin play` | In-memory VM | No |
| Devnet Phase 1 | `cd on-chain && npx tsx scripts/play-devnet.ts` | Devnet | Yes |
| Devnet Phase 2 | `cd on-chain && npx tsx scripts/play-phase2-devnet.ts` | Devnet | Yes |

**Local mode** uses LiteSVM -- instant, no network needed. Runs the actual compiled BPF program.

**Devnet mode** sends real transactions to Solana devnet. Each tx gets an Explorer link:
```
https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet
```

### On-Chain Project Structure

```
on-chain/
  programs/
    on-chain/            ← Phase 1 (commit-reveal)
      src/
        lib.rs                          # Program entry (initialize, reveal, guess)
        state.rs                        # Game account + event structs
        error.rs                        # GameError enum (7 error codes)
        constants.rs                    # MAX_TRIES=10, range 1-100
        instructions/
          initialize.rs                 # Admin creates game, commits blake3 hash
          reveal.rs                     # Admin reveals secret, program verifies hash
          guess.rs                      # Player guesses, program responds
          close_game.rs                 # Admin closes game, recovers rent
      tests/
        test_initialize.rs              # 8 tests (init, reveal, guess, security)
    phase2-vrf/            ← Phase 2 (Switchboard VRF)
      tests/
        test_phase2.rs                  # 16 tests (init, settle, guess, security, mock VRF)
  demos/
    broken-rand/           ← Standalone program: proves rand fails on-chain
  scripts/
    play-devnet.ts                    ← Phase 1 interactive script
    play-phase2-devnet.ts             ← Phase 2 interactive script
    build-broken-rand.ts              ← Build broken-rand, shows error
    demo.sh                           ← Menu launcher for all demos
```

### Instructions

| Instruction | Who | What |
|-------------|-----|------|
| `initialize(secret_number)` | Admin | Creates game PDA, stores `blake3(secret)` as commitment |
| `reveal(secret_number)` | Admin | Reveals the secret, program verifies hash matches commitment |
| `guess(guess)` | Anyone | Submits a guess (1-100), gets too-small/too-big/correct response |
| `close_game()` | Admin | Closes game account, recovers rent lamports to admin |

### Deployed on Devnet

| Program | ID | Explorer |
|---------|----|----------|
| Phase 1 (commit-reveal) | `3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT` | [View](https://explorer.solana.com/address/3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT?cluster=devnet) |
| Phase 2 (Switchboard VRF) | `CHXkyr3GrLvWRXdbnYgPMKhwU1dYF6gW9aUpV8S3oTJw` | [View](https://explorer.solana.com/address/CHXkyr3GrLvWRXdbnYgPMKhwU1dYF6gW9aUpV8S3oTJw?cluster=devnet) |

To redeploy:
```bash
solana program deploy on-chain/target/deploy/on_chain.so --url devnet
solana program deploy on-chain/target/deploy/phase2_vrf.so --url devnet \
  --program-id on-chain/target/deploy/phase2_vrf-keypair.json
```

### Security Model (Phase 1)

- **Commit-reveal**: Admin commits a hash at `initialize`, reveals at `reveal`. Program verifies `blake3(secret) == stored_hash`.
- **No secret stored until reveal**: The `secret_number` field is `0` until the admin reveals.
- **Max 10 attempts**: Game auto-finishes when attempts exhausted.
- **Admin-only reveal**: Only the game admin can reveal the secret.

> Note: Phase 1 uses a trust-on-admin model. Phase 2 is a **separate program** (`phase2-vrf`) that uses Switchboard VRF for trustless randomness. Both programs coexist on devnet.

### Architecture Decision: Why Phase 2 Is Separate

Phase 2 lives in its own Anchor program (`phase2-vrf`), not as an upgrade to Phase 1. Here's why:

- **Phase 1 stays demo-able forever** -- already deployed on devnet, students can interact with it at any time
- **Different instructions** -- Phase 2 drops `reveal` (admin reveals secret) and adds `settle_random` (VRF callback). The instruction set is fundamentally different
- **Side-by-side teaching** -- students see both approaches (commit-reveal vs. VRF) and understand the trade-offs

### Cost to Play

Every transaction costs a flat 5,000 lamports fee (~$0.00075 at $150/SOL).

#### Phase 1: Commit-Reveal

| Action | Fee | Compute Units |
|--------|----:|--------------:|
| close_game | 5,000 lamports | ~3,858 |
| initialize | 5,000 lamports | ~10,018 |
| reveal | 5,000 lamports | ~5,435 |
| guess | 5,000 lamports | ~2,770 |

Rent for the game account (78 bytes) is ~0.0014 SOL, fully recoverable via `close_game`. A full game session (5 guesses) costs ~0.00004 SOL in fees — less than a penny.

#### Phase 2: Switchboard VRF

| Action | Fee | Compute Units | Notes |
|--------|----:|--------------:|-------|
| initialize | 10,000 lamports | ~121,148 | Includes Switchboard VRF instruction |
| settle_random | 5,000 lamports | ~49,836 | VRF reveal + secret derivation |
| guess | 5,000 lamports | ~2,992 | Per guess |
| close_game | 5,000 lamports | ~5,396 | Rent recovery |

Phase 2 `initialize` is a multi-instruction transaction: it creates a Switchboard randomness account and then calls our `initialize` instruction in the same transaction, hence the double fee (10,000 lamports). A full Phase 2 session (init + settle + 5 guesses + close) costs ~0.00006 SOL in fees.

### What It Teaches

- Solana account model and program architecture
- Why `rand` breaks consensus (determinism requirement)
- VRF oracles and the commit-reveal scheme
- Off-chain data security: proof, signature, freshness checks
- The rBPF virtual machine and its constraints
- Anchor fundamentals: accounts, instructions, errors, and testing

---

## Built With

- **Rust** -- The programming language
- **Anchor** -- Solana program framework
- **Switchboard VRF** -- On-chain randomness oracle (Phase 2)
- **rand** crate -- For the CLI version only

## References

- [Phase 1: Commit-Reveal Walkthrough](docs/phase1-commit-reveal.md)
- [On-Chain Randomness & Security Teaching Guide](docs/on-chain-randomness-lesson.md)
- [The Rust Programming Language - Ch.2](https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html)
- [Anchor Documentation](https://www.anchor-lang.com/)
- [solana_rbpf crate](https://docs.rs/solana_rbpf/latest/solana_rbpf/)

---
*Created as part of the Turbine task series.*
