# Solana On-Chain Randomness & Off-Chain Security

> A teaching guide: *"Why can't we use `rand` in a Solana program?"*

---

## Table of Contents

1. [Why Random Numbers Don't Work on Solana](#1-why-random-numbers-dont-work-on-solana)
2. [How We Solve It: VRF Oracles](#2-how-we-solve-it-vrf-oracles)
3. [The Oracle Problem: Can We Trust Outside Data?](#3-the-oracle-problem-can-we-trust-outside-data)
4. [Attacks and How to Stop Them](#4-attacks-and-how-to-stop-them)
5. [Building a Safe Guessing Game](#5-building-a-safe-guessing-game)
6. [Cheat Sheet](#6-cheat-sheet)
7. [Deep Dive: Why `rand` Can't Even Compile](#7-deep-dive-why-rand-cant-even-compile)
8. [References](#8-references)

---

## 1. Why Random Numbers Don't Work on Solana

### Think of It Like This

Imagine you and 3 friends are playing a game. The rule is: **everyone must get the same answer**, or the game is broken.

Now imagine the game says "pick a random number." You pick `42`, your friend picks `73`, another picks `15`. Everyone got a **different** number. The game is broken!

That is exactly what happens on Solana.

```mermaid
flowchart LR
    A[One transaction] --> B[Computer A says 42]
    A --> C[Computer B says 73]
    A --> D[Computer C says 15]

    B --> E{Same answer?}
    C --> E
    D --> E

    E -->|No| F[GAME OVER]
    E -->|Yes| G[GAME OK]

    style F fill:#ff4444,color:#fff
    style G fill:#44bb44,color:#fff
```

On Solana, there is no single computer. There are **hundreds of computers** (called validators). They ALL must get the **same answer** for every transaction. Random numbers give different answers on different computers. So random numbers break the system.

### Three Big Reasons `rand` Fails

```mermaid
flowchart TD
    A[Why rand fails] --> B[Different computers\nget different numbers]
    A --> C[No keyboard or mouse\ninside Solana programs]
    A --> D[The boss computer\ncould cheat]

    B --> E[Computers disagree]
    C --> F[Cannot build the code]
    D --> G[Not fair]

    style A fill:#6c5ce7,color:#fff
    style E fill:#ff4444,color:#fff
    style F fill:#ff4444,color:#fff
    style G fill:#ff4444,color:#fff
```

| Problem | Simple Explanation |
|---------|-------------------|
| Different answers | Each computer picks its own random number. They disagree. |
| No system access | Solana programs live in a tiny box with no keyboard, no mouse, no random source. |
| Cheating | Even if it worked, the computer running the game could pick a number that helps it win. |

---

## 2. How We Solve It: VRF Oracles

### The Idea

Instead of asking the Solana computer to pick a random number, we ask a **trusted helper outside** (called an **oracle**) to do it. But the helper must bring a **mathematical proof** that the number is truly random and not cheating.

Think of it like a teacher who picks a number and writes it in a **sealed envelope**. When it's time to reveal, everyone can check that the envelope was never opened or changed.

```mermaid
sequenceDiagram
    participant You
    participant Game as Solana Game
    participant Helper as Oracle Helper

    You->>Game: I want to play!
    Game->>Helper: Give me a random number
    Helper->>Helper: Pick number + write proof
    Helper->>Game: Here is the number + proof
    Game->>Game: Check proof is real
    Game-->>You: Game is ready!
```

### Why This Is Safe

```mermaid
flowchart LR
    A[VRF Oracle] --> B[Cannot be predicted]
    A --> C[Proof can be checked]
    A --> D[Even the oracle\ncannot cheat]
    A --> E[Changing the number\nbreaks the proof]

    style A fill:#6c5ce7,color:#fff
    style B fill:#4ecdc4,color:#fff
    style C fill:#4ecdc4,color:#fff
    style D fill:#4ecdc4,color:#fff
    style E fill:#4ecdc4,color:#fff
```

---

## 3. The Oracle Problem: Can We Trust Outside Data?

### The Big Question

When we ask a helper outside Solana for data, how do we know they are telling the truth?

Your teacher told you: **"If you go outside, you must lock the door as tightly as possible."** That means: if you use outside data, you must verify it as much as possible.

### The Trust Ladder

```mermaid
flowchart TD
    A["[SAFEST]\nData already on Solana\nNo trust needed"] --> B["[SAFE]\nVRF Oracle with proof\nTrust but verify"]
    B --> C["[RISKY]\nExternal website\nMust trust them"]
    C --> D["[DANGEROUS]\nOne single server\nIf it breaks, you break"]

    style A fill:#44bb44,color:#fff
    style B fill:#bbbb44,color:#fff
    style C fill:#ff9944,color:#fff
    style D fill:#ff4444,color:#fff
```

### Two Ways to Use Outside Data

```mermaid
flowchart LR
    A[Need outside data?] --> B{How?}

    B --> C[Trust blindly]
    B --> D[Require proof]

    C --> E[If helper lies,\nyour game is broken]
    D --> F[Helper cannot lie\nbecause math catches them]

    style C fill:#ff6b6b,color:#fff
    style E fill:#ff4444,color:#fff
    style D fill:#4ecdc4,color:#fff
    style F fill:#44bb44,color:#fff
```

---

## 4. Attacks and How to Stop Them

### The 5 Bad Things That Can Happen

```mermaid
flowchart TD
    subgraph Bad Things
        A1["[ATTACK-1]\nSomeone peeks at the\nnumber before you"]
        A2["[ATTACK-2]\nSomeone sends a\nfake number"]
        A3["[ATTACK-3]\nSomeone uses an\nold number again"]
        A4["[ATTACK-4]\nSomeone changes the\nnumber on the way"]
        A5["[ATTACK-5]\nThe only helper goes\ndown or turns evil"]
    end

    style A1 fill:#ff6b6b,color:#fff
    style A2 fill:#ff6b6b,color:#fff
    style A3 fill:#ff6b6b,color:#fff
    style A4 fill:#ff6b6b,color:#fff
    style A5 fill:#ff6b6b,color:#fff
```

### The 5 Fixes

```mermaid
flowchart TD
    subgraph Fixes
        F1["[FIX-1]\nCommit-Reveal:\nLock the answer first,\nshow it later"]
        F2["[FIX-2]\nProof Check:\nVerify the math\nbefore accepting"]
        F3["[FIX-3]\nFreshness Check:\nReject old data"]
        F4["[FIX-4]\nSignature Check:\nVerify who sent it"]
        F5["[FIX-5]\nMany Helpers:\nUse more than\none oracle"]
    end

    style F1 fill:#44bb44,color:#fff
    style F2 fill:#44bb44,color:#fff
    style F3 fill:#44bb44,color:#fff
    style F4 fill:#44bb44,color:#fff
    style F5 fill:#44bb44,color:#fff
```

### Attack vs Fix Map

| Attack | What Happens | Fix | How It Stops It |
|--------|-------------|-----|----------------|
| [ATTACK-1] Peeking | Bad person sees the number in the waiting line and acts first | [FIX-1] Commit-Reveal | Lock the answer in a box first. Nobody can peek until it's locked. |
| [ATTACK-2] Fake number | Someone sends a made-up number to your game | [FIX-2] Proof Check | The number must come with math proof. Fake numbers fail the math test. |
| [ATTACK-3] Old number | Someone sends an old number that was valid before | [FIX-3] Freshness Check | Your game checks the time. Old numbers get rejected. |
| [ATTACK-4] Changed number | Someone changes the number while it's traveling | [FIX-4] Signature Check | The number is signed. If it changes, the signature breaks. |
| [ATTACK-5] One helper fails | Your only helper breaks or turns evil | [FIX-5] Many Helpers | Use several helpers. If one is bad, the others catch it. |

### Commit-Reveal: The Most Important Fix

Think of it like this:

1. **COMMIT** -- The helper writes a number on paper, puts it in a locked box, and gives you the box. Nobody can see the number. But the box has a label (a hash) so nobody can swap the paper.

2. **REVEAL** -- Later, the helper opens the box and shows the number with proof. You check: does this match the label on the box? If yes, it's safe.

```mermaid
sequenceDiagram
    participant You
    participant Game as Solana Game
    participant Helper as Oracle

    Note over You,Helper: Step 1 -- LOCK
    You->>Game: I want a random number
    Helper->>Helper: Pick secret number X
    Helper->>Helper: Put X in a locked box
    Helper->>Game: Here is the locked box

    Note over You,Helper: Step 2 -- OPEN
    Helper->>Game: Here is the key + proof
    Game->>Game: Does the key match the box?
    Game->>Game: Is the proof valid?
    Game-->>You: Number is safe to use!
```

### What Happens Without Commit-Reveal

```mermaid
sequenceDiagram
    participant BadGuy as Bad Guy
    participant WaitingLine as Waiting Line
    participant Game as Solana Game
    participant Helper as Oracle

    Helper->>WaitingLine: The number is 42
    Note over WaitingLine: Everyone can see it here

    BadGuy->>WaitingLine: I see it will be 42!
    BadGuy->>WaitingLine: My guess is 42! I pay more!
    WaitingLine->>BadGuy: Bad guy goes first
    WaitingLine->>Game: Now process the oracle

    Note over BadGuy,Game: The bad guy wins every time!
```

---

## 5. Building a Safe Guessing Game

### The Full Picture

```mermaid
flowchart TB
    subgraph On-Chain["Your Game (On Solana)"]
        A[Player starts] --> B[Ask oracle for number]
        B --> C[Save the lock]

        D[Oracle replies] --> E[Check proof]
        E --> F[Check who sent it]
        F --> G[Check it is fresh]
        H[Check the lock matches] --> I[Number is safe!]

        J[Player guesses] --> K[Compare guess to number]
    end

    subgraph Off-Chain["Oracle (Outside Solana)"]
        B -->|Ask| L[Oracle Network]
        L -->|Reply with proof| D
    end

    style A fill:#6c5ce7,color:#fff
    style D fill:#6c5ce7,color:#fff
    style J fill:#6c5ce7,color:#fff
    style I fill:#44bb44,color:#fff
    style K fill:#44bb44,color:#fff
    style L fill:#4ecdc4,color:#fff
```

### The Security Gate

Every time outside data arrives, your game should run it through this checklist:

```mermaid
flowchart TD
    A[Outside data arrives] --> B{Proof valid?}
    B -->|No| X[REJECT]
    B -->|Yes| C{From trusted oracle?}
    C -->|No| X
    C -->|Yes| D{Fresh enough?}
    D -->|No| X
    D -->|Yes| E{Signature valid?}
    E -->|No| X
    E -->|Yes| F{Lock matches?}
    F -->|No| X
    F -->|Yes| G[ACCEPT]

    style X fill:#ff4444,color:#fff
    style G fill:#44bb44,color:#fff
    style A fill:#6c5ce7,color:#fff
```

If ANY check fails, throw the data away. Only accept if ALL checks pass.

### What the Code Looks Like

```rust
fn process_randomness(ctx: Context<ReceiveRandomness>, result: u64) -> Result<()> {
    // [CHECK-1] Is the math proof valid?
    verify_vrf_proof(&ctx.accounts.vrf_account, result)?;

    // [CHECK-2] Did this come from the right oracle?
    require!(
        ctx.accounts.oracle.key() == EXPECTED_ORACLE_PUBKEY,
        ErrorCode::UnauthorizedOracle
    );

    // [CHECK-3] Is this data fresh, not old?
    let current_slot = Clock::get()?.slot;
    require!(
        current_slot - ctx.accounts.vrf_account.last_update_slot < MAX_STALENESS,
        ErrorCode::StaleData
    );

    // [CHECK-4] Does the locked box match?
    require!(
        hash(result.to_le_bytes()) == ctx.accounts.commitment.hash,
        ErrorCode::CommitmentMismatch
    );

    // All checks passed! Safe to use.
    game.secret_number = (result % 100) + 1;

    Ok(())
}
```

---

## 6. Cheat Sheet

### Quick Summary

```mermaid
mindmap
  root((Random on Solana))
    Problem
      rand breaks agreement
      No keyboard or mouse inside
      Boss could cheat
    Solution
      Ask an outside oracle
      Oracle brings a math proof
    Security
      Check the proof
      Check who sent it
      Check it is fresh
      Check the signature
      Use commit-reveal
    Rule
      Trust Nothing
      Verify Everything
```

### Which Approach Should You Use?

```mermaid
flowchart TD
    A[Need randomness?] --> B{Is money involved?}
    B -->|Yes| C[Use Switchboard VRF\n+ Commit-Reveal\n+ All checks]
    B -->|No| D{OK if slightly unfair?}
    D -->|No| C
    D -->|Yes| E[Use slot hash\nbut validators can cheat]

    style C fill:#44bb44,color:#fff
    style E fill:#ff9944,color:#fff
```

### 8 Rules to Remember

| # | Rule | Why |
|---|------|-----|
| 1 | Never use `rand` on-chain | Computers will disagree and break |
| 2 | Use VRF oracles | They give random numbers with proof |
| 3 | Verify everything | Never trust outside data blindly |
| 4 | Use commit-reveal | Stops people from peeking |
| 5 | Check timestamps | Stops people from using old data |
| 6 | Check who sent it | Only trust known oracles |
| 7 | Check signatures | Stops tampered data |
| 8 | Use many oracles | Stops one bad oracle from ruining everything |

---

## 7. Deep Dive: Why `rand` Can't Even Compile

### What Is rBPF?

Solana programs do not run on a real computer. They run inside a **tiny pretend computer** called **rBPF**. Think of it like a video game console that can only run certain games.

> **[REF]** [solana_rbpf crate documentation](https://docs.rs/solana_rbpf/latest/solana_rbpf/) -- *"Virtual machine and JIT compiler for eBPF programs."*

### Your Laptop vs. Solana

```mermaid
flowchart LR
    subgraph Your Laptop
        A[Rust Program] --> B[Standard Library]
        B --> C[macOS or Linux]
        C --> D[Real hardware]
    end

    subgraph Solana
        E[Game Program] --> F[rBPF pretend computer]
        F --> G[Only allowed tools]
        G --> H[No keyboard\nNo mouse\nNo random]
    end

    style D fill:#44bb44,color:#fff
    style H fill:#ff4444,color:#fff
    style F fill:#6c5ce7,color:#fff
```

On your laptop, `rand` works because it can ask the operating system for a random number. The operating system gets it from the hardware. But inside rBPF, there is no operating system and no hardware. Just a tiny empty room.

### What Can and Cannot Run Inside rBPF

```mermaid
flowchart LR
    subgraph ALLOWED
        A[Math: add, multiply]
        B[Read your own data]
        C[Call Solana tools]
        D[Same input = same output]
    end

    subgraph BLOCKED
        E[No internet]
        F[No files]
        G[No random source]
        H[No keyboard or mouse]
    end

    style A fill:#44bb44,color:#fff
    style B fill:#44bb44,color:#fff
    style C fill:#44bb44,color:#fff
    style D fill:#44bb44,color:#fff
    style E fill:#ff4444,color:#fff
    style F fill:#ff4444,color:#fff
    style G fill:#ff4444,color:#fff
    style H fill:#ff4444,color:#fff
```

### How rBPF Stops Bad Programs

```mermaid
flowchart TD
    A[You write a program] --> B[Compile it]
    B --> C{rBPF Checker}

    C -->|Bad| D[Unknown tools used]
    C -->|Bad| E[Infinite loop found]
    C -->|Bad| F[Memory trespassing]

    C -->|OK| G{Running checks}
    G --> H[Only whitelisted tools]
    G --> I[Only your own data]
    G --> J[Time limit per play]

    style D fill:#ff6b6b,color:#fff
    style E fill:#ff6b6b,color:#fff
    style F fill:#ff6b6b,color:#fff
    style H fill:#ff9944,color:#fff
    style I fill:#ff9944,color:#fff
    style J fill:#ff9944,color:#fff
    style C fill:#6c5ce7,color:#fff
    style G fill:#6c5ce7,color:#fff
```

### What Happens When You Try to Use `rand`

```mermaid
sequenceDiagram
    participant You
    participant Cargo as Cargo.toml
    participant Compiler as Compiler
    participant VM as rBPF

    You->>Cargo: Add rand = "0.8.5"
    Cargo->>Compiler: Build for Solana

    Note over Compiler: rand needs a random source
    Note over Compiler: Solana has no random source

    Compiler-->>You: ERROR: cannot find random source

    Note over You,VM: Even if it did compile...\nrBPF would block it at runtime

    VM-->>You: ERROR: unknown tool called
```

### Important rBPF Modules

From the [solana_rbpf docs](https://docs.rs/solana_rbpf/latest/solana_rbpf/):

| Module | What It Does | Why It Matters |
|--------|-------------|---------------|
| [`vm`](https://docs.rs/solana_rbpf/latest/solana_rbpf/vm/index.html) | The pretend computer that runs your program | Controls what can and cannot run |
| [`syscalls`](https://docs.rs/solana_rbpf/latest/solana_rbpf/syscalls/index.html) | The allowed list of tools | No randomness in this list |
| [`verifier`](https://docs.rs/solana_rbpf/latest/solana_rbpf/verifier/index.html) | Checks your program before it runs | Catches bad programs early |
| [`interpreter`](https://docs.rs/solana_rbpf/latest/solana_rbpf/interpreter/index.html) | Runs programs step by step | Another way to run your code |
| [`elf`](https://docs.rs/solana_rbpf/latest/solana_rbpf/elf/index.html) | Loads your compiled program | How your game gets into Solana |
| [`assembler`](https://docs.rs/solana_rbpf/latest/solana_rbpf/assembler/index.html) | Turns code into machine instructions | Low-level details |

---

## 8. References

### Primary

| Resource | Link | What It Is |
|----------|------|-----------|
| **solana_rbpf** | [docs.rs/solana_rbpf](https://docs.rs/solana_rbpf/latest/solana_rbpf/) | The pretend computer that runs Solana programs |
| **The Rust Book Ch.2** | [doc.rust-lang.org](https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html) | The guessing game tutorial this project is based on |

### Randomness Solutions

| Resource | Link | What It Is |
|----------|------|-----------|
| **Switchboard VRF** | [docs.switchboard.xyz](https://docs.switchboard.xyz/randomness) | A helper that gives random numbers with proof |
| **Pyth Entropy** | [pyth.network](https://pyth.network/entropy) | Another random number helper |

### Security

| Resource | Link | What It Is |
|----------|------|-----------|
| **Solana Security Checklist** | [GitHub](https://github.com/solana-foundation/solana-dev-skill) | A list of things to check before going live |
| **Commit-Reveal Scheme** | [Wikipedia](https://en.wikipedia.org/wiki/Commitment_scheme) | The lock-then-open trick explained |

### Solana Runtime

| Resource | Link | What It Is |
|----------|------|-----------|
| **Solana Runtime Docs** | [docs.solanalabs.com](https://docs.solanalabs.com/runtime) | How programs run on Solana |
| **eBPF on Solana** | [docs.solanalabs.com](https://docs.solanalabs.com/proposals/abi) | Technical details about the pretend computer |

---

> **"Trust nothing, verify everything."** -- The Golden Rule of Off-Chain Security
