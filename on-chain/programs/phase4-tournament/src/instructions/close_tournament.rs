use anchor_lang::prelude::*;

use crate::error::TournamentError;
use crate::state::{Tournament, TournamentClosed};

#[derive(Accounts)]
pub struct CloseTournament<'info> {
    #[account(
        mut,
        close = admin,
        seeds = [b"tournament", admin.key().as_ref()],
        bump,
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

pub fn close_tournament_handler(ctx: Context<CloseTournament>) -> Result<()> {
    require!(
        ctx.accounts.tournament.admin == ctx.accounts.admin.key(),
        TournamentError::Unauthorized
    );

    let player_count = ctx.accounts.tournament.player_count;

    emit!(TournamentClosed { player_count });

    msg!("Tournament closed with {player_count} players — rent recovered");
    Ok(())
}
