use anchor_lang::prelude::*;

use crate::constants::*;
use crate::error::GameError;
use crate::state::Game;

#[derive(Accounts)]
pub struct Reveal<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    pub admin: Signer<'info>,
}

pub fn reveal_handler(ctx: Context<Reveal>, secret_number: u8) -> Result<()> {
    let game = &mut ctx.accounts.game;

    require!(
        ctx.accounts.admin.key() == game.admin,
        GameError::Unauthorized
    );
    require!(
        (MIN_NUMBER..=MAX_NUMBER).contains(&secret_number),
        GameError::InvalidSecretRange
    );
    require!(!game.is_revealed, GameError::AlreadyRevealed);
    require!(!game.is_finished, GameError::GameFinished);

    let hash = blake3::hash(&secret_number.to_le_bytes());
    require!(
        hash.as_bytes() == &game.secret_hash,
        GameError::HashMismatch
    );

    game.secret_number = secret_number;
    game.is_revealed = true;

    msg!("Secret revealed: {secret_number}");
    Ok(())
}
