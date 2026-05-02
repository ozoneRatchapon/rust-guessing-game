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

// Switchboard on-demand devnet program ID
const SWITCHBOARD_DEVNET_PID: &str = "Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2";

// RandomnessAccountData discriminator from switchboard-on-demand 0.12.1
const RANDOMNESS_DISCRIMINATOR: [u8; 8] = [10, 66, 229, 135, 220, 239, 217, 114];

// Total size of RandomnessAccountData struct (repr(C), Pod)
// authority(32) + queue(32) + seed_slothash(32) + seed_slot(8) + oracle(32)
// + reveal_slot(8) + value(32) + _ebuf2(96) + _ebuf1(128) = 400 bytes
const RANDOMNESS_STRUCT_SIZE: usize = 400;
const RANDOMNESS_ACCOUNT_SIZE: usize = 8 + RANDOMNESS_STRUCT_SIZE; // 408

// Slot to warp to for settle_random (must match reveal_slot in fake data)
const SETTLE_SLOT: u64 = 200;

// ─── Helpers ───────────────────────────────────────────────────────────

fn setup_svm() -> (LiteSVM, Keypair, Pubkey) {
    let program_id = phase2_vrf::id();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/phase2_vrf.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 5_000_000_000).unwrap();
    (svm, admin, program_id)
}

fn get_game_pda(admin: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(&[b"game_v2", admin.as_ref()], program_id);
    pda
}

fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    let result = svm.send_transaction(tx);
    svm.expire_blockhash();
    result
}

/// Build fake Switchboard randomness account data with a known random value.
/// The secret number will be derived as `(value[0] % 100) + 1`.
/// `reveal_slot` is set to SETTLE_SLOT so `get_value(clock_slot)` succeeds when clock matches.
fn build_randomness_account_data(secret_value_byte: u8) -> Vec<u8> {
    let mut data = vec![0u8; RANDOMNESS_ACCOUNT_SIZE];

    // Write discriminator
    data[..8].copy_from_slice(&RANDOMNESS_DISCRIMINATOR);

    // Struct starts at offset 8
    let s = &mut data[8..];

    // authority: Pubkey (32 bytes) at offset 0
    // queue: Pubkey (32 bytes) at offset 32
    // seed_slothash: [u8; 32] at offset 64
    // seed_slot: u64 at offset 96
    // oracle: Pubkey (32 bytes) at offset 104
    // reveal_slot: u64 at offset 136
    // value: [u8; 32] at offset 144
    // _ebuf2: [u8; 96] at offset 176
    // _ebuf1: [u8; 128] at offset 272

    // Set reveal_slot = SETTLE_SLOT
    s[136..144].copy_from_slice(&SETTLE_SLOT.to_le_bytes());

    // Set value[0] to our chosen byte (secret = byte % 100 + 1)
    s[144] = secret_value_byte;

    data
}

/// Compute the expected secret number from a value byte: (byte % 100) + 1
fn expected_secret(value_byte: u8) -> u8 {
    (value_byte % 100) + 1
}

fn create_randomness_account_in_svm(svm: &mut LiteSVM, pubkey: &Pubkey, secret_value_byte: u8) {
    let switchboard_pid: Pubkey = SWITCHBOARD_DEVNET_PID.parse().unwrap();
    let data = build_randomness_account_data(secret_value_byte);
    let account = SolanaAccount {
        lamports: 1_000_000, // LiteSVM removes accounts with 0 lamports
        data,
        owner: switchboard_pid,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*pubkey, account).unwrap();
}

fn update_randomness_account_in_svm(svm: &mut LiteSVM, pubkey: &Pubkey, secret_value_byte: u8) {
    create_randomness_account_in_svm(svm, pubkey, secret_value_byte);
}

fn read_game(svm: &LiteSVM, game_pda: &Pubkey) -> phase2_vrf::state::GameV2 {
    let account = svm.get_account(game_pda).unwrap();
    let mut data: &[u8] = &account.data;
    phase2_vrf::state::GameV2::try_deserialize(&mut data).unwrap()
}

// ─── Instruction Builders ──────────────────────────────────────────────

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
/// Returns (svm, admin, player, program_id, game_pda, secret_number)
fn setup_full_game(secret_value_byte: u8) -> (LiteSVM, Keypair, Keypair, Pubkey, Pubkey, u8) {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();
    let secret = expected_secret(secret_value_byte);

    // 1. Create fake randomness account in SVM
    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    // 2. Initialize game
    let init_ix = build_initialize_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, init_ix, &admin).unwrap();

    // 3. Warp to SETTLE_SLOT so Clock::get().slot == reveal_slot
    svm.warp_to_slot(SETTLE_SLOT);

    // 4. Update randomness account data (reveal_slot now matches)
    // (Not strictly needed since we set it from the start, but ensures consistency)
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    // 5. Settle randomness
    let settle_ix = build_settle_random_ix(
        &program_id,
        &admin.pubkey(),
        &game_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, settle_ix, &admin).unwrap();

    // 6. Setup player
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();

    let game = read_game(&svm, &game_pda);
    assert!(game.is_revealed);
    assert_eq!(game.secret_number, secret);

    (svm, admin, player, program_id, game_pda, secret)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

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
