use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GameV3 {
    pub admin: Pubkey,
    pub secret_hash: [u8; 32],
    pub secret_number: u8,
    pub is_revealed: bool,
    pub attempts: u8,
    pub max_tries: u8,
    pub is_finished: bool,
    pub bump: u8,
    pub vrf_request_pending: bool,
}

#[event]
pub struct GameInitialized {
    pub admin: Pubkey,
    pub client_seed: u8,
}

#[event]
pub struct RandomnessConsumed {
    pub secret_number: u8,
    pub secret_hash: [u8; 32],
}

#[event]
pub struct GuessTooSmall {
    pub guess: u8,
    pub attempts: u8,
}

#[event]
pub struct GuessTooBig {
    pub guess: u8,
    pub attempts: u8,
}

#[event]
pub struct GuessCorrect {
    pub guess: u8,
    pub attempts: u8,
}

#[event]
pub struct GameOver {
    pub attempts: u8,
    pub max_tries: u8,
}
