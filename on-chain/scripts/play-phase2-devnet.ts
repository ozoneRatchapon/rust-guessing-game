/**
 * Interactive Solana Guessing Game — Phase 2 (Switchboard VRF) on Devnet
 *
 * Plays the on-chain Switchboard VRF guessing game:
 *   1. Initialize  – create randomness account, commit, init game
 *   2. Wait ~3s    – oracle generates randomness
 *   3. Settle      – reveal randomness + settle_random on-chain
 *   4. Guess loop  – player guesses (1-100), up to 10 attempts
 *
 * Run:  npx tsx scripts/play-phase2-devnet.ts
 */

import * as anchor from "@anchor-lang/core";
import { Program, AnchorProvider, Wallet } from "@anchor-lang/core";
import {
  Connection,
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  AnchorProvider as AnchorProvider31,
  Program as Program31,
  Wallet as Wallet31,
} from "@coral-xyz/anchor-31";
import * as sb from "@switchboard-xyz/on-demand";
import * as fs from "fs";
import * as path from "path";
import * as readline from "readline/promises";

// ─── Typed program interface ─────────────────────────────────────────────────

interface GameV2Account {
  admin: PublicKey;
  secretHash: Uint8Array | number[];
  secretNumber: number;
  isRevealed: boolean;
  attempts: number;
  maxTries: number;
  isFinished: boolean;
  bump: number;
  randomnessAccount: PublicKey;
  commitSlot: bigint;
}

