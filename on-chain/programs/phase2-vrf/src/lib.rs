pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

// Use a new program ID — will be replaced by `anchor build` generated keypair
declare_id!("CHXkyr3GrLvWRXdbnYgPMKhwU1dYF6gW9aUpV8S3oTJw");

#[program]
pub mod phase2_vrf {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::initialize_handler(ctx)
    }

    pub fn settle_random(ctx: Context<SettleRandom>) -> Result<()> {
        instructions::settle_random::settle_random_handler(ctx)
    }

    pub fn guess(ctx: Context<Guess>, guess: u8) -> Result<()> {
        instructions::guess::guess_handler(ctx, guess)
    }

    pub fn close_game(ctx: Context<CloseGame>) -> Result<()> {
        instructions::close_game::close_game_handler(ctx)
    }
}
