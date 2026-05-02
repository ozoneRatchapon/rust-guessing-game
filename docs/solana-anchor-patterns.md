# Solana + Anchor Patterns Guide

> Distilled from the [Rust Guessing Game](https://github.com/ozoneRatchapon/rust-guessing-game) project.
> Give this to an LLM as context when starting a new Anchor project.

---

## Table of Contents

1. [Project Structure](#1-project-structure)
2. [Program Architecture](#2-program-architecture)
3. [Account Validation](#3-account-validation)
4. [Error Handling](#4-error-handling)
5. [Testing with LiteSVM](#5-testing-with-litesvm)
6. [External Oracles (VRF)](#6-external-oracles-vrf)
7. [Build, Test, Deploy](#7-build-test-deploy)
8. [Quick-Start Checklist](#8-quick-start-checklist)

---

## 1. Project Structure

### Anchor Multi-Program Workspace

```
project-root/
├── Cargo.toml                    # Workspace root: members = ["programs/*"]
├── test.sh                       # Unified test runner
├── Anchor.toml                   # Program IDs, cluster, test runner
├── rust-toolchain.toml           # Pin Solana-compatible Rust version
└── programs/
    ├── program-a/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs            # declare_id! + #[program] dispatch
    │       ├── state.rs          # #[account] structs + #[event] structs
    │       ├── error.rs          # #[error_code] enum
    │       ├── constants.rs      # pub const values
    │       ├── instructions.rs   # mod + pub use re-exports
    │       └── instructions/
    │           ├── initialize.rs
    │           ├── action.rs
    │           └── close.rs
    └── program-b/
        └── ... (same layout)
```

### Workspace `Cargo.toml`

```toml
[workspace]
members = ["programs/*"]
resolver = "2"

[workspace.dependencies]
getrandom = { version = "0.2", features = ["custom"] }

[profile.release]
overflow-checks = true
lto = "fat"
codegen-units = 1

[profile.release.build-override]
opt-level = 3
incremental = false
codegen-units = 1
```

Key points:
- `members = ["programs/*"]` auto-discovers all programs
- Release profile is hardened for on-chain deployment
- `getrandom` with `custom` feature avoids BPF compilation errors when deps pull it in

### Per-Program `Cargo.toml`

```toml
[lib]
crate-type = ["cdylib", "lib"]
name = "program_a"

[dependencies]
anchor-lang = "1.0.1"
blake3 = "1"

[dev-dependencies]
litesvm = "0.7"
solana-message = "2"
solana-transaction = "2"
```

- `cdylib` → `.so` BPF binary for on-chain deployment
- `lib` → usable as Rust library (tests, binaries)
- Dev deps only for LiteSVM testing

### `Anchor.toml`

```toml
[features]
seeds = true

[programs.localnet]
program_a = "ProgramID11111111111111111111111111111111"

[registry]
url = "https://api.apr.dev"

[provider]
cluster = "localnet"
wallet = "~/.config/solana/id.json"

[scripts]
test = "cargo test"
```

---

## 2. Program Architecture

### Module Organization — One File Per Instruction

```
src/
├── lib.rs            # Thin router — delegates to handlers
├── state.rs          # Account structs + events
├── error.rs          # Error enum
├── constants.rs      # Constants
├── instructions.rs   # mod + pub use (index file)
└── instructions/
    ├── initialize.rs # Initialize accounts struct + handler
    ├── action.rs
    └── close.rs
```

### `lib.rs` — Thin Dispatch

```rust
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("ProgramID11111111111111111111111111111111");

#[program]
pub mod program_a {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, param: u8) -> Result<()> {
        initialize::handler(ctx, param)
    }

    pub fn action(ctx: Context<Action>, param: u8) -> Result<()> {
        action::handler(ctx, param)
    }

    pub fn close(ctx: Context<Close>) -> Result<()> {
        close::handler(ctx)
    }
}
```

**Rule:** No business logic in `lib.rs`. It's a router only.

### `instructions.rs` — Index File

```rust
pub mod initialize;
pub mod action;
pub mod close;

pub use initialize::*;
pub use action::*;
pub use close::*;
```

### Single Instruction File

```rust
// instructions/initialize.rs
use anchor_lang::prelude::*;
use crate::state::*;
use crate::error::*;
use crate::constants::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + MyAccount::INIT_SPACE,
        seeds = [b"my_account", admin.key().as_ref()],
        bump,
    )]
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>, param: u8) -> Result<()> {
    let account = &mut ctx.accounts.my_account;
    account.admin = ctx.accounts.admin.key();
    account.value = param;
    account.bump = ctx.bumps.my_account;
    Ok(())
}
```

### State — Account Structs

```rust
// state.rs
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct MyAccount {
    pub admin: Pubkey,       // 32
    pub value: u8,           // 1
    pub is_active: bool,     // 1
    pub attempts: u8,        // 1
    pub bump: u8,            // 1
    // #[max_len(64)]        // use for Vec/String in InitSpace
    // pub name: String,
}
```

- `#[derive(InitSpace)]` auto-calculates space. Use `8 + T::INIT_SPACE` for the discriminator.
- For strings/vecs: `#[max_len(N)]` annotation.

### State Machine Pattern

Encode lifecycle via boolean flags:

```rust
pub struct GameState {
    pub is_initialized: bool,  // can't re-init
    pub is_revealed: bool,     // can't guess until true
    pub is_finished: bool,     // terminal — no more actions
}

// In handlers:
require!(!state.is_finished, MyError::AlreadyFinished);
require!(state.is_revealed, MyError::NotRevealed);
```

---

## 3. Account Validation

### Pattern A: PDA Init with Seeds

```rust
#[account(
    init,
    payer = authority,
    space = 8 + MyAccount::INIT_SPACE,
    seeds = [b"seed", authority.key().as_ref()],
    bump,
)]
pub my_account: Account<'info, MyAccount>,
```

- Seeds bind the account to the authority — only that authority can create it.
- `bump` stores the bump for later validation.

### Pattern B: Mutable with Close (Rent Recovery)

```rust
#[account(
    mut,
    close = authority,
    seeds = [b"seed", authority.key().as_ref()],
    bump,
)]
pub my_account: Account<'info, MyAccount>,
```

- `close = authority` returns rent lamports AND zeroes the account data.
- PDA seeds enforce that only the correct authority can close.

### Pattern C: UncheckedAccount for External Programs

```rust
/// CHECK: Validated manually in handler — reads discriminator + data
pub external_account: UncheckedAccount<'info>,
```

Then validate in the handler:

```rust
let data = ctx.accounts.external_account.try_borrow_data()?;
let discriminator = ExternalType::discriminator();
require!(
    data.len() >= discriminator.len() && data[..discriminator.len()] == *discriminator,
    MyError::InvalidExternalAccount
);
```

### Pattern D: Signer + Authority Check

```rust
#[account(
    mut,
    has_one = admin,           // Anchor checks admin field == signer
    seeds = [b"seed", admin.key().as_ref()],
    bump = my_account.bump,    // Uses stored bump
)]
pub my_account: Account<'info, MyAccount>,
#[account(mut)]
pub admin: Signer<'info>,
```

- `has_one = admin` ensures the stored admin matches the signer.
- `bump = my_account.bump` uses the stored bump for validation.

---

## 4. Error Handling

### Error Enum

```rust
// error.rs
#[error_code]
pub enum MyError {
    #[msg("Only the admin can perform this action")]
    Unauthorized,
    #[msg("Value must be between {MIN} and {MAX}")]
    OutOfRange,
    #[msg("Action not allowed in current state")]
    InvalidState,
}
```

### Three Propagation Patterns

**`require!` — boolean guard (most common):**

```rust
require!(value >= MIN && value <= MAX, MyError::OutOfRange);
require!(!account.is_finished, MyError::InvalidState);
```

**`err!` — early return from helpers:**

```rust
if data.len() < MIN_SIZE {
    return err!(MyError::InvalidData);
}
```

**`error!` + `.ok_or()` — Option → Result:**

```rust
let value = data.get(offset..offset + 8)
    .ok_or(error!(MyError::InvalidData))?;
```

---

## 5. Testing with LiteSVM

### Setup Function (copy this exactly)

```rust
// tests/test_program.rs
use litesvm::LiteSVM;
use solana_message::Message;
use solana_transaction::versioned::{VersionedTransaction, VersionedMessage};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_instruction::Instruction;

fn setup_svm() -> (LiteSVM, Keypair, Pubkey) {
    let program_id = my_program::id();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();

    // Load compiled BPF program — runs REAL bytecode, not simulation
    let bytes = include_bytes!("../../target/deploy/my_program.so");
    svm.add_program(program_id, bytes).unwrap();

    // Fund admin with 2 SOL
    svm.airdrop(&admin.pubkey(), 2_000_000_000).unwrap();

    (svm, admin, program_id)
}
```

**Gotcha:** `include_bytes!` path is relative to the test file. If tests are in `tests/`, the path is `../../target/deploy/<name>.so`.

### Transaction Helper

```rust
fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
) -> Result<(), litesvm::types::FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    let result = svm.send_transaction(tx);
    svm.expire_blockhash(); // Prevent replay — always call this
    result.map(|_| ())
}
```

**Rule:** Always call `svm.expire_blockhash()` after every transaction.

### Instruction Builder

```rust
fn build_initialize_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    account_pda: &Pubkey,
    value: u8,
) -> Instruction {
    let system_program = Pubkey::from([0u8; 32]); // System program in SVM
    Instruction::new_with_bytes(
        *program_id,
        &my_program::instruction::Initialize { value }.data(),
        my_program::accounts::Initialize {
            my_account: *account_pda,
            admin: *admin,
            system_program,
        }
        .to_account_metas(None),
    )
}
```

- Uses `InstructionData` and `ToAccountMetas` from the program's own crate.
- No IDL, no TypeScript — pure Rust.

### Account Reader

```rust
fn read_account<T: anchor_lang::AccountDeserialize>(svm: &LiteSVM, pubkey: &Pubkey) -> T {
    let data = svm.get_account(pubkey).unwrap().data;
    T::try_deserialize(&mut &data[..]).unwrap()
}
```

### PDA Derivation

```rust
fn get_account_pda(admin: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let seeds = &[b"my_account", admin.as_ref()];
    Pubkey::find_program_address(seeds, program_id).0
}
```

### Test Structure — Setup/Act/Assert

```rust
#[test]
fn test_initialize() {
    eprintln!("\n━━━ test_initialize ━━━");

    // Step 1: Setup
    let (mut svm, admin, program_id) = setup_svm();
    eprintln!("  Step 1: Setup LiteSVM + admin funded");

    // Step 2: Derive PDA
    let pda = get_account_pda(&admin.pubkey(), &program_id);
    eprintln!("  Step 2: PDA = {pda}");

    // Step 3: Act
    let ix = build_initialize_ix(&program_id, &admin.pubkey(), &pda, 42);
    let res = send_ix(&mut svm, ix, &admin);
    assert!(res.is_ok());
    eprintln!("  Step 3: initialize(value=42) → OK");

    // Step 4: Assert
    let account: MyAccount = read_account(&svm, &pda);
    assert_eq!(account.admin, admin.pubkey());
    assert_eq!(account.value, 42);
    eprintln!("  ✓ test_initialize passed");
}
```

### Testing Failure Cases

```rust
#[test]
fn test_unauthorized_action() {
    let (mut svm, admin, program_id) = setup_svm();
    let impostor = Keypair::new();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();

    // Setup state with admin
    let pda = get_account_pda(&admin.pubkey(), &program_id);
    let ix = build_initialize_ix(&program_id, &admin.pubkey(), &pda, 42);
    send_ix(&mut svm, ix, &admin).unwrap();

    // Try action with impostor — should fail
    let ix = build_action_ix(&program_id, &impostor.pubkey(), &pda, 10);
    let res = send_ix(&mut svm, ix, &impostor);
    assert!(res.is_err());
}
```

### Composite Setup Helper

For tests that need a fully initialized state:

```rust
fn setup_full_game(value: u8) -> (LiteSVM, Keypair, Pubkey, Pubkey) {
    let (mut svm, admin, program_id) = setup_svm();
    let pda = get_account_pda(&admin.pubkey(), &program_id);

    // Initialize
    let ix = build_initialize_ix(&program_id, &admin.pubkey(), &pda, value);
    send_ix(&mut svm, ix, &admin).unwrap();

    // Any additional setup steps...

    (svm, admin, program_id, pda)
}
```

### Slot Warping (Time-Dependent Logic)

```rust
svm.warp_to_slot(200); // Sets Clock::get().slot to 200
```

Use this for testing time-locks, reveal slots, epoch-dependent logic.

### Boundary Value Testing

```rust
#[test]
fn test_boundary_values() {
    for byte in [0u8, 1, 99, 100, 150, 200, 255] {
        let (mut svm, admin, program_id, pda) = setup_full_game(byte);
        let expected = (byte % 100) + 1;
        let account: MyAccount = read_account(&svm, &pda);
        assert_eq!(account.value, expected);
        assert!((1..=100).contains(&account.value));
    }
}
```

---

## 6. External Oracles (VRF)

### Mocking External Accounts in LiteSVM

When integrating Switchboard, Pyth, or any oracle, mock the account data:

```rust
const ORACLE_DISCRIMINATOR: [u8; 8] = [0x12, 0x34, ...]; // From the oracle SDK

fn create_oracle_account(svm: &mut LiteSVM, pubkey: &Pubkey, value: u8) {
    let oracle_program_id: Pubkey = "OracleProgramID1111111111111111111".parse().unwrap();
    let mut data = vec![0u8; 400]; // Expected account size

    // Write discriminator
    data[..8].copy_from_slice(&ORACLE_DISCRIMINATOR);

    // Write value at the correct offset
    let value_offset = 144; // From the oracle's struct layout
    data[value_offset] = value;

    let account = solana_account::Account {
        lamports: 1_000_000,  // MUST be > 0 — LiteSVM drops zero-lamport accounts
        data,
        owner: oracle_program_id,  // MUST be owned by the oracle program
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*pubkey, account).unwrap();
}
```

**Key gotchas:**
- `lamports` must be > 0 (LiteSVM silently removes zero-lamport accounts)
- `owner` must be the oracle's program ID
- Discriminator must match the oracle's account type

### Manual Byte Extraction (Avoid bytemuck Issues)

When reading external account data on-chain, use manual byte extraction instead of `bytemuck::Pod` casting:

```rust
const VALUE_OFFSET: usize = 8 + 144; // 8 for discriminator + 144 for field offset

fn read_u64_at(data: &[u8], offset: usize) -> Option<u64> {
    if data.len() < offset + 8 {
        return None;
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

// In handler:
let data = ctx.accounts.oracle_account.try_borrow_data()?;
let discriminator = OracleType::discriminator();
require!(
    data.len() >= discriminator.len() && data[..discriminator.len()] == *discriminator,
    MyError::InvalidOracleAccount
);
let value = read_u64_at(&data, VALUE_OFFSET)
    .ok_or(error!(MyError::InvalidOracleAccount))?;
```

This avoids alignment panics that `bytemuck` can cause in BPF.

---

## 7. Build, Test, Deploy

### Build

```bash
anchor build           # Compile all programs → target/deploy/*.so
cargo build-sbf        # Direct BPF compilation (alternative)
```

### Test Runner (`test.sh`)

```bash
#!/usr/bin/env bash
set -euo pipefail

run_tests() {
    local label="$1"
    local manifest="$2"
    echo "━━━ $label ━━━"
    cargo test --manifest-path "$manifest" --quiet -- --nocapture
}

case "${1:-all}" in
    all)
        run_tests "Program A" "programs/program-a/Cargo.toml"
        run_tests "Program B" "programs/program-b/Cargo.toml"
        ;;
    a) run_tests "Program A" "programs/program-a/Cargo.toml" ;;
    b) run_tests "Program B" "programs/program-b/Cargo.toml" ;;
esac
```

Usage: `bash test.sh [all|a|b]`

### Deploy to Devnet

```bash
solana config set --url devnet
solana airdrop 2                          # Get SOL for deployment
anchor build                              # Build first
anchor deploy                             # Deploy all programs

# Or deploy a specific program manually:
solana program deploy target/deploy/my_program.so \
    --program-id target/deploy/my_program-keypair.json
```

### Feature-Gated Binary

For optional CLI tools that need extra deps:

```toml
[features]
play = ["litesvm", "solana-message", "solana-transaction"]

[[bin]]
name = "play"
path = "src/bin/play.rs"
required-features = ["play"]
```

Run: `cargo run --features play`

---

## 8. Quick-Start Checklist

Starting a new Anchor project? Follow this order:

- [ ] `anchor init my-project && cd my-project`
- [ ] Replace `programs/*` workspace layout with the structure above
- [ ] `declare_id!` with your program ID in `lib.rs`
- [ ] Register program ID in `Anchor.toml`
- [ ] Create `state.rs`, `error.rs`, `constants.rs`, `instructions.rs`
- [ ] Create one file per instruction in `instructions/`
- [ ] Keep `lib.rs` as a thin router — handlers live in instruction files
- [ ] Add `crate-type = ["cdylib", "lib"]` to program's `Cargo.toml`
- [ ] Add hardened `[profile.release]` to workspace `Cargo.toml`
- [ ] Add LiteSVM dev-dependencies to program's `Cargo.toml`
- [ ] Create `tests/` with `setup_svm()`, `send_ix()`, builder functions
- [ ] Use `include_bytes!("../../target/deploy/my_program.so")` to load BPF
- [ ] Always call `svm.expire_blockhash()` after transactions
- [ ] Write positive tests (success), negative tests (unauthorized, invalid state), and boundary tests
- [ ] Create `test.sh` runner
- [ ] Build → Test → Deploy

### Cost Expectations (Devnet Reference)

| Instruction Type | Fee | Compute Units |
|-----------------|----:|--------------:|
| Simple (init account) | 5,000 lamports | ~3,000-6,000 |
| With oracle integration | 5,000-10,000 lamports | ~50,000-120,000 |
| Per-action (guess, update) | 5,000 lamports | ~3,000 |
| Close (rent recovery) | 5,000 lamports | ~5,000 |

---

## Common Gotchas

1. **Zero-lamport accounts disappear** in LiteSVM. Always set `lamports > 0` when mocking.
2. **`svm.expire_blockhash()`** must be called after every `send_transaction` or sequential txs will fail.
3. **`bytemuck` alignment** — use manual byte extraction for external account data instead of `Pod` casting.
4. **`getrandom` crate** — must use `features = ["custom"]` or BPF compilation fails.
5. **Account space** — use `8 + T::INIT_SPACE` (8 for discriminator). Wrong size = deserialization errors.
6. **Feature-gated dev-deps** — put heavy test deps behind a feature flag to keep `cargo build-sbf` fast.
7. **PDA seeds are part of security** — `seeds = [b"prefix", authority.as_ref()]` ensures only the right authority can create/access the account.
