/**
 * Phase 2 Cost Analysis — Fetches recent Phase 2 transactions from devnet
 * and extracts compute units and fees per instruction type.
 *
 * Run:  npx tsx scripts/cost-analysis-phase2.ts
 */

import { Connection, PublicKey } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey(
  "94g894DkqpuewD8mKHimaBsuzFT7Qz2E9Wb8QPWUBsZ2"
);
const DEVNET_URL = "https://api.devnet.solana.com";

interface TxStats {
  instruction: string;
  computeUnits: number;
  fee: number;
  signature: string;
  isMultiIx: boolean;
}

async function main() {
  const conn = new Connection(DEVNET_URL);

  console.log("Fetching recent transactions for Phase 2 program...");
  console.log(`Program: ${PROGRAM_ID.toBase58()}`);
  console.log();

  const sigs = await conn.getSignaturesForAddress(PROGRAM_ID, { limit: 20 });
  console.log(`Found ${sigs.length} recent signatures\n`);

  const stats: TxStats[] = [];

  for (const sigInfo of sigs) {
    const tx = await conn.getTransaction(sigInfo.signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    if (!tx || !tx.meta) continue;

    const cu = tx.meta.computeUnitsConsumed;
    const fee = tx.meta.fee;
    const logs = tx.meta.logMessages || [];

    // Determine instruction type from program logs
    let instrName = "unknown";
    for (const l of logs) {
      if (l.includes("Instruction: Initialize")) instrName = "initialize";
      if (
        l.includes("Instruction: SettleRandom") ||
        l.includes("Instruction: Settle")
      )
        instrName = "settle_random";
      if (l.includes("Instruction: Guess")) instrName = "guess";
      if (
        l.includes("Instruction: CloseGame") ||
        l.includes("Instruction: Close")
      )
        instrName = "close_game";
    }

    // Check if multi-instruction tx (Switchboard VRF init + our instruction)
    const isMulti = fee > 5000;

    stats.push({
      instruction: instrName,
      computeUnits: cu,
      fee,
      signature: sigInfo.signature,
      isMultiIx: isMulti,
    });
  }

  // Print raw data
  console.log("Raw Transaction Data:");
  console.log("-".repeat(90));
  console.log(
    "Instruction".padEnd(16) +
      "| CU".padStart(8) +
      " | Fee".padStart(10) +
      " | Multi-IX".padEnd(10) +
      " | Signature"
  );
  console.log("-".repeat(90));

  for (const s of stats) {
    console.log(
      s.instruction.padEnd(16) +
        "|" +
        String(s.computeUnits).padStart(7) +
        " |" +
        String(s.fee).padStart(9) +
        " |" +
        String(s.isMultiIx).padStart(9) +
        " | " +
        s.signature.slice(0, 20) +
        "..."
    );
  }

  // Aggregate by instruction type
  console.log("\n\nAggregated Cost by Instruction:");
  console.log("-".repeat(60));

  const byInstr = new Map<string, { cus: number[]; fees: number[] }>();
  for (const s of stats) {
    const key = s.instruction + (s.isMultiIx ? " (multi-ix)" : "");
    if (!byInstr.has(key)) byInstr.set(key, { cus: [], fees: [] });
    byInstr.get(key)!.cus.push(s.computeUnits);
    byInstr.get(key)!.fees.push(s.fee);
  }

  console.log(
    "Instruction".padEnd(24) +
      "| Avg CU".padStart(10) +
      " | Avg Fee".padStart(10) +
      " | Count"
  );
  console.log("-".repeat(60));

  for (const [instr, data] of byInstr) {
    const avgCu = Math.round(
      data.cus.reduce((a, b) => a + b, 0) / data.cus.length
    );
    const avgFee = Math.round(
      data.fees.reduce((a, b) => a + b, 0) / data.fees.length
    );
    console.log(
      instr.padEnd(24) +
        "|" +
        String(avgCu).padStart(9) +
        " |" +
        String(avgFee).padStart(9) +
        " | " +
        data.cus.length
    );
  }

  console.log("\n\nMarkdown Table for README:");
  console.log("| Instruction | Compute Units | Fee (lamports) | Notes |");
  console.log("|-------------|--------------:|---------------:|--------|");
  for (const [instr, data] of byInstr) {
    const avgCu = Math.round(
      data.cus.reduce((a, b) => a + b, 0) / data.cus.length
    );
    const avgFee = Math.round(
      data.fees.reduce((a, b) => a + b, 0) / data.fees.length
    );
    const note = instr.includes("multi-ix")
      ? "Includes Switchboard VRF instruction"
      : "Single instruction";
    console.log(`| ${instr} | ~${avgCu} | ${avgFee} | ${note} |`);
  }
}

main().catch(console.error);
