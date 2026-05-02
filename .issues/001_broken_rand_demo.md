# 001: Broken `rand` Demo — Proof That Randomness Fails On-Chain

## Status: Planned (after Phase 2)

## What

A standalone minimal Anchor program that uses the `rand` crate to demonstrate that it **cannot compile** for the Solana BPF target.

## Why

The teaching guide (`docs/on-chain-randomness-lesson.md`) explains *why* `rand` doesn't work on Solana. But explanation is not proof. Watching `cargo build-sbf` fail with `compile_error!("target is not supported")` is visceral and memorable.

## Plan

- Create `on-chain/programs/broken-rand/` as a separate Anchor program
- Add `rand` as a dependency
- Use `rand::thread_rng().gen_range(1..=100)` in a single instruction
- Run `cargo build-sbf` → watch it fail
- Document the error output in `docs/bonus-broken-rand.md`
- Optionally: create a script that runs the build and captures the error

## Acceptance Criteria

- [ ] `broken-rand` program compiles for host target but fails for BPF
- [ ] Error message is captured and documented
- [ ] Doc links back to the teaching guide's explanation

## Refs

- `docs/on-chain-randomness-lesson.md` — Section on why `rand` breaks consensus
- Phase 1 commit-reveal — the working alternative
- Phase 2 VRF — the production solution
