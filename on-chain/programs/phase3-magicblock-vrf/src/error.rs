use anchor_lang::prelude::*;

#[error_code]
pub enum GameError {
    #[msg("Only the game admin can perform this action")]
    Unauthorized,
    #[msg("Secret number must be between 1 and 100")]
    InvalidSecretRange,
    #[msg("Randomness has not been consumed yet")]
    NotRevealed,
    #[msg("Randomness has already been consumed")]
    AlreadyRevealed,
    #[msg("Game is already finished")]
    GameFinished,
    #[msg("No more attempts remaining")]
    NoAttemptsRemaining,
    #[msg("Guess must be between 1 and 100")]
    InvalidGuessRange,
    #[msg("VRF request is not pending")]
    VrfRequestNotPending,
    #[msg("Invalid VRF program identity")]
    InvalidVrfIdentity,
}
