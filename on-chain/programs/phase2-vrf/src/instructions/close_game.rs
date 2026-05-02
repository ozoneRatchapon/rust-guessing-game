use anchor_lang::prelude::*;

use crate::error::GameError;
use crate::state::GameV2;

#[derive(Accounts)]
pub struct CloseGame<'info> {
    #[account(
        mut,
        close = admin,
        seeds = [b"game_v2", admin.key().as_ref()],
        bump,
    )]
    pub game: Account<'info, GameV2>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

pub fn close_game_handler(ctx: Context<CloseGame>) -> Result<()> {
    require!(
        ctx.accounts.game.admin == ctx.accounts.admin.key(),
        GameError::Unauthorized
    );

    msg!("Game account closed, rent recovered");
    Ok(())
}
