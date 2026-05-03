use anchor_lang::prelude::*;

use crate::constants::*;
use crate::error::TournamentError;
use crate::state::{GuessOutcome, GuessResult, PlayerEntry, Tournament};

#[derive(Accounts)]
pub struct SubmitGuess<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    #[account(
        mut,
        seeds = [b"player", tournament.key().as_ref(), player.key().as_ref()],
        bump = player_entry.bump,
    )]
    pub player_entry: Account<'info, PlayerEntry>,
    pub player: Signer<'info>,
}

pub fn submit_guess_handler(ctx: Context<SubmitGuess>, guess: u8) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player_entry = &mut ctx.accounts.player_entry;

    // Security: player must be the entry owner
    require!(
        ctx.accounts.player.key() == player_entry.player,
        TournamentError::NotJoined
    );

    // Security: tournament must be settled (secret number available)
    require!(tournament.is_settled, TournamentError::NotSettled);

    // Security: tournament not finished
    require!(!tournament.is_finished, TournamentError::TournamentFinished);

    // Security: valid guess range
    require!(
        (MIN_NUMBER..=MAX_NUMBER).contains(&guess),
        TournamentError::InvalidGuessRange
    );

    // Security: player has attempts remaining
    require!(
        player_entry.guess_count < tournament.max_tries_per_player,
        TournamentError::NoAttemptsRemaining
    );

    player_entry.guess_count += 1;

    // Compute distance from secret
    let distance = if guess > tournament.secret_number {
        guess - tournament.secret_number
    } else {
        tournament.secret_number - guess
    };

    // Track best distance (only improves if this guess is closer)
    if distance < player_entry.best_distance {
        player_entry.best_distance = distance;
    }

    let outcome = match guess.cmp(&tournament.secret_number) {
        std::cmp::Ordering::Equal => {
            player_entry.found_exact = true;
            GuessOutcome::Correct
        }
        std::cmp::Ordering::Less => GuessOutcome::TooSmall,
        std::cmp::Ordering::Greater => GuessOutcome::TooBig,
    };

    emit!(GuessResult {
        player: ctx.accounts.player.key(),
        guess,
        result: outcome.clone(),
        guess_count: player_entry.guess_count,
        best_distance: player_entry.best_distance,
    });

    match &outcome {
        GuessOutcome::Correct => {
            msg!(
                "Player {} guessed correctly in {} attempts!",
                ctx.accounts.player.key(),
                player_entry.guess_count
            );
        }
        GuessOutcome::TooSmall => {
            msg!(
                "Player {} guessed {} — too small!",
                ctx.accounts.player.key(),
                guess
            );
        }
        GuessOutcome::TooBig => {
            msg!(
                "Player {} guessed {} — too big!",
                ctx.accounts.player.key(),
                guess
            );
        }
    }

    Ok(())
}
