use anchor_lang::prelude::*;

#[error_code]
pub enum GameError {
    #[msg("Only the game admin can perform this action")]
    Unauthorized,
    #[msg("Secret number must be between 1 and 100")]
    InvalidSecretRange,
    #[msg("Invalid hash - secret does not match committed hash")]
    HashMismatch,
    #[msg("Game has not been revealed yet")]
    NotRevealed,
    #[msg("Secret has already been revealed")]
    AlreadyRevealed,
    #[msg("Game is already finished")]
    GameFinished,
    #[msg("No more attempts remaining")]
    NoAttemptsRemaining,
}
