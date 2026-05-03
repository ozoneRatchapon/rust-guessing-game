/**
 * Interactive Solana Guessing Game — Phase 4 (Multi-Player Tournament) on Devnet
 *
 * Plays the on-chain multi-player tournament with Switchboard VRF:
 *   1. Admin creates tournament with Switchboard VRF randomness commitment
 *   2. Wait ~3s for oracle to generate randomness
 *   3. Admin settles — reveal randomness → secret determined by VRF
 *   4. Players join (up to 16, each is a generated keypair)
 *   5. Players submit guesses (10 tries each), with too-small/too-big feedback
 *   6. Show final rankings (exact matches first, then sorted by best_distance)
 *   7. Admin closes tournament — rent recovered
 *
 * Prerequisites:
 *   - Program deployed to devnet
 *   - Switchboard VRF oracle active on devnet
 *   - Keypair funded with devnet SOL
 *
 * Run:  npx tsx scripts/play-phase4-devnet.ts
 *       npx tsx scripts/play-phase4-devnet.ts --local
 */

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

interface TournamentAccount {
  admin: PublicKey;
  secretHash: Uint8Array | number[];
  secretNumber: number;
  isSettled: boolean;
  maxTriesPerPlayer: number;
  playerCount: number;
  maxPlayers: number;
  isFinished: boolean;
  bump: number;
  randomnessAccount: PublicKey;
  commitSlot: bigint;
}

interface PlayerEntryAccount {
  player: PublicKey;
  tournament: PublicKey;
  guessCount: number;
  bestDistance: number;
  foundExact: boolean;
  bump: number;
}

