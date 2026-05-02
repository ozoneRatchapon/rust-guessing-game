use anchor_lang::prelude::*;
use rand::Rng;

#[derive(Accounts)]
pub struct TryRandom<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
}

pub fn try_random_handler(_ctx: Context<TryRandom>) -> Result<()> {
    let secret: u8 = rand::thread_rng().gen_range(1..=100);
    msg!("Random number: {}", secret);
    Ok(())
}
