pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("DnrNKTTspzjip8CAFXzCNkbMbQKXjNbZGnx6gNGtCEAH");

#[program]
pub mod phase3_magicblock_vrf {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, client_seed: u8) -> Result<()> {
        instructions::initialize::initialize_handler(ctx, client_seed)
    }

    pub fn request_randomness(ctx: Context<RequestRandomness>, client_seed: u8) -> Result<()> {
        instructions::request_randomness::request_randomness_handler(ctx, client_seed)
    }

    pub fn consume_randomness(ctx: Context<ConsumeRandomness>, randomness: [u8; 32]) -> Result<()> {
        instructions::consume_randomness::consume_randomness_handler(ctx, randomness)
    }

    pub fn guess(ctx: Context<Guess>, guess: u8) -> Result<()> {
        instructions::guess::guess_handler(ctx, guess)
    }

    pub fn close_game(ctx: Context<CloseGame>) -> Result<()> {
        instructions::close_game::close_game_handler(ctx)
    }
}
