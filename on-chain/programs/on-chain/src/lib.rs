pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3FQq3uEM4wCzoGpxjQiYwyjjPjzbPpf98YSm2NbUuejT");

#[program]
pub mod on_chain {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, secret_number: u8) -> Result<()> {
        initialize::initialize_handler(ctx, secret_number)
    }

    pub fn reveal(ctx: Context<Reveal>, secret_number: u8) -> Result<()> {
        reveal::reveal_handler(ctx, secret_number)
    }

    pub fn guess(ctx: Context<Guess>, guess: u8) -> Result<()> {
        guess::guess_handler(ctx, guess)
    }
}
