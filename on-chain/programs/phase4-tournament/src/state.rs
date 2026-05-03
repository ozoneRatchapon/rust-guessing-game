use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Tournament {
    pub admin: Pubkey,
    pub secret_hash: [u8; 32],
    pub secret_number: u8,
    pub is_settled: bool,
    pub max_tries_per_player: u8,
    pub player_count: u8,
    pub max_players: u8,
    pub is_finished: bool,
    pub bump: u8,
    pub randomness_account: Pubkey,
    pub commit_slot: u64,
}

#[account]
#[derive(InitSpace)]
pub struct PlayerEntry {
    pub player: Pubkey,
    pub tournament: Pubkey,
    pub guess_count: u8,
    pub best_distance: u8, // |guess - secret|, lower is better; 0 = exact
    pub found_exact: bool,
    pub bump: u8,
}

#[event]
pub struct TournamentCreated {
    pub randomness_account: Pubkey,
    pub commit_slot: u64,
    pub max_players: u8,
}

#[event]
pub struct TournamentSettled {
    pub secret_hash: [u8; 32],
}

#[event]
pub struct PlayerJoined {
    pub player: Pubkey,
    pub player_count: u8,
}

#[event]
pub struct GuessResult {
    pub player: Pubkey,
    pub guess: u8,
    pub result: GuessOutcome,
    pub guess_count: u8,
    pub best_distance: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum GuessOutcome {
    TooSmall,
    TooBig,
    Correct,
}

#[event]
pub struct TournamentClosed {
    pub player_count: u8,
}
