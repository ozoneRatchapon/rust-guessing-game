use anchor_lang::prelude::*;

use crate::error::GameError;
use crate::state::{Game, GameOver, GuessCorrect, GuessTooBig, GuessTooSmall};

#[derive(Accounts)]
pub struct Guess<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    pub player: Signer<'info>,
}

pub fn guess_handler(ctx: Context<Guess>, guess: u8) -> Result<()> {
    let game = &mut ctx.accounts.game;

    require!(!game.is_finished, GameError::GameFinished);
    require!(game.is_revealed, GameError::NotRevealed);
    require!(
        game.attempts < game.max_tries,
        GameError::NoAttemptsRemaining
    );

    game.attempts += 1;

    match guess.cmp(&game.secret_number) {
        std::cmp::Ordering::Equal => {
            game.is_finished = true;
            emit!(GuessCorrect {
                guess,
                attempts: game.attempts,
            });
            msg!(
                "Correct! You guessed the number in {} attempts",
                game.attempts
            );
        }
        std::cmp::Ordering::Less => {
            emit!(GuessTooSmall {
                guess,
                attempts: game.attempts,
            });
            msg!("Guess {guess} is too small!");
        }
        std::cmp::Ordering::Greater => {
            emit!(GuessTooBig {
                guess,
                attempts: game.attempts,
            });
            msg!("Guess {guess} is too big!");
        }
    }

    if game.attempts >= game.max_tries && !game.is_finished {
        game.is_finished = true;
        emit!(GameOver {
            attempts: game.attempts,
            max_tries: game.max_tries,
        });
        msg!("Game over! No more attempts remaining");
    }

    Ok(())
}
