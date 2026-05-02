use anchor_lang::prelude::*;

#[error_code]
pub enum GameError {
    #[msg("Only the game admin can perform this action")]
    Unauthorized,
    #[msg("Secret number must be between 1 and 100")]
    InvalidSecretRange,
    #[msg("Randomness has not been settled yet")]
    NotRevealed,
    #[msg("Randomness has already been settled")]
    AlreadyRevealed,
    #[msg("Game is already finished")]
    GameFinished,
    #[msg("No more attempts remaining")]
    NoAttemptsRemaining,
    #[msg("Randomness value is not available yet")]
    RandomnessNotAvailable,
    #[msg("Invalid randomness account")]
    InvalidRandomnessAccount,
    #[msg("Guess must be between 1 and 100")]
    InvalidGuessRange,
}
