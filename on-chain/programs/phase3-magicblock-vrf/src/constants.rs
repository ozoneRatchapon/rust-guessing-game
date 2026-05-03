pub const MAX_TRIES: u8 = 10;
pub const MIN_NUMBER: u8 = 1;
pub const MAX_NUMBER: u8 = 100;

// MagicBlock VRF constants (inlined from ephemeral-vrf-sdk v0.2.3)
// See: https://github.com/magicblock-labs/ephemeral-vrf

/// MagicBlock VRF program ID on mainnet/devnet
pub const VRF_PROGRAM_ID: &str = "Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz";

/// Default oracle queue for randomness requests
pub const DEFAULT_QUEUE: &str = "Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh";

/// VRF program identity PDA (signer on consume_randomness callback)
pub const VRF_PROGRAM_IDENTITY: &str = "9irBy75QS2BN81FUgXuHcjqceJJRuc9oDkAe8TKVvvAw";

/// Seed for the VRF identity PDA
pub const IDENTITY_SEED: &[u8] = b"identity";

/// RequestRandomness instruction discriminator (borsh enum index 3)
pub const REQUEST_RANDOMNESS_DISCRIMINATOR: [u8; 8] = [3, 0, 0, 0, 0, 0, 0, 0];