interface Phase4Program extends Program {
  account: {
    tournament: { fetch(address: PublicKey): Promise<TournamentAccount> };
    playerEntry: { fetch(address: PublicKey): Promise<PlayerEntryAccount> };
  } & Program["account"];
  methods: {
    createTournament(): any;
    settleTournament(): any;
    joinTournament(): any;
    submitGuess(guess: number): any;
    closeTournament(): any;
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
  "FKqXgQYFUgMifKoQTYbb5UzMLry6RDo9E6dWm6E4fKoL"
);
const KEYPAIR_PATH = path.join(
  process.env.HOME!,
  ".config",
  "solana",
  "id.json"
);

// Switchboard devnet addresses
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

// ─── IDL type ─────────────────────────────────────────────────────────────────

type Phase4Idl = {
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

function formatTournament(state: TournamentAccount): string {
  const hashArr = Array.from(state.secretHash as Uint8Array | number[]);
  return [
    `    admin:               ${state.admin.toBase58()}`,
    `    secret_hash:         [${hashArr.slice(0, 4).join(", ")}...${hashArr
      .slice(-2)
      .join(", ")}]`,
    `    secret_number:       ${state.isSettled ? state.secretNumber : "???"}`,
    `    is_settled:          ${state.isSettled}`,
    `    max_tries_per_player:${state.maxTriesPerPlayer}`,
    `    player_count:        ${state.playerCount}`,
    `    max_players:         ${state.maxPlayers}`,
    `    is_finished:         ${state.isFinished}`,
    `    bump:                ${state.bump}`,
    `    randomness_account:  ${state.randomnessAccount.toBase58()}`,
    `    commit_slot:         ${state.commitSlot}`,
  ].join("\n");
}

function formatPlayerEntry(entry: PlayerEntryAccount): string {
  return [
    `    player:        ${entry.player.toBase58()}`,
    `    tournament:    ${entry.tournament.toBase58()}`,
    `    guess_count:   ${entry.guessCount}`,
    `    best_distance: ${entry.bestDistance}`,
    `    found_exact:   ${entry.foundExact}`,
    `    bump:          ${entry.bump}`,
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

function shortKey(pk: PublicKey): string {
  const b58 = pk.toBase58();
  return `${b58.slice(0, 4)}..${b58.slice(-4)}`;
}

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log();
  console.log(
    banner("══════════════════════════════════════════════════════════")
  );
  console.log(banner("  Solana Guessing Game — Devnet"));
  console.log(banner("  Phase 4: Multi-Player Tournament"));
  console.log(
    banner("══════════════════════════════════════════════════════════")
  );
  console.log();
  console.log(dim("How it works:"));
  console.log(
    dim("  1. Admin creates tournament with Switchboard VRF commitment")
  );
  console.log(dim("  2. Wait ~3s for oracle to generate randomness"));
  console.log(dim("  3. Admin settles — VRF randomness reveals the secret"));
  console.log(dim("  4. Players join the tournament (up to 16)"));
  console.log(dim("  5. Each player gets 10 guesses with feedback"));
  console.log(
    dim("  6. Final rankings: exact matches first, then by distance")
  );
  console.log(dim("  7. Admin closes tournament — rent recovered"));
  console.log();

  // ── Load keypair ──────────────────────────────────────────────────────────

  const keypair = loadKeypair();
  const adminPk = keypair.publicKey;
  console.log(`${C.bold}[SETUP]${C.reset}  Admin:    ${adminPk.toBase58()}`);

  // ── Connect ───────────────────────────────────────────────────────────────

  const connection = new Connection(DEVNET_URL, "confirmed");
  console.log(`${C.bold}[SETUP]${C.reset}  Network:  ${DEVNET_URL}`);

  const balance = await connection.getBalance(adminPk);
  console.log(
    `${C.bold}[SETUP]${C.reset}  Balance:  ${(
      balance / LAMPORTS_PER_SOL
    ).toFixed(4)} SOL`
  );

  if (balance < 0.1 * LAMPORTS_PER_SOL) {
    console.log(warn("  Balance low! Requesting airdrop..."));
    const sig = await connection.requestAirdrop(adminPk, 2 * LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig, "confirmed");
    console.log(ok("  Airdrop confirmed! +2 SOL"));
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
    "phase4_tournament.json"
  );
  const idl: Phase4Idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(idl as any, provider) as unknown as Phase4Program;

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
    `${C.bold}[SETUP]${
      C.reset
    }  Switchboard Queue: ${SWITCHBOARD_QUEUE.toBase58()}`
  );
  console.log();

  // ── PDA ───────────────────────────────────────────────────────────────────

  const [tournamentPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("tournament"), adminPk.toBytes()],
    PROGRAM_ID
  );
  console.log(
    `${C.bold}[SETUP]${C.reset}  Tournament PDA: ${tournamentPda.toBase58()}`
  );
  console.log();

  // ── Check existing tournament ─────────────────────────────────────────────

  let existingTournament: TournamentAccount | null = null;
  try {
    existingTournament = (await program.account.tournament.fetch(
      tournamentPda
    )) as unknown as TournamentAccount;
  } catch {
    // No tournament account exists yet
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  // ── RNG state ─────────────────────────────────────────────────────────────

  let rngAccount: sb.Randomness;
  let rngKp: Keypair;
  let tournamentState: TournamentAccount;

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 1: CREATE TOURNAMENT (with Switchboard VRF)
  // ──────────────────────────────────────────────────────────────────────────

  if (existingTournament) {
    console.log(
      warn("[CREATE]  Tournament account already exists at this PDA.")
    );
    console.log(dim(formatTournament(existingTournament)));

    if (existingTournament.isFinished) {
      const reinit = await askYesNo(rl, "  Close and re-create? [y/N]: ");
      if (!reinit) {
        console.log(dim("  Exiting."));
        rl.close();
        return;
      }
      console.log(dim("  Closing existing tournament..."));
      await doCloseTournament(program, keypair, tournamentPda, adminPk);
      // Fall through to create new tournament below
    } else if (!existingTournament.isSettled) {
      console.log(warn("  Tournament exists but randomness not settled yet."));
      const settle = await askYesNo(rl, "  Try to settle now? [Y/n]: ");
      if (settle) {
        rngAccount = new sb.Randomness(
          sbProgram,
          existingTournament.randomnessAccount
        );
        await doSettleStep(
          program,
          sbProgram,
          rngAccount,
          keypair,
          tournamentPda,
          adminPk,
          connection
        );
        tournamentState = (await program.account.tournament.fetch(
          tournamentPda
        )) as unknown as TournamentAccount;
        // Go to player phase
        await playerPhase(
          program,
          keypair,
          tournamentPda,
          adminPk,
          tournamentState,
          connection,
          rl
        );
        rl.close();
        return;
      }
      console.log(dim("  Exiting."));
      rl.close();
      return;
    } else {
      // Tournament is settled but not finished — resume player phase
      console.log(
        ok("  Tournament is active and settled. Resuming player phase.")
      );
      tournamentState = existingTournament;
      await playerPhase(
        program,
        keypair,
        tournamentPda,
        adminPk,
        tournamentState,
        connection,
        rl
      );
      rl.close();
      return;
    }
  }

  // ── Fresh creation ────────────────────────────────────────────────────────

  console.log();
  console.log(
    `${C.bold}[CREATE]${C.reset}  Creating tournament with Switchboard VRF...`
  );
  console.log(dim("  The secret number will be determined by VRF randomness."));

  try {
    // 1. Create randomness account + commit instructions
    const [rng, kp, rngIxs] = await sb.Randomness.createAndCommitIxs(
      sbProgram,
      SWITCHBOARD_QUEUE,
      adminPk
    );
    rngAccount = rng;
    rngKp = kp;

    console.log(dim(`  Randomness account: ${rngAccount.pubkey.toBase58()}`));

    // 2. Build our create_tournament instruction
    const initIx = await program.methods
      .createTournament()
      .accounts({
        tournament: tournamentPda,
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

    console.log(ok(`  [CREATE]  Transaction confirmed!`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);

    const state = (await program.account.tournament.fetch(
      tournamentPda
    )) as unknown as TournamentAccount;
    console.log(dim(formatTournament(state)));
  } catch (e: any) {
    console.log(err(`  [CREATE]  Failed: ${e.message}`));
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
  //  STEP 3: SETTLE TOURNAMENT
  // ──────────────────────────────────────────────────────────────────────────

  tournamentState = (await program.account.tournament.fetch(
    tournamentPda
  )) as unknown as TournamentAccount;

  if (!tournamentState.isSettled) {
    await doSettleStep(
      program,
      sbProgram,
      rngAccount!,
      keypair,
      tournamentPda,
      adminPk,
      connection
    );
    tournamentState = (await program.account.tournament.fetch(
      tournamentPda
    )) as unknown as TournamentAccount;
  } else {
    console.log(dim("[SETTLE]  Randomness already settled. Skipping."));
  }

  console.log();
  console.log(
    ok(`  Secret number is: ${C.bold}${tournamentState.secretNumber}${C.reset}`)
  );
  console.log(
    dim("  (Known to admin for testing — players should not know this!)")
  );
  console.log();

  // ──────────────────────────────────────────────────────────────────────────
  //  STEP 4-5: PLAYER PHASE
  // ──────────────────────────────────────────────────────────────────────────

  await playerPhase(
    program,
    keypair,
    tournamentPda,
    adminPk,
    tournamentState,
    connection,
    rl
  );

  rl.close();
}

// ─── Settle step ──────────────────────────────────────────────────────────────

async function doSettleStep(
  program: Phase4Program,
  sbProgram: Program31,
  rngAccount: sb.Randomness,
  keypair: Keypair,
  tournamentPda: PublicKey,
  adminPk: PublicKey,
  connection: Connection
) {
  console.log(
    `${C.bold}[SETTLE]${C.reset}  Revealing VRF randomness and settling tournament...`
  );

  try {
    // 1. Get Switchboard reveal instruction
    const revealIx = await rngAccount.revealIx(adminPk);

    // 2. Build our settle_tournament instruction
    const settleIx = await program.methods
      .settleTournament()
      .accounts({
        tournament: tournamentPda,
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

    const state = (await program.account.tournament.fetch(
      tournamentPda
    )) as unknown as TournamentAccount;
    console.log(dim(formatTournament(state)));
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

// ─── Player phase ─────────────────────────────────────────────────────────────

async function playerPhase(
  program: Phase4Program,
  adminKeypair: Keypair,
  tournamentPda: PublicKey,
  adminPk: PublicKey,
  tournamentState: TournamentAccount,
  connection: Connection,
  rl: readline.Interface
) {
  console.log(banner("━━━ PLAYER PHASE ━━━"));
  console.log();

  // Ask how many players
  const numPlayers = await askNumber(
    rl,
    `  How many players? (1-${tournamentState.maxPlayers}): `,
    1,
    tournamentState.maxPlayers
  );
  console.log();

  // Track all players
  const players: {
    keypair: Keypair;
    publicKey: PublicKey;
    playerEntryPda: PublicKey;
    entry: PlayerEntryAccount | null;
  }[] = [];

  // ── Join phase ────────────────────────────────────────────────────────────

  console.log(banner("  ── JOIN PHASE ──"));
  console.log();

  for (let i = 0; i < numPlayers; i++) {
    const playerKp = Keypair.generate();
    const playerPk = playerKp.publicKey;

    // Derive PlayerEntry PDA
    const [playerEntryPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("player"), tournamentPda.toBytes(), playerPk.toBytes()],
      PROGRAM_ID
    );

    console.log(`${C.bold}  Player ${i + 1}${C.reset}: ${shortKey(playerPk)}`);

    // Fund player via SOL transfer from admin (avoids devnet faucet rate limits)
    console.log(dim(`    Funding player account...`));
    try {
      const transferIx = SystemProgram.transfer({
        fromPubkey: adminKeypair.publicKey,
        toPubkey: playerPk,
        lamports: 0.05 * LAMPORTS_PER_SOL,
      });
      const tx = new Transaction().add(transferIx);
      const sig = await sendAndConfirmTransaction(
        connection,
        tx,
        [adminKeypair],
        {
          commitment: "confirmed",
          skipPreflight: false,
        }
      );
      console.log(dim(`    Transferred 0.05 SOL from admin`));
    } catch (e: any) {
      console.log(warn(`    Transfer failed: ${e.message}`));
      console.log(dim(`    Trying to continue anyway...`));
    }

    // Join tournament
    try {
      const sig = await program.methods
        .joinTournament()
        .accounts({
          tournament: tournamentPda,
          playerEntry: playerEntryPda,
          player: playerPk,
          systemProgram: SystemProgram.programId,
        })
        .signers([playerKp])
        .rpc();

      console.log(ok(`    Joined! ${link(explorerUrl(sig))}`));
    } catch (e: any) {
      console.log(err(`    Join failed: ${e.message}`));
      if (e.logs) {
        console.log(dim(e.logs.join("\n")));
      }
      console.log(dim(`    Skipping player ${i + 1}...`));
      continue;
    }

    const entry = (await program.account.playerEntry.fetch(
      playerEntryPda
    )) as unknown as PlayerEntryAccount;

    players.push({
      keypair: playerKp,
      publicKey: playerPk,
      playerEntryPda,
      entry,
    });
    console.log();
  }

  if (players.length === 0) {
    console.log(err("  No players joined. Exiting."));
    return;
  }

  // Refresh tournament state
  tournamentState = (await program.account.tournament.fetch(
    tournamentPda
  )) as unknown as TournamentAccount;

  // ── Guess phase ───────────────────────────────────────────────────────────

  console.log(banner("  ── GUESS PHASE ──"));
  console.log();
  console.log(
    `  Each player gets ${C.bold}${tournamentState.maxTriesPerPlayer}${C.reset} guesses (1-100).`
  );
  console.log(`  Secret number: ${C.bold}${C.red}(hidden)${C.reset}`);
  console.log();

  // Each player guesses in turn
  for (let pi = 0; pi < players.length; pi++) {
    const p = players[pi];
    console.log(banner(`  ── Player ${pi + 1}: ${shortKey(p.publicKey)} ──`));
    console.log();

    // Refresh entry
    p.entry = (await program.account.playerEntry.fetch(
      p.playerEntryPda
    )) as unknown as PlayerEntryAccount;

    const maxTries = tournamentState.maxTriesPerPlayer;

    while (p.entry.guessCount < maxTries && !p.entry.foundExact) {
      const attemptNum = p.entry.guessCount + 1;

      const guessNum = await askNumber(
        rl,
        `    ${C.bold}[GUESS]${C.reset} Player ${pi + 1} — Attempt ${
          C.bold
        }${attemptNum}${C.reset}/${maxTries}: `,
        1,
        100
      );

      try {
        const sig = await program.methods
          .submitGuess(guessNum)
          .accounts({
            tournament: tournamentPda,
            playerEntry: p.playerEntryPda,
            player: p.publicKey,
          })
          .signers([p.keypair])
          .rpc();

        console.log(dim(`    Confirmed: ${link(explorerUrl(sig))}`));
      } catch (e: any) {
        console.log(err(`    Guess failed: ${e.message}`));
        if (e.logs) {
          console.log(dim(e.logs.join("\n")));
        }
        continue;
      }

      // Read updated PlayerEntry
      await sleep(500);
      p.entry = (await program.account.playerEntry.fetch(
        p.playerEntryPda
      )) as unknown as PlayerEntryAccount;

      // Determine result from state
      if (p.entry.foundExact) {
        console.log(
          `    ${C.bgGreen}${C.bold}${C.white} CORRECT! ${C.reset} Guessed ${
            C.bold
          }${guessNum}${C.reset} in ${C.bold}${p.entry.guessCount}${
            C.reset
          } attempt${p.entry.guessCount !== 1 ? "s" : ""}!`
        );
      } else {
        // Compare guess to the actual secret (readable on-chain after settle)
        if (guessNum < tournamentState.secretNumber) {
          console.log(
            warn(
              `    >> ${guessNum} is too small! (${
                maxTries - p.entry.guessCount
              } left, best distance: ${p.entry.bestDistance})`
            )
          );
        } else {
          console.log(
            warn(
              `    >> ${guessNum} is too big! (${
                maxTries - p.entry.guessCount
              } left, best distance: ${p.entry.bestDistance})`
            )
          );
        }
      }
      console.log();
    }

    // Summary for this player
    if (p.entry!.foundExact) {
      console.log(
        ok(
          `  Player ${pi + 1} (${shortKey(p.publicKey)}): Found it in ${
            p.entry!.guessCount
          } attempts!`
        )
      );
    } else {
      console.log(
        warn(
          `  Player ${pi + 1} (${shortKey(
            p.publicKey
          )}): Exhausted all attempts. Best distance: ${p.entry!.bestDistance}`
        )
      );
    }
    console.log();
  }

  // ── Final rankings ────────────────────────────────────────────────────────

  console.log(banner("━━━ FINAL RANKINGS ━━━"));
  console.log();

  // Refresh all entries
  for (const p of players) {
    p.entry = (await program.account.playerEntry.fetch(
      p.playerEntryPda
    )) as unknown as PlayerEntryAccount;
  }

  // Sort: exact matches first (sorted by fewer guesses), then by best_distance
  const ranked = [...players].sort((a, b) => {
    const eA = a.entry!;
    const eB = b.entry!;

    // Exact matches come first
    if (eA.foundExact !== eB.foundExact) {
      return eA.foundExact ? -1 : 1;
    }

    if (eA.foundExact && eB.foundExact) {
      // Both found exact: fewer guesses wins
      return eA.guessCount - eB.guessCount;
    }

    // Neither found exact: lower best_distance wins
    return eA.bestDistance - eB.bestDistance;
  });

  const medal = ["🥇", "🥈", "🥉"];

  for (let i = 0; i < ranked.length; i++) {
    const p = ranked[i];
    const e = p.entry!;
    const m = i < 3 ? medal[i] : ` ${i + 1}.`;

    if (e.foundExact) {
      console.log(
        `  ${m} ${C.bold}${shortKey(p.publicKey)}${C.reset} — ${ok(
          `CORRECT`
        )} in ${e.guessCount} attempts`
      );
    } else {
      console.log(
        `  ${m} ${C.bold}${shortKey(p.publicKey)}${C.reset} — distance: ${
          e.bestDistance
        } (${e.guessCount} attempts, no exact match)`
      );
    }
  }

  console.log();
  console.log(
    dim(
      `  Secret number was: ${C.bold}${tournamentState.secretNumber}${C.reset}`
    )
  );
  console.log();

  // ── Close tournament ──────────────────────────────────────────────────────

  console.log(banner("━━━ CLEANUP ━━━"));
  console.log();

  const doClose = await askYesNo(
    rl,
    "  Close tournament and recover rent? [Y/n]: "
  );
  if (doClose) {
    await doCloseTournament(program, adminKeypair, tournamentPda, adminPk);
  } else {
    console.log(dim("  Tournament left open. You can close it later."));
  }
}

// ─── Close tournament ─────────────────────────────────────────────────────────

async function doCloseTournament(
  program: Phase4Program,
  keypair: Keypair,
  tournamentPda: PublicKey,
  adminPk: PublicKey
) {
  try {
    const sig = await program.methods
      .closeTournament()
      .accounts({
        tournament: tournamentPda,
        admin: adminPk,
      })
      .signers([keypair])
      .rpc();

    console.log(ok(`  [CLOSE]  Tournament closed, rent recovered.`));
    console.log(`  ${C.bold}Explorer:${C.reset} ${link(explorerUrl(sig))}`);
  } catch (e: any) {
    console.log(err(`  [CLOSE]  Failed: ${e.message}`));
    if (e.logs) {
      console.log(dim(e.logs.join("\n")));
    }
    process.exit(1);
  }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

main().catch((e) => {
  console.error(err(`Fatal: ${e.message}`));
  process.exit(1);
});
