use anchor_lang::prelude::*;
use switchboard_on_demand::Discriminator;
use switchboard_on_demand::on_demand::accounts::RandomnessAccountData;

use crate::constants::*;
use crate::error::GameError;
use crate::state::{GameInitialized, GameV2};

/// Validate that the account data has the correct Switchboard randomness discriminator
/// and sufficient size. Uses manual byte checks instead of `RandomnessAccountData::parse()`
/// to avoid `bytemuck` alignment issues in LiteSVM tests.
fn validate_randomness_account(data: &[u8]) -> Result<()> {
    let discriminator = RandomnessAccountData::discriminator();
    if data.len() < discriminator.len() {
        return err!(GameError::InvalidRandomnessAccount);
    }
    if data[..discriminator.len()] != *discriminator {
        return err!(GameError::InvalidRandomnessAccount);
    }
    if data.len() < RandomnessAccountData::size() {
        return err!(GameError::InvalidRandomnessAccount);
    }
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + GameV2::INIT_SPACE,
        seeds = [b"game_v2", admin.key().as_ref()],
        bump,
    )]
    pub game: Account<'info, GameV2>,
    /// CHECK: Switchboard randomness account — validated by reading its data
    pub randomness_account: UncheckedAccount<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(ctx: Context<Initialize>) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let clock = Clock::get()?;

    // Validate randomness account has correct discriminator and size
    validate_randomness_account(&ctx.accounts.randomness_account.data.borrow())?;

    game.admin = ctx.accounts.admin.key();
    game.secret_hash = [0u8; 32];
    game.secret_number = 0;
    game.is_revealed = false;
    game.attempts = 0;
    game.max_tries = MAX_TRIES;
    game.is_finished = false;
    game.bump = ctx.bumps.game;
    game.randomness_account = ctx.accounts.randomness_account.key();
    game.commit_slot = clock.slot;

    emit!(GameInitialized {
        randomness_account: ctx.accounts.randomness_account.key(),
        commit_slot: clock.slot,
    });

    msg!(
        "Game initialized with Switchboard VRF commitment at slot {}",
        clock.slot
    );
    Ok(())
}