interface Phase2VrfProgram extends Program {
  account: {
    gameV2: {
      fetch(address: PublicKey): Promise<GameV2Account>;
    };
  } & Program["account"];
  methods: {
    initialize(): any;
    settleRandom(): any;
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
  "CHXkyr3GrLvWRXdbnYgPMKhwU1dYF6gW9aUpV8S3oTJw"
);
const KEYPAIR_PATH = path.join(
  process.env.HOME!,
  ".config",
  "solana",
  "id.json"
);

// Switchboard devnet addresses (from @switchboard-xyz/on-demand package)
const SWITCHBOARD_QUEUE = new PublicKey(
  "EYiAmGSdsQTuCw413V5BzaruWuCCSDgTPtBGvLkXHbe7"
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

type Phase2Idl = {
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

function formatGame(state: GameV2Account): string {
  const hashArr = Array.from(state.secretHash as Uint8Array | number[]);
  return [
    `    admin:              ${state.admin.toBase58()}`,
    `    secret_hash:        [${hashArr.slice(0, 4).join(", ")}...${hashArr
      .slice(-2)
      .join(", ")}]`,
    `    secret_number:      ${state.isRevealed ? state.secretNumber : "???"}`,
    `    is_revealed:        ${state.isRevealed}`,
    `    attempts:           ${state.attempts}`,
    `    max_tries:          ${state.maxTries}`,
    `    is_finished:        ${state.isFinished}`,
    `    bump:               ${state.bump}`,
    `    randomness_account: ${state.randomnessAccount.toBase58()}`,
    `    commit_slot:        ${state.commitSlot}`,
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

async function sendAndLog(
  connection: Connection,
  tx: Transaction,
  signers: Keypair[],
  label: string
): Promise<string> {
  const sig = await sendAndConfirmTransaction(connection, tx, signers, {
    commitment: "confirmed",
    skipPreflight: false,
  });
  console.log(ok(`  [${label}]  Transaction confirmed!`));
  console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);
  return sig;
}

async function doCloseGame(
  program: Phase2VrfProgram,
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
  console.log(banner("  Phase 2: Switchboard VRF Edition"));
  console.log(banner("============================================"));
  console.log();
  console.log(dim("How it works:"));
  console.log(dim("  1. Initialize with Switchboard VRF randomness commitment"));
  console.log(dim("  2. Wait ~3s for oracle to generate randomness"));
  console.log(dim("  3. Settle randomness on-chain (secret determined by VRF)"));
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

  // ── Provider & Program (our Phase 2 program) ─────────────────────────────

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
    "phase2_vrf.json"
  );
  const idl: Phase2Idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(
    idl as any,
    provider
  ) as unknown as Phase2VrfProgram;

  console.log(`${C.bold}[SETUP]${C.reset}  Program:  ${PROGRAM_ID.toBase58()}`);
  console.log(
    `${C.bold}[SETUP]${C.reset}  IDL:      ${idl.metadata.name} v${idl.metadata.version}`
  );

  // ── Switchboard program (Anchor 0.31) ─────────────────────────────────────

  const sbWallet = new Wallet31(keypair);
  const sbProvider = new AnchorProvider31(connection, sbWallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  const sbProgramId = await sb.getProgramId(connection);
  const sbProgram = await Program31.at(sbProgramId, sbProvider);

  console.log(
    `${C.bold}[SETUP]${C.reset}  Switchboard PID: ${sbProgramId.toBase58()}`
  );
  console.log(
    `${C.bold}[SETUP]${C.reset}  Switchboard Queue: ${SWITCHBOARD_QUEUE.toBase58()}`
  );
  console.log();

  // ── PDA ───────────────────────────────────────────────────────────────────

  const [gamePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("game_v2"), adminPk.toBytes()],
    PROGRAM_ID
  );
  console.log(`${C.bold}[SETUP]${C.reset}  Game PDA: ${gamePda.toBase58()}`);
  console.log();

  // ── Check existing game ───────────────────────────────────────────────────

  let existingGame: GameV2Account | null = null;
  try {
    existingGame = (await program.account.gameV2.fetch(
      gamePda
    )) as unknown as GameV2Account;
  } catch {
    // No game account exists yet
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 1: INITIALIZE (with Switchboard VRF)
  // ──────────────────────────────────────────────────────────────────────────

  let rngAccount: sb.Randomness;
  let rngKp: Keypair;

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
      // Fall through to create new game below
    } else if (!existingGame.isRevealed) {
      console.log(
        warn("  Game exists but randomness not settled yet.")
      );
      const settle = await askYesNo(rl, "  Try to settle now? [Y/n]: ");
      if (settle) {
        // Reconstruct randomness instance from existing account
        rngAccount = new sb.Randomness(
          sbProgram,
          existingGame.randomnessAccount
        );
        // Skip to settle step
        await doSettleStep(
          program,
          sbProgram,
          rngAccount,
          keypair,
          gamePda,
          adminPk,
          connection
        );
        // Then go to guess loop
        let gameState = (await program.account.gameV2.fetch(
          gamePda
        )) as unknown as GameV2Account;
        await guessLoop(program, keypair, gamePda, adminPk, gameState, rl);
        rl.close();
        return;
      }
      console.log(dim("  Exiting."));
      rl.close();
      return;
    } else {
      // Game is revealed but not finished — go to guess loop
      console.log(ok("  Game is active and randomness settled. Resuming."));
      await guessLoop(program, keypair, gamePda, adminPk, existingGame, rl);
      rl.close();
      return;
    }
  }

  // ── Fresh initialization ──────────────────────────────────────────────────

  console.log();
  console.log(
    `${C.bold}[INITIALIZE]${C.reset}  Creating game with Switchboard VRF...`
  );
  console.log(
    dim("  The secret number will be determined by VRF randomness.")
  );

  try {
    // 1. Create randomness account + commit instructions
    const [rng, kp, rngIxs] = await sb.Randomness.createAndCommitIxs(
      sbProgram,
      SWITCHBOARD_QUEUE,
      adminPk
    );
    rngAccount = rng;
    rngKp = kp;

    console.log(
      dim(`  Randomness account: ${rngAccount.pubkey.toBase58()}`)
    );

    // 2. Build our initialize instruction
    const initIx = await program.methods
      .initialize()
      .accounts({
        game: gamePda,
        randomnessAccount: rngAccount.pubkey,
        admin: adminPk,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    // 3. Send all instructions in one transaction
    const tx = new Transaction().add(...rngIxs, initIx);
    const sig = await sendAndConfirmTransaction(
      connection,
      tx,
      [keypair, rngKp],
      { commitment: "confirmed", skipPreflight: false }
    );

    console.log(ok(`  [INITIALIZE]  Transaction confirmed!`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);

    const state = (await program.account.gameV2.fetch(
      gamePda
    )) as unknown as GameV2Account;
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
  //  STEP 2: WAIT FOR ORACLE
  // ──────────────────────────────────────────────────────────────────────────

  console.log(
    `${C.bold}[WAIT]${C.reset}  Waiting ~3s for Switchboard oracle to generate randomness...`
  );
  await sleep(3000);
  console.log(ok("  Done waiting."));
  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 3: SETTLE RANDOM
  // ──────────────────────────────────────────────────────────────────────────

  let gameState = (await program.account.gameV2.fetch(
    gamePda
  )) as unknown as GameV2Account;

  if (!gameState.isRevealed) {
    await doSettleStep(
      program,
      sbProgram,
      rngAccount!,
      keypair,
      gamePda,
      adminPk,
      connection
    );
    gameState = (await program.account.gameV2.fetch(
      gamePda
    )) as unknown as GameV2Account;
  } else {
    console.log(dim("[SETTLE]  Randomness already settled. Skipping."));
  }

  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 4: GUESS LOOP
  // ──────────────────────────────────────────────────────────────────────────

  await guessLoop(program, keypair, gamePda, adminPk, gameState, rl);

  rl.close();
}

// ─── Settle step (reusable) ──────────────────────────────────────────────────

async function doSettleStep(
  program: Phase2VrfProgram,
  sbProgram: Program31,
  rngAccount: sb.Randomness,
  keypair: Keypair,
  gamePda: PublicKey,
  adminPk: PublicKey,
  connection: Connection
) {
  console.log(
    `${C.bold}[SETTLE]${C.reset}  Revealing VRF randomness and settling on-chain...`
  );

  try {
    // 1. Get Switchboard reveal instruction
    const revealIx = await rngAccount.revealIx(adminPk);

    // 2. Build our settle_random instruction
    const settleIx = await program.methods
      .settleRandom()
      .accounts({
        game: gamePda,
        randomnessAccount: rngAccount.pubkey,
        admin: adminPk,
      })
      .instruction();

    // 3. Send both in one transaction
    const tx = new Transaction().add(revealIx, settleIx);
    const sig = await sendAndConfirmTransaction(connection, tx, [keypair], {
      commitment: "confirmed",
      skipPreflight: false,
    });

    console.log(
      ok(`  [SETTLE]  Randomness revealed! Secret number is now on-chain.`)
    );
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);

    const state = (await program.account.gameV2.fetch(
      gamePda
    )) as unknown as GameV2Account;
    console.log(dim(formatGame(state)));
  } catch (e: any) {
    console.log(err(`  [SETTLE]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    console.log(
      warn(
        "  Hint: The oracle might need more time. Try re-running the script and choosing 'settle now'."
      )
    );
    process.exit(1);
  }
}

// ─── Guess loop (reusable) ───────────────────────────────────────────────────

async function guessLoop(
  program: Phase2VrfProgram,
  keypair: Keypair,
  gamePda: PublicKey,
  adminPk: PublicKey,
  gameState: GameV2Account,
  rl: readline.Interface
) {
  if (gameState.isFinished) {
    console.log(warn("Game is already finished!"));
    console.log(formatGame(gameState));
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
    gameState = (await program.account.gameV2.fetch(
      gamePda
    )) as unknown as GameV2Account;

    // Determine result from state
    if (gameState.isFinished && gameState.attempts <= maxTries) {
      // Check if won or lost by looking at events / state
      // In the VRF version, the secret is on-chain after reveal
      if (guessNum === gameState.secretNumber) {
        console.log();
        console.log(
          `${C.bgGreen}${C.bold}${C.white}  CORRECT!  ${C.reset} You guessed ${C.bold}${guessNum}${C.reset} in ${C.bold}${gameState.attempts}${C.reset} attempts!`
        );
      } else {
        console.log();
        console.log(
          `${C.bgRed}${C.bold}${C.white}  GAME OVER!  ${C.reset} No more attempts. The secret was ${C.bold}${gameState.secretNumber}${C.reset}.`
        );
      }
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
          `  >> ${guessNum} is too big! (${
            maxTries - gameState.attempts
          } left)`
        )
      );
    }
    console.log();
  }

  // ─── Final state ────────────────────────────────────────────────────────

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
