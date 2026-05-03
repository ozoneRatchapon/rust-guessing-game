/**
 * Interactive Solana Guessing Game — Phase 3 (MagicBlock VRF) on Devnet
 *
 * Plays the on-chain MagicBlock VRF guessing game:
 *   1. Initialize  – create game PDA, ready for VRF request
 *   2. Request     – send VRF randomness request (CPI to MagicBlock VRF)
 *   3. Wait ~1s    – oracle generates randomness and calls back
 *   4. Guess loop  – player guesses (1-100), up to 10 attempts
 *   5. Close       – recover rent lamports
 *
 * Prerequisites:
 *   - Program deployed to devnet
 *   - MagicBlock VRF oracle active on devnet
 *   - Keypair funded with devnet SOL
 *
 * Run:  npx tsx scripts/play-phase3-devnet.ts
 *       npx tsx scripts/play-phase3-devnet.ts --local
 */

import { Program, AnchorProvider, Wallet, Idl } from "@anchor-lang/core";
import {
  Connection,
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
  SystemProgram,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as readline from "readline/promises";

// ─── Typed program interface ─────────────────────────────────────────────────

interface GameV3Account {
  admin: PublicKey;
  secretHash: Uint8Array | number[];
  secretNumber: number;
  isRevealed: boolean;
  attempts: number;
  maxTries: number;
  isFinished: boolean;
  bump: number;
  vrfRequestPending: boolean;
}

interface Phase3VrfProgram extends Program {
  account: {
    gameV3: {
      fetch(address: PublicKey): Promise<GameV3Account>;
    };
  } & Program["account"];
  methods: {
    initialize(clientSeed: number): any;
    requestRandomness(clientSeed: number): any;
    consumeRandomness(randomness: number[]): any;
    guess(guessNumber: number): any;
    closeGame(): any;
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
  "DnrNKTTspzjip8CAFXzCNkbMbQKXjNbZGnx6gNGtCEAH"
);
const KEYPAIR_PATH = path.join(
  process.env.HOME!,
  ".config",
  "solana",
  "id.json"
);

// MagicBlock VRF addresses (mainnet/devnet)
const VRF_PROGRAM_ID = new PublicKey(
  "Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz"
);
const DEFAULT_QUEUE = new PublicKey(
  "Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh"
);
const SLOT_HASHES_SYSVAR = new PublicKey(
  "SysvarS1otHashes111111111111111111111111111"
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

// ─── Helpers ──────────────────────────────────────────────────────────────────

function explorerUrl(signature: string): string {
  return `${EXPLORER_TX}/${signature}?cluster=${CLUSTER}`;
}

function formatGame(state: GameV3Account): string {
  const hashArr = Array.from(state.secretHash as Uint8Array | number[]);
  return [
    `    admin:               ${state.admin.toBase58()}`,
    `    secret_hash:         [${hashArr.slice(0, 4).join(", ")}...${hashArr
      .slice(-2)
      .join(", ")}]`,
    `    secret_number:       ${state.isRevealed ? state.secretNumber : "???"}`,
    `    is_revealed:         ${state.isRevealed}`,
    `    attempts:            ${state.attempts}`,
    `    max_tries:           ${state.maxTries}`,
    `    is_finished:         ${state.isFinished}`,
    `    bump:                ${state.bump}`,
    `    vrf_request_pending: ${state.vrfRequestPending}`,
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

// ─── Transaction helpers ──────────────────────────────────────────────────────

async function doCloseGame(
  program: Phase3VrfProgram,
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

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log();
  console.log(banner("============================================"));
  console.log(banner("  Solana Guessing Game — Devnet"));
  console.log(banner("  Phase 3: MagicBlock VRF Edition"));
  console.log(banner("============================================"));
  console.log();
  console.log(dim("How it works:"));
  console.log(dim("  1. Initialize — create game PDA"));
  console.log(dim("  2. Request randomness — CPI to MagicBlock VRF oracle"));
  console.log(dim("  3. Wait ~1s — oracle calls back with random bytes"));
  console.log(dim("  4. YOU guess the number (1-100)"));
  console.log(dim("  5. You have 10 attempts"));
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

  const idlPath = path.join(
    __dirname,
    "..",
    "target",
    "idl",
    "phase3_magicblock_vrf.json"
  );
  const idl: Idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(idl, provider) as unknown as Phase3VrfProgram;

  console.log(`${C.bold}[SETUP]${C.reset}  Program:  ${PROGRAM_ID.toBase58()}`);
  console.log(
    `${C.bold}[SETUP]${C.reset}  VRF:      ${VRF_PROGRAM_ID.toBase58()}`
  );
  console.log(
    `${C.bold}[SETUP]${C.reset}  Queue:    ${DEFAULT_QUEUE.toBase58()}`
  );
  console.log();

  // ── PDA ───────────────────────────────────────────────────────────────────

  const [gamePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("game_v3"), adminPk.toBytes()],
    PROGRAM_ID
  );
  console.log(`${C.bold}[SETUP]${C.reset}  Game PDA: ${gamePda.toBase58()}`);
  console.log();

  // ── Check existing game ───────────────────────────────────────────────────

  let existingGame: GameV3Account | null = null;
  try {
    existingGame = (await program.account.gameV3.fetch(
      gamePda
    )) as unknown as GameV3Account;
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

  if (existingGame) {
    console.log(warn("[INITIALIZE]  Game account already exists at this PDA."));
    console.log(dim(formatGame(existingGame)));

    if (existingGame.isFinished) {
      const reinit = await askYesNo(rl, "  Close and re-initialize? [y/N]: ");
      if (!reinit) {
        console.log(dim("  Exiting."));
        rl.close();
        return;
      }
      console.log(dim("  Closing existing game account..."));
      await doCloseGame(program, keypair, gamePda, adminPk);
      // Fall through to create new game
    } else if (existingGame.isRevealed && !existingGame.isFinished) {
      console.log(
        ok("  Game is active and randomness consumed. Resuming guess loop.")
      );
      await guessLoop(program, keypair, gamePda, adminPk, existingGame, rl);
      rl.close();
      return;
    } else if (!existingGame.isRevealed && existingGame.vrfRequestPending) {
      console.log(warn("  VRF request is pending. Waiting for callback..."));
      await waitForVrfCallback(program, gamePda, connection);
      const gameState = (await program.account.gameV3.fetch(
        gamePda
      )) as unknown as GameV3Account;
      if (gameState.isRevealed) {
        await guessLoop(program, keypair, gamePda, adminPk, gameState, rl);
      }
      rl.close();
      return;
    } else {
      console.log(dim("  Exiting."));
      rl.close();
      return;
    }
  }

  // ── Fresh initialization ──────────────────────────────────────────────────

  const clientSeed = await askNumber(
    rl,
    `  Enter a client seed (0-255) for extra entropy: `,
    0,
    255
  );

  console.log();
  console.log(`${C.bold}[INITIALIZE]${C.reset}  Creating game PDA...`);
  console.log(dim("  The secret number will be determined by MagicBlock VRF."));

  try {
    const sig = await program.methods
      .initialize(clientSeed)
      .accounts({
        game: gamePda,
        admin: adminPk,
        systemProgram: SystemProgram.programId,
      })
      .signers([keypair])
      .rpc();

    console.log(ok(`  [INITIALIZE]  Transaction confirmed!`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);

    const state = (await program.account.gameV3.fetch(
      gamePda
    )) as unknown as GameV3Account;
    console.log(dim(formatGame(state)));
  } catch (e: any) {
    console.log(err(`  [INITIALIZE]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    process.exit(1);
  }

  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 2: REQUEST RANDOMNESS
  // ──────────────────────────────────────────────────────────────────────────

  console.log(
    `${C.bold}[REQUEST]${C.reset}  Requesting randomness from MagicBlock VRF...`
  );

  // Derive the identity PDA from OUR program (seeds: [b"identity"], program: our PROGRAM_ID)
  // This matches the on-chain seeds constraint in RequestRandomness.
  // The VRF program verifies the callback by deriving the same PDA from callback_program_id.
  const [programIdentity] = PublicKey.findProgramAddressSync(
    [Buffer.from("identity")],
    PROGRAM_ID
  );

  try {
    const sig = await program.methods
      .requestRandomness(clientSeed)
      .accounts({
        programIdentity: programIdentity,
        oracleQueue: DEFAULT_QUEUE,
        slotHashes: SLOT_HASHES_SYSVAR,
        vrfProgram: VRF_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        payer: adminPk,
      })
      .signers([keypair])
      .rpc();

    console.log(ok(`  [REQUEST]  VRF request submitted!`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);
  } catch (e: any) {
    console.log(err(`  [REQUEST]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    console.log(
      warn(
        "  Hint: MagicBlock VRF oracle might not be available. Ensure the program is deployed and the oracle is active."
      )
    );
    process.exit(1);
  }

  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 3: WAIT FOR VRF CALLBACK
  // ──────────────────────────────────────────────────────────────────────────

  let gameState = await waitForVrfCallback(program, gamePda, connection);

  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 4: GUESS LOOP
  // ──────────────────────────────────────────────────────────────────────────

  await guessLoop(program, keypair, gamePda, adminPk, gameState, rl);

  rl.close();
}

// ─── Wait for VRF callback ────────────────────────────────────────────────────

async function waitForVrfCallback(
  program: Phase3VrfProgram,
  gamePda: PublicKey,
  connection: Connection
): Promise<GameV3Account> {
  console.log(
    `${C.bold}[WAIT]${C.reset}  Waiting for MagicBlock VRF oracle callback (~1-3s)...`
  );

  const maxAttempts = 30;
  let state: GameV3Account | null = null;

  for (let i = 0; i < maxAttempts; i++) {
    await sleep(1000);

    try {
      state = (await program.account.gameV3.fetch(
        gamePda
      )) as unknown as GameV3Account;
    } catch {
      continue;
    }

    if (state!.isRevealed) {
      console.log(
        ok(
          `  [WAIT]  Randomness consumed! Secret is now on-chain (attempt ${
            i + 1
          }).`
        )
      );
      return state!;
    }

    process.stdout.write(dim(`  Waiting... (${i + 1}/${maxAttempts})\r`));
  }

  console.log();
  console.log(
    warn(
      "  VRF callback not received within timeout. The oracle might be slow or unavailable."
    )
  );
  console.log(
    warn(
      "  You can re-run the script — it will detect the pending request and wait again."
    )
  );
  process.exit(1);
}

// ─── Guess loop ───────────────────────────────────────────────────────────────

async function guessLoop(
  program: Phase3VrfProgram,
  keypair: Keypair,
  gamePda: PublicKey,
  adminPk: PublicKey,
  gameState: GameV3Account,
  rl: readline.Interface
) {
  if (gameState.isFinished) {
    console.log(warn("Game is already finished!"));
    console.log(formatGame(gameState));
    return;
  }

  if (!gameState.isRevealed) {
    console.log(err("Randomness not consumed yet. Cannot guess."));
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
    await sleep(500);
    gameState = (await program.account.gameV3.fetch(
      gamePda
    )) as unknown as GameV3Account;

    // Determine result from state
    if (gameState.isFinished && guessNum === gameState.secretNumber) {
      console.log();
      console.log(
        `${C.bgGreen}${C.bold}${C.white}  CORRECT!  ${C.reset} You guessed ${C.bold}${guessNum}${C.reset} in ${C.bold}${gameState.attempts}${C.reset} attempts!`
      );
    } else if (gameState.isFinished) {
      console.log();
      console.log(
        `${C.bgRed}${C.bold}${C.white}  GAME OVER!  ${C.reset} No more attempts. The secret was ${C.bold}${gameState.secretNumber}${C.reset}.`
      );
    } else if (guessNum < gameState.secretNumber) {
      console.log(
        warn(
          `  >> ${guessNum} is too small! (${
            maxTries - gameState.attempts
          } left)`
        )
      );
    } else if (guessNum > gameState.secretNumber) {
      console.log(
        warn(
          `  >> ${guessNum} is too big! (${maxTries - gameState.attempts} left)`
        )
      );
    }
    console.log();
  }

  // ─── Close game ──────────────────────────────────────────────────────────

  const shouldClose = await askYesNo(
    rl,
    "  Close game and recover rent? [Y/n]: "
  );
  if (shouldClose) {
    console.log();
    await doCloseGame(program, keypair, gamePda, adminPk);
  }

  // ─── Final state ────────────────────────────────────────────────────────

  console.log();
  console.log(banner("━━━ FINAL GAME STATE ━━━"));
  console.log();
  console.log(formatGame(gameState));
  console.log();
  console.log(dim("  Game PDA: " + gamePda.toBase58()));
  console.log();
}

main().catch((e) => {
  console.error(err(`Fatal: ${e.message}`));
  process.exit(1);
});
