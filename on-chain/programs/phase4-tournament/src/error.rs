use anchor_lang::prelude::*;

#[error_code]
pub enum TournamentError {
    #[msg("Only the tournament admin can perform this action")]
    Unauthorized,
    #[msg("Tournament randomness has already been settled")]
    AlreadySettled,
    #[msg("Tournament randomness has not been settled yet")]
    NotSettled,
    #[msg("Tournament is already finished")]
    TournamentFinished,
    #[msg("Player has no attempts remaining")]
    NoAttemptsRemaining,
    #[msg("Guess must be between 1 and 100")]
    InvalidGuessRange,
    #[msg("Randomness value is not available yet")]
    RandomnessNotAvailable,
    #[msg("Invalid randomness account")]
    InvalidRandomnessAccount,
    #[msg("Tournament is full — no more players can join")]
    TournamentFull,
    #[msg("Player has already joined this tournament")]
    AlreadyJoined,
    #[msg("Player has not joined this tournament")]
    NotJoined,
}
