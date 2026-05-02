# 004: Project Closeout + Patterns Guide

## Status: Complete

## What Happened

Final session to close out the Rust Guessing Game project.

### 1. Fixed Diagnostics

Two warnings in `play-phase2-devnet.ts`:
- Removed unused `import * as anchor`
- Removed unused `sendAndLog()` function

### 2. Created Patterns Guide

Created `docs/solana-anchor-patterns.md` — a 727-line portable guide covering:

| Section | What It Covers |
|---------|---------------|
| Project Structure | Anchor workspace layout, Cargo.toml patterns |
| Program Architecture | One-file-per-instruction, thin lib.rs router, state machines |
| Account Validation | PDA init, close/recovery, UncheckedAccount, has_one |
| Error Handling | require!, err!, error! + .ok_or() patterns |
| Testing with LiteSVM | setup_svm(), send_ix(), builders, slot warping, boundary tests |
| External Oracles | Mocking accounts, manual byte extraction, bytemuck avoidance |
| Build/Test/Deploy | test.sh runner, feature-gated binaries, deploy commands |
| Quick-Start Checklist | Step-by-step for new projects |

This guide is designed to be given to an LLM as context when starting a new Anchor project. It replaces "distill from this repo" with a curated, battle-tested reference.

### 3. Verified All Tests

All 24 tests green (8 Phase 1 + 16 Phase 2).

## Where Is the Code

| File | Action | Description |
|------|--------|-------------|
| `docs/solana-anchor-patterns.md` | **New** | Portable patterns guide (727 lines) |
| `on-chain/scripts/play-phase2-devnet.ts` | **Modified** | Removed 2 unused declarations |

## Git Log

```
bca873f (main) fix: remove unused import and sendAndLog function in play-phase2-devnet.ts
```

## How to Use the Patterns Guide

### Option A: Give it to an LLM directly
```
@docs/solana-anchor-patterns.md
```
Attach or paste when starting a new Anchor project conversation.

### Option B: Distill from this repo
Point the LLM at https://github.com/ozoneRatchapon/rust-guessing-game as a reference repo. The patterns guide is a more focused alternative — the LLM won't need to wade through 30+ files.

**Recommendation:** Use the patterns guide. It's curated, battle-tested, and includes gotchas that "distill from repo" might miss.

## Project Summary

| Metric | Value |
|--------|-------|
| Programs | 2 (Phase 1 commit-reveal, Phase 2 Switchboard VRF) |
| Tests | 24 (all passing) |
| Devnet programs | 2 live |
| Demos | 4 working |
| Patterns guide | 727 lines |
| Commits (main) | 15 |

## Branch Structure

```
main (bca873f) ← pushed to origin
develop ← in sync with main
tag: phase1-demo ← Phase 1 snapshot
```
