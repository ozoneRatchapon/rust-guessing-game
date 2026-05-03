use anchor_lang::prelude::*;

use crate::constants::*;
use crate::state::{GameInitialized, GameV3};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + GameV3::INIT_SPACE,
        seeds = [b"game_v3", admin.key().as_ref()],
        bump,
    )]
    pub game: Account<'info, GameV3>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(ctx: Context<Initialize>, _client_seed: u8) -> Result<()> {
    let game = &mut ctx.accounts.game;

    game.admin = ctx.accounts.admin.key();
    game.secret_hash = [0u8; 32];
    game.secret_number = 0;
    game.is_revealed = false;
    game.attempts = 0;
    game.max_tries = MAX_TRIES;
    game.is_finished = false;
    game.bump = ctx.bumps.game;
    game.vrf_request_pending = true;

    emit!(GameInitialized {
        admin: ctx.accounts.admin.key(),
        client_seed: _client_seed,
    });

    msg!("Game initialized, ready for VRF randomness request");
    Ok(())
}
