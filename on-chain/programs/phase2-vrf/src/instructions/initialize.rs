use anchor_lang::prelude::*;
use switchboard_on_demand::on_demand::accounts::RandomnessAccountData;

use crate::constants::*;
use crate::error::GameError;
use crate::state::{GameInitialized, GameV2};

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

    // Read randomness account to get the commitment slot
    // The randomness account is created by the client before this instruction
    // and committed in the same transaction
    let _randomness_data =
        RandomnessAccountData::parse(ctx.accounts.randomness_account.data.borrow())
            .map_err(|_| GameError::InvalidRandomnessAccount)?;

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
