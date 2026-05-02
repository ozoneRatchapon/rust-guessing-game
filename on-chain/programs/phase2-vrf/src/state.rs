use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GameV2 {
    pub admin: Pubkey,
    pub secret_hash: [u8; 32],       // VRF commitment (stored after settle)
    pub secret_number: u8,            // Derived from VRF after settle
    pub is_revealed: bool,            // True after settle_random succeeds
    pub attempts: u8,
    pub max_tries: u8,
    pub is_finished: bool,
    pub bump: u8,
    pub randomness_account: Pubkey,   // Switchboard randomness PDA
    pub commit_slot: u64,             // Slot when randomness was committed
}

#[event]
pub struct GameInitialized {
    pub randomness_account: Pubkey,
    pub commit_slot: u64,
}

#[event]
pub struct RandomnessSettled {
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
