// Phase 2 LiteSVM Tests — Switchboard VRF Guessing Game
//
// These tests run the actual compiled BPF program in-memory using LiteSVM.
// No network, no devnet, no TypeScript — just `cargo test`.
//
// Flow: init (commit VRF randomness) → settle_random (VRF reveals value) → player guesses
//
// The Switchboard VRF oracle is mocked by constructing fake RandomnessAccountData
// with the correct discriminator, a known value byte, and reveal_slot synced
// to svm.warp_to_slot(). This lets us test the full VRF flow offline.

use {
    anchor_lang::AccountDeserialize,
    litesvm::LiteSVM,
    solana_account::Account as SolanaAccount,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use anchor_lang::InstructionData;
use phase2_vrf::instruction::{CloseGame, Guess, Initialize, SettleRandom};

// ─── Constants ─────────────────────────────────────────────────────────

// Switchboard on-demand devnet program ID (used as the owner of fake randomness accounts)
const SWITCHBOARD_DEVNET_PID: &str = "Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2";

// RandomnessAccountData discriminator from switchboard-on-demand 0.12.1
// This is the first 8 bytes of the account — identifies it as a Switchboard randomness account
const RANDOMNESS_DISCRIMINATOR: [u8; 8] = [10, 66, 229, 135, 220, 239, 217, 114];

// Total size of RandomnessAccountData struct (repr(C), Pod)
// Fields: authority(32) + queue(32) + seed_slothash(32) + seed_slot(8) + oracle(32)
//         + reveal_slot(8) + value(32) + _ebuf2(96) + _ebuf1(128) = 400 bytes
// Plus 8-byte Anchor discriminator = 408 bytes total
const RANDOMNESS_STRUCT_SIZE: usize = 400;
const RANDOMNESS_ACCOUNT_SIZE: usize = 8 + RANDOMNESS_STRUCT_SIZE; // 408

// The slot we warp LiteSVM to for settle_random.
// Must match the reveal_slot written into the fake randomness account data.
// When the program calls Clock::get().slot, it gets this value.
const SETTLE_SLOT: u64 = 200;

// ─── Helpers ───────────────────────────────────────────────────────────

/// Create a fresh LiteSVM instance with the Phase 2 program loaded and admin funded.
fn setup_svm() -> (LiteSVM, Keypair, Pubkey) {
    let program_id = phase2_vrf::id();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();

    // Load the compiled BPF program into LiteSVM
    let bytes = include_bytes!("../../../target/deploy/phase2_vrf.so");
    svm.add_program(program_id, bytes).unwrap();

    // Fund admin with 5 SOL so they can pay for transactions
    svm.airdrop(&admin.pubkey(), 5_000_000_000).unwrap();

    (svm, admin, program_id)
}

/// Derive the game PDA from admin's pubkey: seeds = [b"game_v2", admin]
fn get_game_pda(admin: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(&[b"game_v2", admin.as_ref()], program_id);
    pda
}

/// Build a transaction with a single instruction, sign it, and send to LiteSVM.
/// Returns Ok if the instruction succeeded, Err if it failed.
fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata> {
    // Build message with recent blockhash
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);

    // Sign transaction with payer
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();

    // Send to LiteSVM and expire blockhash for next tx
    let result = svm.send_transaction(tx);
    svm.expire_blockhash();
    result
}

