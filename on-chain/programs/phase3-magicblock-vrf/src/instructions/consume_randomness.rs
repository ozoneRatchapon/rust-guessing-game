use anchor_lang::prelude::*;

use crate::constants::*;
use crate::error::GameError;
use crate::state::{GameV3, RandomnessConsumed};

#[derive(Accounts)]
pub struct ConsumeRandomness<'info> {
    /// VRF program identity PDA — ensures only MagicBlock VRF can call this.
    /// The VRF program signs with this PDA when invoking the callback.
    /// CHECK: Validated by address check against the known VRF identity.
    #[account(address = VRF_PROGRAM_IDENTITY.parse::<Pubkey>().unwrap())]
    pub vrf_program_identity: Signer<'info>,
    #[account(mut)]
    pub game: Account<'info, GameV3>,
}

/// Generates a random u8 value within a specified range from a 32-byte VRF seed.
///
/// Inlined from ephemeral-vrf-sdk `random_u8_with_range`.
/// Uses rejection sampling to avoid modulo bias:
/// scans through the 32 bytes looking for a value that, when mapped,
/// gives an unbiased result. Falls back to the last byte if none qualify.
fn random_u8_with_range(bytes: &[u8; 32], min_value: u8, max_value: u8) -> u8 {
    let range = (max_value - min_value + 1) as u16;
    let threshold = (256 / range * range) as u8;

    for &b in bytes.iter().rev() {
        if b < threshold {
            return min_value + (b % range as u8);
        }
    }
    // Fallback (slight bias, but rare — only when all 32 bytes >= threshold)
    min_value + (bytes[31] % range as u8)
}

pub fn consume_randomness_handler(
    ctx: Context<ConsumeRandomness>,
    randomness: [u8; 32],
) -> Result<()> {
    let game = &mut ctx.accounts.game;

    require!(!game.is_revealed, GameError::AlreadyRevealed);
    require!(!game.is_finished, GameError::GameFinished);

    // Derive secret number in [1, 100] using unbiased range mapping
    let secret_number = random_u8_with_range(&randomness, MIN_NUMBER, MAX_NUMBER);

    // Store blake3 hash as an audit trail
    let hash = blake3::hash(&secret_number.to_le_bytes());

    game.secret_number = secret_number;
    game.secret_hash = *hash.as_bytes();
    game.is_revealed = true;
    game.vrf_request_pending = false;

    emit!(RandomnessConsumed {
        secret_number,
        secret_hash: *hash.as_bytes(),
    });

    msg!("VRF randomness consumed, secret derived from MagicBlock oracle");
    Ok(())
}
