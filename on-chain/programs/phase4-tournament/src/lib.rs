pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("FKqXgQYFUgMifKoQTYbb5UzMLry6RDo9E6dWm6E4fKoL");

#[program]
pub mod phase4_tournament {
    use super::*;

    pub fn create_tournament(ctx: Context<CreateTournament>) -> Result<()> {
        instructions::create_tournament::create_tournament_handler(ctx)
    }

    pub fn settle_tournament(ctx: Context<SettleTournament>) -> Result<()> {
        instructions::settle_tournament::settle_tournament_handler(ctx)
    }

    pub fn join_tournament(ctx: Context<JoinTournament>) -> Result<()> {
        instructions::join_tournament::join_tournament_handler(ctx)
    }

    pub fn submit_guess(ctx: Context<SubmitGuess>, guess: u8) -> Result<()> {
        instructions::submit_guess::submit_guess_handler(ctx, guess)
    }

    pub fn close_tournament(ctx: Context<CloseTournament>) -> Result<()> {
        instructions::close_tournament::close_tournament_handler(ctx)
    }
}
