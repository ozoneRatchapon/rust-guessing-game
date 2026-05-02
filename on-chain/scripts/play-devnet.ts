/**
 * Interactive Solana Guessing Game on Devnet
 *
 * Plays the on-chain commit-reveal guessing game:
 *   1. Initialize  – admin commits blake3(secret) on-chain
 *   2. Reveal      – admin reveals secret (program verifies hash)
 *   3. Guess loop  – player guesses (1-100), up to 10 attempts
 *
 * Run:  npx tsx scripts/play-devnet.ts
 */

import * as anchor from "@anchor-lang/core";
import { Program, AnchorProvider, Wallet } from "@anchor-lang/core";
import {
  Connection,
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as readline from "readline/promises";

// Anchor Program with typed account/method namespaces for our IDL
interface OnChainProgram extends Program {
  account: {
    game: {
      fetch(address: PublicKey): Promise<GameAccount>;
    };
  } & Program["account"];
  methods: {
    initialize(secretNumber: number): any;
    reveal(secretNumber: number): any;
    guess(guessNumber: number): any;
  } & Program["methods"];
}

// ─── Constants ────────────────────────────────────────────────────────────────

const DEVNET_URL = process.argv.includes("--local")
  ? "http://localhost:8899"
  : "https://api.devnet.solana.com";
const EXPLORER_TX = "https://explorer.solana.com/tx";
const CLUSTER = process.argv.includes("--local")
  ? "custom&customUrl=http://localhost:8899"
  : "devnet";
const PROGRAM_ID = new PublicKey(
  "3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT"
);
const KEYPAIR_PATH = path.join(
  process.env.HOME!,
  ".config",
  "solana",
  "id.json"
);

// ─── ANSI helpers ─────────────────────────────────────────────────────────────

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

const banner = (text: string) => `${C.bold}${C.cyan}${text}${C.reset}`;
const ok = (text: string) => `${C.green}${text}${C.reset}`;
const warn = (text: string) => `${C.yellow}${text}${C.reset}`;
const err = (text: string) => `${C.red}${text}${C.reset}`;
const dim = (text: string) => `${C.dim}${text}${C.reset}`;
const link = (url: string) => `${C.underline}${C.blue}${url}${C.reset}`;

// ─── IDL type (from generated JSON) ───────────────────────────────────────────

interface GameAccount {
  admin: PublicKey;
  secretHash: Uint8Array | number[];
  secretNumber: number;
  isRevealed: boolean;
  attempts: number;
  maxTries: number;
  isFinished: boolean;
  bump: number;
}

type OnChainIdl = {
  address: string;
  instructions: any[];
  accounts: any[];
  events: any[];
  errors: any[];
  types: any[];
  metadata: {
    name: string;
    version: string;
    spec: string;
    description: string;
  };
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

function explorerUrl(signature: string): string {
  return `${EXPLORER_TX}/${signature}?cluster=${CLUSTER}`;
}

function formatGame(state: GameAccount): string {
  const hashArr = Array.from(state.secretHash);
  return [
    `    admin:         ${state.admin.toBase58()}`,
    `    secret_hash:   [${hashArr.slice(0, 4).join(", ")}...${hashArr
      .slice(-2)
      .join(", ")}]`,
    `    secret_number: ${state.isRevealed ? state.secretNumber : "???"}`,
    `    is_revealed:   ${state.isRevealed}`,
    `    attempts:      ${state.attempts}`,
    `    max_tries:     ${state.maxTries}`,
    `    is_finished:   ${state.isFinished}`,
    `    bump:          ${state.bump}`,
  ].join("\n");
}

function loadKeypair(): Keypair {
  const raw = JSON.parse(fs.readFileSync(KEYPAIR_PATH, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

async function askNumber(
  rl: readline.Interface,
  prompt: string,
  min: number,
  max: number
): Promise<number> {
  while (true) {
    const answer = await rl.question(prompt);
    const n = parseInt(answer.trim(), 10);
    if (isNaN(n) || n < min || n > max) {
      console.log(warn(`  Please enter a number between ${min} and ${max}.`));
      continue;
    }
    return n;
  }
}

async function askYesNo(
  rl: readline.Interface,
  prompt: string
): Promise<boolean> {
  const answer = await rl.question(prompt);
  return (
    answer.trim().toLowerCase() === "y" || answer.trim().toLowerCase() === "yes"
  );
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log();
  console.log(banner("============================================"));
  console.log(banner("  Solana Guessing Game — Devnet"));
  console.log(banner("  Commit-Reveal Edition"));
  console.log(banner("============================================"));
  console.log();
  console.log(dim("How it works:"));
  console.log(dim("  1. ADMIN commits blake3(secret) on-chain"));
  console.log(dim("  2. ADMIN reveals the secret (program verifies hash)"));
  console.log(dim("  3. YOU guess the number (1-100)"));
  console.log(dim("  4. You have 10 attempts"));
  console.log();

  // ── Load keypair ──────────────────────────────────────────────────────────

  const keypair = loadKeypair();
  const adminPk = keypair.publicKey;
  console.log(`${C.bold}[SETUP]${C.reset}  Keypair: ${adminPk.toBase58()}`);

  // ── Connect ───────────────────────────────────────────────────────────────

  const connection = new Connection(DEVNET_URL, "confirmed");
  console.log(`${C.bold}[SETUP]${C.reset}  Network:  ${DEVNET_URL}`);

  const balance = await connection.getBalance(adminPk);
  console.log(
    `${C.bold}[SETUP]${C.reset}  Balance:  ${(
      balance / LAMPORTS_PER_SOL
    ).toFixed(4)} SOL`
  );

  if (balance < 0.05 * LAMPORTS_PER_SOL) {
    console.log(warn("  Balance low! Requesting airdrop..."));
    const sig = await connection.requestAirdrop(adminPk, 1 * LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig, "confirmed");
    console.log(ok("  Airdrop confirmed! +1 SOL"));
  }
  console.log();

  // ── Provider & Program ────────────────────────────────────────────────────

  const wallet = new Wallet(keypair);
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });

  const idlPath = path.join(__dirname, "..", "target", "idl", "on_chain.json");
  const idl: OnChainIdl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(
    idl as any,
    provider
  ) as unknown as OnChainProgram;

  console.log(`${C.bold}[SETUP]${C.reset}  Program:  ${PROGRAM_ID.toBase58()}`);
  console.log(
    `${C.bold}[SETUP]${C.reset}  IDL:      ${idl.metadata.name} v${idl.metadata.version}`
  );
  console.log();

  // ── PDA ───────────────────────────────────────────────────────────────────

  const [gamePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("game"), adminPk.toBytes()],
    PROGRAM_ID
  );
  console.log(`${C.bold}[SETUP]${C.reset}  Game PDA: ${gamePda.toBase58()}`);
  console.log();

  // ── Check existing game ───────────────────────────────────────────────────

  let existingGame: GameAccount | null = null;
  try {
    existingGame = (await program.account.game.fetch(
      gamePda
    )) as unknown as GameAccount;
  } catch {
    // No game account exists yet
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 1: INITIALIZE
  // ──────────────────────────────────────────────────────────────────────────

  let secretNumber: number;

  if (existingGame) {
    console.log(warn("[INITIALIZE]  Game account already exists at this PDA."));
    console.log(dim(formatGame(existingGame)));

    const reinit = await askYesNo(rl, "  Re-initialize (overwrite)? [y/N]: ");
    if (!reinit) {
      console.log(dim("  Skipping initialize. Using existing game."));
      secretNumber = 0; // Will be set in reveal step
    } else {
      // Close existing game account first
      console.log(dim("  Closing existing game account..."));
      await doCloseGame(program, keypair, gamePda, adminPk);

      secretNumber = await askNumber(
        rl,
        "  Enter secret number (1-100): ",
        1,
        100
      );
      await doInitialize(program, keypair, gamePda, adminPk, secretNumber);
    }
  } else {
    const pickRandom = await askYesNo(
      rl,
      "  Pick a random secret number? [Y/n]: "
    );
    if (pickRandom || pickRandom === undefined) {
      secretNumber = Math.floor(Math.random() * 100) + 1;
      console.log(ok(`  Random secret: ${secretNumber}`));
    } else {
      secretNumber = await askNumber(
        rl,
        "  Enter secret number (1-100): ",
        1,
        100
      );
    }
    await doInitialize(program, keypair, gamePda, adminPk, secretNumber);
  }

  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 2: REVEAL
  // ──────────────────────────────────────────────────────────────────────────

  // Re-read state
  let gameState = (await program.account.game.fetch(
    gamePda
  )) as unknown as GameAccount;

  if (!gameState.isRevealed) {
    // If we skipped initialize, we need to ask for the secret
    if (secretNumber === 0) {
      secretNumber = await askNumber(
        rl,
        "  Enter the admin's secret number (1-100): ",
        1,
        100
      );
    }
    await doReveal(program, keypair, gamePda, adminPk, secretNumber);
    gameState = (await program.account.game.fetch(
      gamePda
    )) as unknown as GameAccount;
  } else {
    console.log(dim("[REVEAL]  Secret already revealed. Skipping."));
  }

  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 3: GUESS LOOP
  // ──────────────────────────────────────────────────────────────────────────

  if (gameState.isFinished) {
    console.log(warn("Game is already finished!"));
    console.log(formatGame(gameState));
    rl.close();
    return;
  }

  console.log(banner("━━━ GUESSING PHASE ━━━"));
  console.log();
  console.log(
    `  Guess the number between ${C.bold}1${C.reset} and ${C.bold}100${C.reset}.`
  );
  console.log(
    `  You have ${C.bold}${gameState.maxTries}${C.reset} attempts. Good luck!`
  );
  console.log();

  // Refresh state
  gameState = (await program.account.game.fetch(
    gamePda
  )) as unknown as GameAccount;
  const maxTries = gameState.maxTries;

  while (!gameState.isFinished) {
    const attemptNum = gameState.attempts + 1;

    const guessNum = await askNumber(
      rl,
      `  [GUESS] Attempt ${C.bold}${attemptNum}${C.reset}/${maxTries} — Your guess: `,
      1,
      100
    );

    try {
      const sig = await program.methods
        .guess(guessNum)
        .accounts({
          game: gamePda,
          player: adminPk,
        })
        .signers([keypair])
        .rpc();

      console.log();
      console.log(ok(`  [GUESS] Transaction confirmed!`));
      console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);
    } catch (e: any) {
      console.log();
      console.log(err(`  [GUESS] Transaction failed: ${e.message}`));
      if (e.logs) {
        console.log(dim(e.logs.join("\n")));
      }
      continue;
    }

    // Read on-chain state
    await sleep(500); // Small delay for replication
    gameState = (await program.account.game.fetch(
      gamePda
    )) as unknown as GameAccount;

    // Determine result from state
    if (gameState.isFinished && guessNum === secretNumber) {
      console.log();
      console.log(
        `${C.bgGreen}${C.bold}${C.white}  CORRECT!  ${C.reset} You guessed ${C.bold}${guessNum}${C.reset} in ${C.bold}${gameState.attempts}${C.reset} attempts!`
      );
    } else if (gameState.isFinished && gameState.attempts >= maxTries) {
      console.log();
      console.log(
        `${C.bgRed}${C.bold}${C.white}  GAME OVER!  ${C.reset} No more attempts. The secret was ${C.bold}${secretNumber}${C.reset}.`
      );
    } else if (guessNum < secretNumber) {
      console.log(
        warn(
          `  >> ${guessNum} is too small! (${
            maxTries - gameState.attempts
          } left)`
        )
      );
    } else {
      console.log(
        warn(
          `  >> ${guessNum} is too big! (${maxTries - gameState.attempts} left)`
        )
      );
    }
    console.log();
  }

  // ──────────────────────────────────────────────────────────────────────────
  //  FINAL STATE
  // ──────────────────────────────────────────────────────────────────────────

  console.log(banner("━━━ FINAL GAME STATE ━━━"));
  console.log();
  console.log(formatGame(gameState));
  console.log();
  console.log(dim("  Game PDA: " + gamePda.toBase58()));
  console.log();

  rl.close();
}

// ── Transaction helpers ───────────────────────────────────────────────────────

async function doInitialize(
  program: OnChainProgram,
  keypair: Keypair,
  gamePda: PublicKey,
  adminPk: PublicKey,
  secretNumber: number
) {
  console.log();
  console.log(`${C.bold}[INITIALIZE]${C.reset}  Creating game with secret...`);
  console.log(dim(`  Secret will be stored as blake3 hash on-chain.`));

  try {
    const sig = await program.methods
      .initialize(secretNumber)
      .accounts({
        game: gamePda,
        admin: adminPk,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([keypair])
      .rpc();

    console.log(ok(`  [INITIALIZE]  Transaction confirmed!`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);

    const state = (await program.account.game.fetch(
      gamePda
    )) as unknown as GameAccount;
    console.log(dim(formatGame(state)));
  } catch (e: any) {
    console.log(err(`  [INITIALIZE]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    process.exit(1);
  }
}

async function doReveal(
  program: OnChainProgram,
  keypair: Keypair,
  gamePda: PublicKey,
  adminPk: PublicKey,
  secretNumber: number
) {
  console.log();
  console.log(`${C.bold}[REVEAL]${C.reset}  Revealing secret to on-chain...`);
  console.log(dim("  Program verifies: blake3(secret) == stored_hash"));

  try {
    const sig = await program.methods
      .reveal(secretNumber)
      .accounts({
        game: gamePda,
        admin: adminPk,
      })
      .signers([keypair])
      .rpc();

    console.log(ok(`  [REVEAL]  Hash verified! Secret is now on-chain.`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);

    const state = (await program.account.game.fetch(
      gamePda
    )) as unknown as GameAccount;
    console.log(dim(formatGame(state)));
  } catch (e: any) {
    console.log(err(`  [REVEAL]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    process.exit(1);
  }
}

async function doCloseGame(
  program: OnChainProgram,
  keypair: Keypair,
  gamePda: PublicKey,
  adminPk: PublicKey
) {
  try {
    const sig = await program.methods
      .closeGame()
      .accounts({
        game: gamePda,
        admin: adminPk,
      })
      .signers([keypair])
      .rpc();

    console.log(ok(`  [CLOSE]  Game account closed, rent recovered.`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);
  } catch (e: any) {
    console.log(err(`  [CLOSE]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    process.exit(1);
  }
}

// ── Run ───────────────────────────────────────────────────────────────────────

main().catch((e) => {
  console.error(err(`Fatal: ${e.message}`));
  process.exit(1);
});
