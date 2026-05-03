use anchor_lang::prelude::*;

use crate::error::TournamentError;
use crate::state::{PlayerEntry, PlayerJoined, Tournament};

#[derive(Accounts)]
pub struct JoinTournament<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    #[account(
        init,
        payer = player,
        space = 8 + PlayerEntry::INIT_SPACE,
        seeds = [b"player", tournament.key().as_ref(), player.key().as_ref()],
        bump,
    )]
    pub player_entry: Account<'info, PlayerEntry>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn join_tournament_handler(ctx: Context<JoinTournament>) -> Result<()> {
    // Capture keys before mutable borrows to satisfy borrow checker
    let tournament_key = ctx.accounts.tournament.key();
    let player_key = ctx.accounts.player.key();
    let player_entry_bump = ctx.bumps.player_entry;

    let tournament = &mut ctx.accounts.tournament;
    let player_entry = &mut ctx.accounts.player_entry;

    // Security: tournament not finished
    require!(!tournament.is_finished, TournamentError::TournamentFinished);

    // Security: tournament not full
    require!(
        tournament.player_count < tournament.max_players,
        TournamentError::TournamentFull
    );

    tournament.player_count += 1;

    player_entry.player = player_key;
    player_entry.tournament = tournament_key;
    player_entry.guess_count = 0;
    player_entry.best_distance = u8::MAX; // worst possible distance
    player_entry.found_exact = false;
    player_entry.bump = player_entry_bump;

    emit!(PlayerJoined {
        player: player_key,
        player_count: tournament.player_count,
    });

    msg!(
        "Player {} joined tournament ({} / {})",
        player_key,
        tournament.player_count,
        tournament.max_players
    );
    Ok(())
}
