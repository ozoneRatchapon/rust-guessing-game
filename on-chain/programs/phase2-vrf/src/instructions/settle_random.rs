#[allow(unused_imports)]
use anchor_lang::Discriminator;
use anchor_lang::prelude::*;
use switchboard_on_demand::Discriminator as SbDiscriminator;
use switchboard_on_demand::on_demand::accounts::RandomnessAccountData;

use crate::error::GameError;
use crate::state::{GameV2, RandomnessSettled};

/// Field offsets within RandomnessAccountData struct (after 8-byte discriminator)
const REVEAL_SLOT_OFFSET: usize = 8 + 136;
const VALUE_OFFSET: usize = 8 + 144;
const VALUE_SIZE: usize = 32;

/// Read a u64 from raw bytes at the given offset (little-endian).
fn read_u64_at(data: &[u8], offset: usize) -> Option<u64> {
    if data.len() < offset + 8 {
        return None;
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// Read 32 random bytes from the randomness account data.
/// Validates discriminator, size, and that reveal_slot matches the clock.
fn read_randomness_value(data: &[u8], clock_slot: u64) -> Result<[u8; 32]> {
    // Check discriminator
    let discriminator = RandomnessAccountData::discriminator();
    if data.len() < discriminator.len() || data[..discriminator.len()] != *discriminator {
        return err!(GameError::InvalidRandomnessAccount);
    }

    // Check size
    if data.len() < RandomnessAccountData::size() {
        return err!(GameError::InvalidRandomnessAccount);
    }

    // Read reveal_slot and check freshness (must match current slot)
    let reveal_slot =
        read_u64_at(data, REVEAL_SLOT_OFFSET).ok_or(error!(GameError::InvalidRandomnessAccount))?;
    if clock_slot != reveal_slot {
        return err!(GameError::RandomnessNotAvailable);
    }

    // Read the 32-byte value
    let mut value = [0u8; 32];
    value.copy_from_slice(&data[VALUE_OFFSET..VALUE_OFFSET + VALUE_SIZE]);
    Ok(value)
}

#[derive(Accounts)]
pub struct SettleRandom<'info> {
    #[account(mut)]
    pub game: Account<'info, GameV2>,
    /// CHECK: Switchboard randomness account — validated by checking pubkey
    pub randomness_account: UncheckedAccount<'info>,
    pub admin: Signer<'info>,
}

pub fn settle_random_handler(ctx: Context<SettleRandom>) -> Result<()> {
    let game = &mut ctx.accounts.game;

    // Security: only admin can settle
    require!(
        ctx.accounts.admin.key() == game.admin,
        GameError::Unauthorized
    );

    // Security: not already settled
    require!(!game.is_revealed, GameError::AlreadyRevealed);

    // Security: game not finished
    require!(!game.is_finished, GameError::GameFinished);

    // Security: verify randomness account matches stored pubkey
    require!(
        ctx.accounts.randomness_account.key() == game.randomness_account,
        GameError::InvalidRandomnessAccount
    );

    // Read randomness value using manual byte extraction
    let clock = Clock::get()?;
    let value = read_randomness_value(&ctx.accounts.randomness_account.data.borrow(), clock.slot)?;

    // Derive secret number from first byte: 1-100
    let secret_number = (value[0] % 100) + 1;

    // Store hash of the VRF-derived secret for auditability
    let hash = blake3::hash(&secret_number.to_le_bytes());

    game.secret_number = secret_number;
    game.secret_hash = *hash.as_bytes();
    game.is_revealed = true;

    emit!(RandomnessSettled {
        secret_number,
        secret_hash: *hash.as_bytes(),
    });

    msg!("Randomness settled, secret number derived from VRF");
    // Don't log the actual number — it should be discovered by guessing!
    Ok(())
}