/// Build fake Switchboard randomness account data with a known random value.
///
/// Instead of calling the real Switchboard oracle, we construct the 408-byte account data
/// manually. This lets us test the VRF flow entirely offline.
///
/// Structure (after 8-byte discriminator):
///   authority(32) + queue(32) + seed_slothash(32) + seed_slot(8) + oracle(32)
///   + reveal_slot(8) + value(32) + _ebuf2(96) + _ebuf1(128) = 400 bytes
///
/// The secret number is derived as `(value[0] % 100) + 1`.
/// `reveal_slot` is set to SETTLE_SLOT so the freshness check passes when clock matches.
fn build_randomness_account_data(secret_value_byte: u8) -> Vec<u8> {
    let mut data = vec![0u8; RANDOMNESS_ACCOUNT_SIZE];

    // Step 1: Write the Switchboard discriminator (first 8 bytes)
    data[..8].copy_from_slice(&RANDOMNESS_DISCRIMINATOR);

    // Struct starts at offset 8 (after discriminator)
    let s = &mut data[8..];

    // Step 2: Set reveal_slot at struct offset 136 (= absolute offset 144)
    //         This must match the clock slot when settle_random is called
    s[136..144].copy_from_slice(&SETTLE_SLOT.to_le_bytes());

    // Step 3: Set value[0] at struct offset 144 (= absolute offset 152)
    //         The secret number will be (value[0] % 100) + 1
    s[144] = secret_value_byte;

    data
}

/// Compute the expected secret number from a value byte: (byte % 100) + 1
/// This must match the on-chain derivation in settle_random.rs.
fn expected_secret(value_byte: u8) -> u8 {
    (value_byte % 100) + 1
}

/// Create a fake Switchboard randomness account in LiteSVM.
/// This simulates what the Switchboard oracle would create on-chain.
/// Key: lamports must be > 0 (LiteSVM silently removes zero-lamport accounts).
fn create_randomness_account_in_svm(svm: &mut LiteSVM, pubkey: &Pubkey, secret_value_byte: u8) {
    let switchboard_pid: Pubkey = SWITCHBOARD_DEVNET_PID.parse().unwrap();
    let data = build_randomness_account_data(secret_value_byte);
    let account = SolanaAccount {
        lamports: 1_000_000, // LiteSVM removes accounts with 0 lamports
        data,
        owner: switchboard_pid, // Must be owned by Switchboard program
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*pubkey, account).unwrap();
}

/// Update existing randomness account data (e.g., after slot warp).
fn update_randomness_account_in_svm(svm: &mut LiteSVM, pubkey: &Pubkey, secret_value_byte: u8) {
    create_randomness_account_in_svm(svm, pubkey, secret_value_byte);
}

/// Read the game account from LiteSVM and deserialize it into the GameV2 struct.
fn read_game(svm: &LiteSVM, game_pda: &Pubkey) -> phase2_vrf::state::GameV2 {
    let account = svm.get_account(game_pda).unwrap();
    let mut data: &[u8] = &account.data;
    phase2_vrf::state::GameV2::try_deserialize(&mut data).unwrap()
}

// ─── Instruction Builders ──────────────────────────────────────────────

/// Build an `initialize` instruction: admin creates game with VRF randomness commitment.
/// Stores the randomness account pubkey for later verification in settle_random.
fn build_initialize_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    game_pda: &Pubkey,
    randomness_account: &Pubkey,
) -> Instruction {
    let system_program_id = Pubkey::from([0u8; 32]);
    Instruction::new_with_bytes(
        *program_id,
        &Initialize {}.data(),
        vec![
            AccountMeta::new(*game_pda, false),
            AccountMeta::new_readonly(*randomness_account, false),
            AccountMeta::new(*admin, true),
            AccountMeta::new_readonly(system_program_id, false),
        ],
    )
}

/// Build a `settle_random` instruction: admin settles the VRF randomness.
/// Program reads the randomness value, derives secret = (value[0] % 100) + 1,
/// and stores blake3(secret) as the hash.
fn build_settle_random_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    game_pda: &Pubkey,
    randomness_account: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &SettleRandom {}.data(),
        vec![
            AccountMeta::new(*game_pda, false),
            AccountMeta::new_readonly(*randomness_account, false),
            AccountMeta::new_readonly(*admin, true),
        ],
    )
}

/// Build a `guess` instruction: player submits a guess (1-100).
/// Program responds with too-small / too-big / correct.
/// Anyone can guess — no admin check.
fn build_guess_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    game_pda: &Pubkey,
    guess: u8,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &Guess { guess }.data(),
        vec![
            AccountMeta::new(*game_pda, false),
            AccountMeta::new_readonly(*player, true),
        ],
    )
}

