pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("EmYxqJbNaAFCMfGr6Xsf4LnJQVDxL4wQJgubor8JDAcF");

#[program]
pub mod broken_rand {
    use super::*;

    pub fn try_random(ctx: Context<TryRandom>) -> Result<()> {
        instructions::try_random::try_random_handler(ctx)
    }
}
