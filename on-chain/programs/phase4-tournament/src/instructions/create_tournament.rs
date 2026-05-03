#[allow(unused_imports)]
use anchor_lang::Discriminator;
use anchor_lang::prelude::*;
use switchboard_on_demand::Discriminator as SbDiscriminator;
use switchboard_on_demand::on_demand::accounts::RandomnessAccountData;

use crate::constants::*;
use crate::error::TournamentError;
use crate::state::{Tournament, TournamentCreated};

/// Validate that the account data has the correct Switchboard randomness discriminator
/// and sufficient size.
fn validate_randomness_account(data: &[u8]) -> Result<()> {
    let discriminator = RandomnessAccountData::discriminator();
    if data.len() < discriminator.len() {
        return err!(TournamentError::InvalidRandomnessAccount);
    }
    if data[..discriminator.len()] != *discriminator {
        return err!(TournamentError::InvalidRandomnessAccount);
    }
    if data.len() < RandomnessAccountData::size() {
        return err!(TournamentError::InvalidRandomnessAccount);
    }
    Ok(())
}

#[derive(Accounts)]
pub struct CreateTournament<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + Tournament::INIT_SPACE,
        seeds = [b"tournament", admin.key().as_ref()],
        bump,
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Switchboard randomness account — validated by reading its data
    pub randomness_account: UncheckedAccount<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn create_tournament_handler(ctx: Context<CreateTournament>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let clock = Clock::get()?;

    // Validate randomness account has correct discriminator and size
    validate_randomness_account(&ctx.accounts.randomness_account.data.borrow())?;

    tournament.admin = ctx.accounts.admin.key();
    tournament.secret_hash = [0u8; 32];
    tournament.secret_number = 0;
    tournament.is_settled = false;
    tournament.max_tries_per_player = MAX_TRIES_PER_PLAYER;
    tournament.player_count = 0;
    tournament.max_players = MAX_PLAYERS;
    tournament.is_finished = false;
    tournament.bump = ctx.bumps.tournament;
    tournament.randomness_account = ctx.accounts.randomness_account.key();
    tournament.commit_slot = clock.slot;

    emit!(TournamentCreated {
        randomness_account: ctx.accounts.randomness_account.key(),
        commit_slot: clock.slot,
        max_players: MAX_PLAYERS,
    });

    msg!(
        "Tournament created — max {} players, VRF committed at slot {}",
        MAX_PLAYERS,
        clock.slot
    );
    Ok(())
}
