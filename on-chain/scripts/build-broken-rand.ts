/**
 * Broken `rand` Demo — Proof That Randomness Fails On-Chain
 *
 * Runs `cargo build-sbf` on the broken-rand program.
 * The build fails because `rand` -> `getrandom` has no entropy source in the BPF VM.
 *
 * Run:  npx tsx scripts/build-broken-rand.ts
 */

import { execSync } from "child_process";
import * as path from "path";

// Copy the color constants from play-devnet.ts
// Use the same C object, banner, ok, warn, err, dim functions

const C = {
  reset: "\x1b[0m",
  bold: "\x1b[1m",
  dim: "\x1b[2m",
  underline: "\x1b[4m",
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  magenta: "\x1b[35m",
  cyan: "\x1b[36m",
  white: "\x1b[37m",
  bgGreen: "\x1b[42m",
  bgRed: "\x1b[41m",
};

const banner = (s: string) => `${C.cyan}${s}${C.reset}`;
const ok = (s: string) => `${C.green}✓${C.reset} ${s}`;
const warn = (s: string) => `${C.yellow}⚠${C.reset} ${s}`;
const err = (s: string) => `${C.red}✗${C.reset} ${s}`;
const dim = (s: string) => `${C.dim}${s}${C.reset}`;

function main() {
  console.log();
  console.log(banner("============================================"));
  console.log(banner("  Broken `rand` Demo"));
  console.log(banner("  Proof That Randomness Fails On-Chain"));
  console.log(banner("============================================"));
  console.log();

  console.log(dim("This demo proves that the `rand` crate cannot compile"));
  console.log(dim("for the Solana BPF target (sBPF VM)."));
  console.log();
  console.log("  Why? Solana programs run in a sandboxed VM with:");
  console.log(dim("    - No OS access (no /dev/urandom, no syscalls)"));
  console.log(dim("    - No entropy source for cryptographic randomness"));
  console.log(dim("    - Deterministic execution required for consensus"));
  console.log();

  // Step 1: Show that host compilation works
  console.log(banner("━━━ STEP 1: Host Compilation (should pass) ━━━"));
  console.log();

  const manifestPath = path.join(
    __dirname,
    "..",
    "demos",
    "broken-rand",
    "Cargo.toml"
  );

  try {
    execSync(`cargo check --manifest-path "${manifestPath}" 2>&1`, {
      stdio: "pipe",
      encoding: "utf-8",
    });
    console.log(ok("Host compilation: PASSED"));
    console.log(dim("  The `rand` crate works fine on your laptop."));
  } catch (e: any) {
    console.log(err("Host compilation failed unexpectedly:"));
    console.log(dim(e.stdout || e.message));
    process.exit(1);
  }

  console.log();

  // Step 2: Show that BPF compilation fails
  console.log(banner("━━━ STEP 2: BPF Compilation (should FAIL) ━━━"));
  console.log();

  try {
    execSync(`cargo build-sbf --manifest-path "${manifestPath}" 2>&1`, {
      stdio: "pipe",
      encoding: "utf-8",
    });
    // If we get here, something is wrong — it should have failed
    console.log(err("BPF compilation PASSED — this should NOT happen!"));
    console.log(
      warn("The `rand` crate somehow compiled for BPF. The demo is broken.")
    );
    process.exit(1);
  } catch (e: any) {
    const output = e.stdout || e.stderr || e.message || "";

    console.log(err("BPF compilation: FAILED (as expected!)"));
    console.log();

    // Highlight the key error lines
    const lines = output.split("\n");
    const errorLines = lines.filter(
      (l: string) =>
        l.includes("error:") ||
        l.includes("compile_error!") ||
        l.includes("target is not supported") ||
        l.includes("getrandom") ||
        l.includes("unresolved")
    );

    if (errorLines.length > 0) {
      console.log(`  ${C.bold}Key error messages:${C.reset}`);
      for (const line of errorLines.slice(0, 8)) {
        const highlighted = line.includes("target is not supported")
          ? `${C.red}${C.bold}${line.trim()}${C.reset}`
          : `${C.red}${line.trim()}${C.reset}`;
        console.log(`    ${highlighted}`);
      }
    } else {
      console.log(dim(output.slice(-500)));
    }
  }

  console.log();

  // Step 3: Summary
  console.log(banner("━━━ CONCLUSION ━━━"));
  console.log();
  console.log(
    `  ${C.bgRed}${C.white}${C.bold} PROOF ${C.reset}  The \`rand\` crate ${C.bold}cannot${C.reset} be used on Solana.`
  );
  console.log();
  console.log("  Root cause:");
  console.log(
    dim("    `rand` → `getrandom` → requires OS entropy → no OS in BPF VM")
  );
  console.log();
  console.log("  Solutions:");
  console.log(
    ok("Phase 1: Commit-Reveal (admin picks secret, proves with hash)")
  );
  console.log(
    ok("Phase 2: Switchboard VRF (oracle generates randomness off-chain)")
  );
  console.log();
  console.log(dim("  Learn more: docs/on-chain-randomness-lesson.md"));
  console.log();
}

main();