/// Build a `close_game` instruction: admin closes game and recovers rent lamports.
fn build_close_game_ix(program_id: &Pubkey, admin: &Pubkey, game_pda: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &CloseGame {}.data(),
        vec![
            AccountMeta::new(*game_pda, false),
            AccountMeta::new(*admin, true),
        ],
    )
}

// ─── Full Game Setup Helper ────────────────────────────────────────────

/// Sets up a complete game ready for guessing.
/// This helper does the full init → settle flow so individual guess tests
/// can start from a game that already has a secret number.
///
/// Steps:
///   1. Create fake randomness account with known value byte
///   2. Initialize game (stores randomness pubkey)
///   3. Warp to SETTLE_SLOT so clock matches reveal_slot
///   4. Settle randomness (derives secret from VRF value)
///   5. Create and fund a player
///
/// Returns (svm, admin, player, program_id, game_pda, secret_number)
fn setup_full_game(secret_value_byte: u8) -> (LiteSVM, Keypair, Keypair, Pubkey, Pubkey, u8) {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();
    let secret = expected_secret(secret_value_byte);

    // Step 1: Create fake randomness account in SVM
    //         This simulates what the Switchboard oracle creates on-chain
    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    // Step 2: Initialize game — stores randomness pubkey as commitment
    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    // Step 3: Warp to SETTLE_SLOT so Clock::get().slot == reveal_slot
    //         Without this, the freshness check in settle_random would fail
    svm.warp_to_slot(SETTLE_SLOT);

    // Step 4: Re-create randomness account (ensures data is fresh after warp)
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    // Step 5: Settle randomness — reads value from fake account, derives secret
    let settle_ix = build_settle_random_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, settle_ix, &admin).unwrap();

    // Step 6: Create and fund player for guessing
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();

    // Verify game is in correct state
    let game = read_game(&svm, &game_pda);
    assert!(game.is_revealed);
    assert_eq!(game.secret_number, secret);

    (svm, admin, player, program_id, game_pda, secret)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

/// Test: Initialize a new game with VRF randomness commitment.
/// Verifies: admin stored, hash=0 (not settled yet), secret=0, randomness pubkey saved, defaults correct.
#[test]
fn test_initialize() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, init_ix, &admin);
    assert!(res.is_ok(), "Initialize should succeed");

    let game = read_game(&svm, &game_pda);
    assert_eq!(game.admin, admin.pubkey());
    assert_eq!(game.secret_hash, [0u8; 32]);
    assert_eq!(game.secret_number, 0);
    assert!(!game.is_revealed);
    assert_eq!(game.attempts, 0);
    assert_eq!(game.max_tries, 10);
    assert!(!game.is_finished);
    assert_eq!(game.randomness_account, randomness_keypair.pubkey());
}

/// Test: Settle randomness with value_byte=41 → secret=(41%100)+1=42.
/// Verifies: is_revealed=true, secret_number=42, blake3(42) hash stored correctly.
#[test]
fn test_settle_random() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();
    let secret_value_byte: u8 = 41; // secret = (41 % 100) + 1 = 42

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    // Warp to settle slot
    svm.warp_to_slot(SETTLE_SLOT);

    let settle_ix = build_settle_random_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_ok(), "Settle random should succeed");

    let game = read_game(&svm, &game_pda);
    assert!(game.is_revealed);
    assert_eq!(game.secret_number, 42);

    // Verify blake3 hash
    let expected_hash = blake3::hash(&42u8.to_le_bytes());
    assert_eq!(game.secret_hash, *expected_hash.as_bytes());
}

