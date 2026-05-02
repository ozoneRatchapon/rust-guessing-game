use anchor_lang::prelude::*;

use crate::constants::*;
use crate::error::GameError;
use crate::state::Game;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + Game::INIT_SPACE,
        seeds = [b"game", admin.key().as_ref()],
        bump,
    )]
    pub game: Account<'info, Game>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(ctx: Context<Initialize>, secret_number: u8) -> Result<()> {
    require!(
        (MIN_NUMBER..=MAX_NUMBER).contains(&secret_number),
        GameError::InvalidSecretRange
    );

    let game = &mut ctx.accounts.game;
    let hash = blake3::hash(&secret_number.to_le_bytes());

    game.admin = ctx.accounts.admin.key();
    game.secret_hash = *hash.as_bytes();
    game.secret_number = 0;
    game.is_revealed = false;
    game.attempts = 0;
    game.max_tries = MAX_TRIES;
    game.is_finished = false;
    game.bump = ctx.bumps.game;

    msg!("Game initialized with secret hash");
    Ok(())
}
