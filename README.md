# Rust Guessing Game

A guessing game that started as a CLI toy and is evolving into an **on-chain Solana program**.

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

## On-Chain Version (In Progress)

The same guessing game, rebuilt as a Solana program using [Anchor](https://www.anchor-lang.com/) with proper on-chain randomness.

### Why On-Chain Is Different

On Solana, hundreds of computers (validators) must all get the **same answer** for every transaction. Random numbers give different answers on different computers, which breaks the network. The `rand` crate cannot even compile for the Solana target because programs run inside a sandboxed VM ([rBPF](https://docs.rs/solana_rbpf/latest/solana_rbpf/)) with no OS access.

Instead, we use a **VRF oracle** (like Switchboard) that generates randomness off-chain and provides a cryptographic proof that can be verified on-chain. This is covered in detail in our [teaching guide](docs/on-chain-randomness-lesson.md).

### How It Works (Phase 1: Commit-Reveal)

1. **Admin** starts a game -- commits a `blake3` hash of a secret number (1-100) on-chain
2. **Admin** reveals the secret -- program verifies the hash matches, then stores the number
3. **Player** submits guesses -- the program responds with "too small", "too big", or "you win!"
4. Game tracks attempts and enforces a 10-try limit

> In Phase 2, step 1-2 will be replaced by a VRF oracle for trustless randomness.

### Build Phases

| Phase | What | Status |
|-------|------|--------|
| **Phase 1** | Core game with Anchor (commit-reveal for the secret, no VRF yet) | Done |
| **Phase 2** | Upgrade to Switchboard VRF for real on-chain randomness | Planned |

### How to Build and Test

```bash
# Build the on-chain program (BPF)
cargo build-sbf --manifest-path on-chain/programs/on-chain/Cargo.toml

# Run all tests (8 tests using LiteSVM)
cargo test --manifest-path on-chain/programs/on-chain/Cargo.toml

# Generate IDL (needed for devnet play script)
cd on-chain && anchor build --skip-lint
```

### How to Play

Each phase has its own playable experience:

| Mode | Command | Network | Explorer |
|------|---------|---------|----------|
| Local (LiteSVM) | `cargo run --manifest-path on-chain/programs/on-chain/Cargo.toml --features play --bin play` | In-memory VM | No |
| Devnet (Explorer) | `cd on-chain && yarn play:devnet` | Devnet | Yes |

**Local mode** uses LiteSVM -- instant, no network needed. Runs the actual compiled BPF program.

**Devnet mode** sends real transactions to Solana devnet. Each tx gets an Explorer link:
```
https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet
```

### On-Chain Project Structure

```
on-chain/
  programs/on-chain/src/
    lib.rs                          # Program entry (initialize, reveal, guess)
    state.rs                        # Game account + event structs
    error.rs                        # GameError enum (7 error codes)
    constants.rs                    # MAX_TRIES=10, range 1-100
    instructions/
      initialize.rs                 # Admin creates game, commits blake3 hash
      reveal.rs                     # Admin reveals secret, program verifies hash
      guess.rs                      # Player guesses, program responds
      close_game.rs                 # Admin closes game, recovers rent
  programs/on-chain/tests/
    test_initialize.rs              # 8 tests (init, reveal, guess, security)
```

### Instructions

| Instruction | Who | What |
|-------------|-----|------|
| `initialize(secret_number)` | Admin | Creates game PDA, stores `blake3(secret)` as commitment |
| `reveal(secret_number)` | Admin | Reveals the secret, program verifies hash matches commitment |
| `guess(guess)` | Anyone | Submits a guess (1-100), gets too-small/too-big/correct response |
| `close_game()` | Admin | Closes game account, recovers rent lamports to admin |

### Deploy to Devnet

The program is deployed on devnet:
```
Program ID: 3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT
Explorer:   https://explorer.solana.com/address/3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT?cluster=devnet
```

To redeploy:
```bash
solana program deploy on-chain/target/deploy/on_chain.so --url devnet
solana program show --url devnet 3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT
```

### Security Model (Phase 1)

- **Commit-reveal**: Admin commits a hash at `initialize`, reveals at `reveal`. Program verifies `blake3(secret) == stored_hash`.
- **No secret stored until reveal**: The `secret_number` field is `0` until the admin reveals.
- **Max 10 attempts**: Game auto-finishes when attempts exhausted.
- **Admin-only reveal**: Only the game admin can reveal the secret.

> Note: Phase 1 uses a trust-on-admin model. Phase 2 replaces this with Switchboard VRF for trustless randomness.

### Cost to Play

Every transaction costs a flat 5,000 lamports fee (~$0.00075 at $150/SOL).

| Action | Fee | Compute Units |
|--------|----:|--------------:|
| close_game | 5,000 lamports | ~3,858 |
| initialize | 5,000 lamports | ~10,018 |
| reveal | 5,000 lamports | ~5,435 |
| guess | 5,000 lamports | ~2,770 |

Rent for the game account (78 bytes) is ~0.0014 SOL, fully recoverable via `close_game`. A full game session (5 guesses) costs ~0.00004 SOL in fees — less than a penny.

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