/// Test: Settle with various value bytes to ensure secret derivation is always in [1,100].
/// Tests bytes: 0, 1, 99, 100, 150, 200, 255 — covers edge cases around modulo.
#[test]
fn test_settle_random_boundary_values() {
    // Test multiple value bytes to ensure secret derivation is correct
    for value_byte in [0u8, 1, 99, 100, 150, 200, 255] {
        let (mut svm, admin, program_id) = setup_svm();
        let game_pda = get_game_pda(&admin.pubkey(), &program_id);
        let randomness_keypair = Keypair::new();
        let expected = expected_secret(value_byte);

        create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), value_byte);

        let init_ix = build_initialize_ix(
            &program_id,
            &admin.pubkey(),
            &game_pda,
            &randomness_keypair.pubkey(),
        );
        send_ix(&mut svm, init_ix, &admin).unwrap();

        svm.warp_to_slot(SETTLE_SLOT);

        let settle_ix = build_settle_random_ix(
            &program_id,
            &admin.pubkey(),
            &game_pda,
            &randomness_keypair.pubkey(),
        );
        send_ix(&mut svm, settle_ix, &admin).unwrap();

        let game = read_game(&svm, &game_pda);
        assert_eq!(
            game.secret_number, expected,
            "value_byte={value_byte} should produce secret={expected}"
        );
        assert!(
            (1..=100).contains(&game.secret_number),
            "secret must be in [1,100], got {}",
            game.secret_number
        );
    }
}

/// Test: Player guesses the exact secret (42) → game finished, 1 attempt.
#[test]
fn test_guess_correct() {
    // value_byte=41 → secret=(41%100)+1=42
    let (mut svm, _admin, player, program_id, game_pda, secret) = setup_full_game(41);
    assert_eq!(secret, 42);

    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_ok());

    let game = read_game(&svm, &game_pda);
    assert!(game.is_finished);
    assert_eq!(game.attempts, 1);
}

/// Test: Player guesses below the secret (10 < 50) → game continues, 1 attempt.
#[test]
fn test_guess_too_small() {
    // value_byte=149 → secret=(149%100)+1=50
    let (mut svm, _admin, player, program_id, game_pda, secret) = setup_full_game(149);
    assert_eq!(secret, 50);

    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 10);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_ok());

    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 1);
}

/// Test: Player guesses above the secret (90 > 50) → game continues, 1 attempt.
#[test]
fn test_guess_too_big() {
    // value_byte=149 → secret=(149%100)+1=50
    let (mut svm, _admin, player, program_id, game_pda, secret) = setup_full_game(149);
    assert_eq!(secret, 50);

    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 90);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_ok());

    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 1);
}

/// Test: 10 wrong guesses → game finished, 11th guess rejected.
#[test]
fn test_guess_game_over() {
    // value_byte=41 → secret=42, guess 1 ten times → game over
    let (mut svm, _admin, player, program_id, game_pda, _secret) = setup_full_game(41);

    for i in 0..10 {
        let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 1);
        let res = send_ix(&mut svm, guess_ix, &player);
        assert!(res.is_ok(), "Guess iteration {i} should succeed");
    }

    let game = read_game(&svm, &game_pda);
    assert!(game.is_finished);
    assert_eq!(game.attempts, 10);

    // 11th guess should fail
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 1);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err(), "11th guess should fail");
}

/// Test: Player guesses BEFORE settle_random → rejected (secret not determined yet).
#[test]
fn test_guess_before_settle_fails() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    // Try to guess WITHOUT settling randomness
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();

    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err(), "Guess before settle should fail");
}

/// Test: Non-admin tries to settle randomness → rejected.
#[test]
fn test_unauthorized_settle() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    svm.warp_to_slot(SETTLE_SLOT);

    // Impostor tries to settle
    let impostor = Keypair::new();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();

    let settle_ix = build_settle_random_ix(
        &program_id,
        &impostor.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &impostor);
    assert!(res.is_err(), "Non-admin settle should fail");
}

/// Test: Settle randomness twice → second settle rejected (AlreadyRevealed).
#[test]
fn test_double_settle_fails() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    svm.warp_to_slot(SETTLE_SLOT);

    // First settle succeeds
    let settle_ix = build_settle_random_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix.clone(), &admin);
    assert!(res.is_ok(), "First settle should succeed");

    // Second settle fails
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_err(), "Double settle should fail");
}

