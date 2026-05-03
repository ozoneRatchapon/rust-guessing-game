#[allow(unused_imports)]
use anchor_lang::Discriminator;
use anchor_lang::prelude::*;
use switchboard_on_demand::Discriminator as SbDiscriminator;
use switchboard_on_demand::on_demand::accounts::RandomnessAccountData;

use crate::error::TournamentError;
use crate::state::{Tournament, TournamentSettled};

/// Field offsets within RandomnessAccountData struct (after 8-byte discriminator)
const REVEAL_SLOT_OFFSET: usize = 8 + 136;
const VALUE_OFFSET: usize = 8 + 144;
const VALUE_SIZE: usize = 32;

/// Read a u64 from raw bytes at the given offset (little-endian).
fn read_u64_at(data: &[u8], offset: usize) -> Option<u64> {
    if data.len() < offset + 8 {
        return None;
    }
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// Read 32 random bytes from the randomness account data.
fn read_randomness_value(data: &[u8], clock_slot: u64) -> Result<[u8; 32]> {
    let discriminator = RandomnessAccountData::discriminator();
    if data.len() < discriminator.len() || data[..discriminator.len()] != *discriminator {
        return err!(TournamentError::InvalidRandomnessAccount);
    }
    if data.len() < RandomnessAccountData::size() {
        return err!(TournamentError::InvalidRandomnessAccount);
    }
    let reveal_slot = read_u64_at(data, REVEAL_SLOT_OFFSET)
        .ok_or(error!(TournamentError::InvalidRandomnessAccount))?;
    if clock_slot != reveal_slot {
        return err!(TournamentError::RandomnessNotAvailable);
    }
    let mut value = [0u8; 32];
    value.copy_from_slice(&data[VALUE_OFFSET..VALUE_OFFSET + VALUE_SIZE]);
    Ok(value)
}

#[derive(Accounts)]
pub struct SettleTournament<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Switchboard randomness account — validated by checking pubkey
    pub randomness_account: UncheckedAccount<'info>,
    pub admin: Signer<'info>,
}

pub fn settle_tournament_handler(ctx: Context<SettleTournament>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;

    // Security: only admin can settle
    require!(
        ctx.accounts.admin.key() == tournament.admin,
        TournamentError::Unauthorized
    );

    // Security: not already settled
    require!(!tournament.is_settled, TournamentError::AlreadySettled);

    // Security: tournament not finished
    require!(!tournament.is_finished, TournamentError::TournamentFinished);

    // Security: verify randomness account matches stored pubkey
    require!(
        ctx.accounts.randomness_account.key() == tournament.randomness_account,
        TournamentError::InvalidRandomnessAccount
    );

    // Read randomness value
    let clock = Clock::get()?;
    let value = read_randomness_value(&ctx.accounts.randomness_account.data.borrow(), clock.slot)?;

    // Derive secret number: 1-100
    let secret_number = (value[0] % 100) + 1;

    // Store blake3 hash for auditability
    let hash = blake3::hash(&secret_number.to_le_bytes());

    tournament.secret_number = secret_number;
    tournament.secret_hash = *hash.as_bytes();
    tournament.is_settled = true;

    emit!(TournamentSettled {
        secret_hash: *hash.as_bytes(),
    });

    msg!("Tournament settled — VRF randomness applied");
    Ok(())
}
