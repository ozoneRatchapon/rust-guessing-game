use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Game {
    pub admin: Pubkey,
    pub secret_hash: [u8; 32],
    pub secret_number: u8,
    pub is_revealed: bool,
    pub attempts: u8,
    pub max_tries: u8,
    pub is_finished: bool,
    pub bump: u8,
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