/// Test: Guess outside valid range (0 or 101) → rejected (InvalidGuessRange).
#[test]
fn test_invalid_guess_range() {
    let (mut svm, _admin, player, program_id, game_pda, _secret) = setup_full_game(41);

    // Guess 0 (below range)
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 0);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err(), "Guess 0 should fail (below range)");

    // Guess 101 (above range)
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 101);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err(), "Guess 101 should fail (above range)");
}

/// Test: Close game → account deleted, rent recovered to admin.
#[test]
fn test_close_game() {
    let (mut svm, admin, _player, program_id, game_pda, _secret) = setup_full_game(41);

    let admin_balance_before = svm.get_balance(&admin.pubkey());

    let close_ix = build_close_game_ix(&program_id, &admin.pubkey(), &game_pda);
    let res = send_ix(&mut svm, close_ix, &admin);
    assert!(res.is_ok(), "Close game should succeed");

    // Game account should be gone
    let account = svm.get_account(&game_pda);
    assert!(account.is_none(), "Game account should be closed");

    // Admin should have recovered rent (minus tx fee)
    let admin_balance_after = svm.get_balance(&admin.pubkey());
    assert!(
        admin_balance_after > admin_balance_before,
        "Admin should recover rent lamports"
    );
}

/// Test: Non-admin tries to close game → rejected (wrong PDA derivation).
#[test]
fn test_unauthorized_close() {
    let (mut svm, _admin, _player, program_id, _game_pda, _secret) = setup_full_game(41);

    let impostor = Keypair::new();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();

    // The close_game instruction uses PDA seeds with admin key, so impostor can't close it.
    // Using impostor as admin parameter derives a different PDA which doesn't have a game.
    let impostor_game_pda = get_game_pda(&impostor.pubkey(), &program_id);

    let close_ix = build_close_game_ix(&program_id, &impostor.pubkey(), &impostor_game_pda);
    let res = send_ix(&mut svm, close_ix, &impostor);
    assert!(res.is_err(), "Impostor close should fail");
}

/// Test: Settle with a different randomness account than stored during init → rejected.
#[test]
fn test_wrong_randomness_account_fails() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();
    let fake_randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);
    create_randomness_account_in_svm(&mut svm, &fake_randomness_keypair.pubkey(), 41);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    svm.warp_to_slot(SETTLE_SLOT);

    // Try to settle with wrong randomness account
    let settle_ix = build_settle_random_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &fake_randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_err(), "Wrong randomness account should fail");
}

/// Test: Settle WITHOUT warping to SETTLE_SLOT → rejected (clock slot ≠ reveal_slot).
#[test]
fn test_randomness_not_ready_fails() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    // Don't warp — clock slot won't match reveal_slot
    // The randomness account has reveal_slot=SETTLE_SLOT(200)
    // but svm is at slot ~0

    let settle_ix = build_settle_random_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_err(), "Settle without slot match should fail");
}

/// Test: Complete game session — init → settle → 3 wrong guesses → win → close.
/// Simulates a real player experience end-to-end.
#[test]
fn test_full_game_session() {
    // Simulate a complete game: init → settle → multiple guesses → win → close
    let (mut svm, admin, player, program_id, game_pda, secret) = setup_full_game(99);
    // value_byte=99 → secret=(99%100)+1=100
    assert_eq!(secret, 100);

    // Guess 50 → too small
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 50);
    send_ix(&mut svm, guess_ix, &player).unwrap();
    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 1);

    // Guess 75 → too small
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 75);
    send_ix(&mut svm, guess_ix, &player).unwrap();
    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 2);

    // Guess 90 → too small
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 90);
    send_ix(&mut svm, guess_ix, &player).unwrap();
    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 3);

    // Guess 100 → correct!
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 100);
    send_ix(&mut svm, guess_ix, &player).unwrap();
    let game = read_game(&svm, &game_pda);
    assert!(game.is_finished);
    assert_eq!(game.attempts, 4);

    // Guess after finish should fail
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 50);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err(), "Guess after finish should fail");

    // Close game
    let close_ix = build_close_game_ix(&program_id, &admin.pubkey(), &game_pda);
    let res = send_ix(&mut svm, close_ix, &admin);
    assert!(res.is_ok(), "Close should succeed");
}
