# 001: Phase 2 — Switchboard VRF Upgrade

## Status: Planning

## What Happened

Phase 1 commit-reveal guessing game is complete and deployed on devnet:
- Program: `3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT`
- 4 instructions: initialize, reveal, guess, close_game
- Devnet script with Explorer links
- 8 LiteSVM tests
- Docs: architecture, cost analysis, security model

## Phase 2 Plan

### Goal

Replace admin-chosen secret with Switchboard VRF for trustless randomness. Nobody — including the admin — knows the number in advance.

### Architecture Change

**Phase 1 flow (trust-on-admin):**
```
initialize(secret_number) → reveal(secret_number) → guess(guess)
```

**Phase 2 flow (trustless VRF):**
```
initialize → [wait for oracle] → settle_random → guess(guess)
```

### Switchboard Commit-Reveal Pattern

```
1. COMMIT    →    2. GENERATE    →    3. REVEAL
   Player            Oracle             Settlement
   commits to        generates          Player reveals
   slothash          randomness         and uses value
```

- Commit happens in the same tx as our `initialize`
- Oracle generates randomness off-chain (~3 seconds)
- Reveal happens in the same tx as our `settle_random`

### Dependencies

| Package | Version | Notes |
|---------|---------|-------|
| `switchboard-on-demand` | `0.12.1` | `features = ["anchor"]`, compatible with anchor-lang 1.0.1 |
| `@switchboard-xyz/on-demand` | `^3.9.0` | TypeScript SDK for devnet script |

### Program Changes

**New instruction: `initialize`**
- No longer takes `secret_number` parameter
- Creates Switchboard randomness account
- Commits to randomness (slothash)
- Stores randomness account pubkey in Game struct

**New instruction: `settle_random`**
- Reveals randomness from Switchboard oracle
- Derives secret number: `random_bytes[0] % 100 + 1`
- Stores secret in game, sets `is_revealed = true`

**Existing instruction: `guess`**
- No changes (same comparison logic)

**Existing instruction: `close_game`**
- No changes (closes game account)

**Removed instruction: `reveal`**
- No longer needed — VRF replaces manual reveal

### Game Account Changes

| Field | Phase 1 | Phase 2 |
|-------|---------|---------|
| admin | Pubkey | Pubkey (unchanged) |
| secret_hash | [u8; 32] | [u8; 32] (now stores VRF commitment) |
| secret_number | u8 | u8 (unchanged) |
| is_revealed | bool | bool (unchanged) |
| attempts | u8 | u8 (unchanged) |
| max_tries | u8 | u8 (unchanged) |
| is_finished | bool | bool (unchanged) |
| bump | u8 | u8 (unchanged) |
| randomness_account | — | Pubkey (NEW: Switchboard randomness PDA) |
| commit_slot | — | u64 (NEW: slot when randomness was committed) |

### Security Checks (from teaching guide)

1. **Slot freshness**: `randomness_data.seed_slot == clock.slot - 1`
2. **Randomness not already revealed**: at commit time
3. **Randomness account reference**: verify at settle time matches stored pubkey
4. **Commit slot matches**: at settle time

### Implementation Steps

1. Add `switchboard-on-demand` dependency with `anchor` feature
2. Update `Game` struct with new fields
3. Rewrite `initialize` (no secret param, commit randomness)
4. Add `settle_random` instruction
5. Remove `reveal` instruction
6. Update `guess` (no changes needed)
7. Write LiteSVM tests
8. Update devnet play script (3-step: init, wait, settle, then guess)
9. Deploy to devnet
10. Update docs

### TypeScript Client Changes

```typescript
// Phase 2 flow
const [randomness, createIx] = await sb.Randomness.create(sbProgram, rngKp, queue);
const commitIx = await randomness.commitIx(queue);
const initIx = program.methods.initialize().accounts({...}).instruction();
// Send: [createIx, commitIx, initIx] in one tx

// Wait ~3 seconds for oracle
await sleep(3000);

// Settle
const revealIx = await randomness.revealIx();
const settleIx = program.methods.settleRandom().accounts({...}).instruction();
// Send: [revealIx, settleIx] in one tx

// Then guess loop (same as Phase 1)
```

### Open Questions

- **DECIDED: Phase 2 is a separate program (`phase2-vrf`).** Phase 1 (`on-chain`) stays deployed at its own Program ID. Both coexist on devnet. Reasons: (1) Phase 1 stays demo-able forever, (2) different instructions need different IDLs, (3) side-by-side comparison is better for teaching.
- Can LiteSVM tests work with Switchboard? 
  - Likely need to mock the randomness account data since Switchboard oracle won't run in LiteSVM
  - Alternative: test with `solana-test-validator` instead

### Remain Work

- [ ] Create `on-chain/programs/phase2-vrf/` as new Anchor program
- [ ] Add `switchboard-on-demand` dependency with `anchor` feature
- [ ] Implement `GameV2` account struct (add randomness_account, commit_slot)
- [ ] Implement `initialize` (no secret param, commit randomness)
- [ ] Implement `settle_random` (reveal VRF, derive secret)
- [ ] Implement `guess` (same logic as Phase 1)
- [ ] Implement `close_game` (same logic as Phase 1)
- [ ] Write LiteSVM tests with mocked randomness
- [ ] Create `scripts/play-phase2-devnet.ts` with Switchboard SDK
- [ ] Create `scripts/demo.sh` launcher for all 4 demos
- [ ] Create `on-chain/programs/broken-rand/` for broken rand demo
- [ ] Create `scripts/build-broken-rand.ts`
- [ ] Deploy Phase 2 to devnet (new Program ID)
- [ ] Update all docs

### Issues Ref

- `.issues/001_broken_rand_demo.md` — planned bonus after Phase 2

### How to Dev/Test

```bash
# Build Phase 2
cd on-chain && anchor build --skip-lint

# Test Phase 2 (LiteSVM with mocked randomness)
cargo test --manifest-path on-chain/programs/phase2-vrf/Cargo.toml

# Deploy Phase 2 (separate Program ID from Phase 1)
solana program deploy --url devnet target/deploy/phase2_vrf.so

# Play Phase 2
npx tsx scripts/play-phase2-devnet.ts
```
