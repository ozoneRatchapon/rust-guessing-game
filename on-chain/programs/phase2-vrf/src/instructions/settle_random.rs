use anchor_lang::prelude::*;
use switchboard_on_demand::on_demand::accounts::RandomnessAccountData;

use crate::error::GameError;
use crate::state::{GameV2, RandomnessSettled};

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

    // Read randomness value
    let randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account.data.borrow())
            .map_err(|_| GameError::InvalidRandomnessAccount)?;

    // Get the random value — takes slot u64, returns Result<[u8; 32], OnDemandError>
    let clock = Clock::get()?;
    let value = randomness_data
        .get_value(clock.slot)
        .map_err(|_| GameError::RandomnessNotAvailable)?;

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
