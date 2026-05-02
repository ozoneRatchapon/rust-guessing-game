# 001: Broken `rand` Demo — Proof That Randomness Fails On-Chain

## Status: Complete

## What

A standalone minimal Anchor program that uses the `rand` crate to demonstrate that it **cannot compile** for the Solana BPF target.

## Why

The teaching guide (`docs/on-chain-randomness-lesson.md`) explains *why* `rand` doesn't work on Solana. But explanation is not proof. Watching `cargo build-sbf` fail with `compile_error!("target is not supported")` is visceral and memorable.

## Plan

Create `on-chain/demos/broken-rand/` as a standalone workspace (not in Anchor workspace)
- Add `rand` as a dependency
- Use `rand::thread_rng().gen_range(1..=100)` in a single instruction
- Run `cargo build-sbf` → watch it fail
- Document the error output in `docs/bonus-broken-rand.md`
- Optionally: create a script that runs the build and captures the error

## Acceptance Criteria

- [x] `broken-rand` program compiles for host target but fails for BPF
- [x] Error message is captured and documented
- [x] Doc links back to the teaching guide's explanation

### Demo Script

A TypeScript runner `scripts/build-broken-rand.ts` that:
Runs `cargo build-sbf --manifest-path on-chain/demos/broken-rand/Cargo.toml`
2. Captures stderr
3. Displays the error with highlighted key lines
4. Prints a summary: "This is why `rand` cannot be used on Solana"
5. Links to `docs/on-chain-randomness-lesson.md` for the full explanation

Can also be run via `./scripts/demo.sh 4`.

## Result

- Program lives in `on-chain/demos/broken-rand/` (standalone workspace, not in Anchor workspace)
- Host compilation: PASSES (`cargo check`)
- BPF compilation: FAILS with `getrandom` error: "target is not supported"
- Demo script `scripts/build-broken-rand.ts` shows both steps with color output
- Available via `bash scripts/demo.sh 4`

## Refs

- `docs/on-chain-randomness-lesson.md` — Section on why `rand` breaks consensus
- Phase 1 commit-reveal — the working alternative
- Phase 2 VRF — the production solution
