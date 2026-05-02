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

### How It Works

1. **Admin** starts a game -- the program requests a random secret number (1-100) from a VRF oracle
2. **Player** submits guesses -- the program responds with "too small", "too big", or "you win!"
3. Game tracks attempts and enforces a max-try limit

### Build Phases

| Phase | What | Status |
|-------|------|--------|
| **Phase 1** | Core game with Anchor (commit-reveal for the secret, no VRF yet) | Up Next |
| **Phase 2** | Upgrade to Switchboard VRF for real on-chain randomness | Planned |

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

- [On-Chain Randomness & Security Teaching Guide](docs/on-chain-randomness-lesson.md)
- [The Rust Programming Language - Ch.2](https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html)
- [Anchor Documentation](https://www.anchor-lang.com/)
- [solana_rbpf crate](https://docs.rs/solana_rbpf/latest/solana_rbpf/)

---
*Created as part of the Turbine task series.*
