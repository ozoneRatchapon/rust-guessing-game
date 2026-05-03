use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::system_program;
use borsh::BorshSerialize;

use crate::constants::*;

/// Accounts for requesting VRF randomness.
/// Manually constructs the CPI instruction instead of using ephemeral-vrf-sdk
/// (which is incompatible with Anchor 1.0.1's Pubkey type).
#[derive(Accounts)]
pub struct RequestRandomness<'info> {
    /// CHECK: The VRF program identity PDA derived from seeds [b"identity"].
    /// Used as a signing PDA for the CPI to the VRF program.
    #[account(
        seeds = [IDENTITY_SEED],
        bump,
        seeds::program = vrf_program,
    )]
    pub program_identity: UncheckedAccount<'info>,
    /// CHECK: The oracle queue (validated by address)
    #[account(mut, address = DEFAULT_QUEUE.parse::<Pubkey>().unwrap())]
    pub oracle_queue: UncheckedAccount<'info>,
    /// CHECK: Slot hashes sysvar — needed by VRF program for seed derivation
    pub slot_hashes: UncheckedAccount<'info>,
    /// CHECK: MagicBlock VRF program
    pub vrf_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

/// Inlined request data structure from ephemeral-vrf-sdk.
/// Borsh-serialized after the 8-byte discriminator.
#[derive(BorshSerialize)]
struct RequestRandomnessData {
    caller_seed: [u8; 32],
    callback_program_id: Pubkey,
    callback_discriminator: Vec<u8>,
    callback_accounts_metas: Vec<InlinedAccountMeta>,
    callback_args: Vec<u8>,
}

/// Inlined from ephemeral-vrf-sdk SerializableAccountMeta.
#[derive(BorshSerialize)]
struct InlinedAccountMeta {
    pubkey: Pubkey,
    is_signer: bool,
    is_writable: bool,
}

/// Anchor discriminator for the consume_randomness instruction.
/// Computed as: sha256("global:consume_randomness")[..8]
/// = [190, 217, 49, 162, 99, 26, 73, 234]
const CONSUME_RANDOMNESS_DISCRIMINATOR: [u8; 8] = [190, 217, 49, 162, 99, 26, 73, 234];

pub fn request_randomness_handler(ctx: Context<RequestRandomness>, client_seed: u8) -> Result<()> {
    let vrf_program_id = VRF_PROGRAM_ID.parse::<Pubkey>().unwrap();

    // Build caller_seed from client_seed (padded to 32 bytes)
    let mut caller_seed = [0u8; 32];
    caller_seed[0] = client_seed;

    let request_data = RequestRandomnessData {
        caller_seed,
        callback_program_id: crate::ID,
        callback_discriminator: CONSUME_RANDOMNESS_DISCRIMINATOR.to_vec(),
        callback_accounts_metas: vec![],
        callback_args: vec![],
    };

    // Serialize: 8-byte discriminator + borsh-encoded data
    let mut data = REQUEST_RANDOMNESS_DISCRIMINATOR.to_vec();
    request_data.serialize(&mut data)?;

    let identity_bump = ctx.bumps.program_identity;

    // Slot hashes sysvar ID: SysvarS1otHashes11111111111111111111111111111111111111111
    let slot_hashes_id: Pubkey = "SysvarS1otHashes11111111111111111111111111111111111111111"
        .parse()
        .unwrap();

    let ix = Instruction {
        program_id: vrf_program_id,
        accounts: vec![
            AccountMeta::new(ctx.accounts.payer.key(), true),
            AccountMeta::new_readonly(ctx.accounts.program_identity.key(), true),
            AccountMeta::new(ctx.accounts.oracle_queue.key(), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(slot_hashes_id, false),
        ],
        data,
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.program_identity.to_account_info(),
            ctx.accounts.oracle_queue.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.slot_hashes.to_account_info(),
            ctx.accounts.vrf_program.to_account_info(),
        ],
        &[&[IDENTITY_SEED, &[identity_bump]]],
    )?;

    msg!("VRF randomness request submitted with seed {client_seed}");
    Ok(())
}
