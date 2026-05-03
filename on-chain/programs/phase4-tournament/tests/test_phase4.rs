// Phase 4 LiteSVM Tests — Multi-Player Tournament Guessing Game
//
// Tests run the actual compiled BPF program in-memory using LiteSVM.
// No network, no devnet — just `cargo test`.
//
// Flow:
//   create_tournament (commit VRF randomness)
//   → join_tournament (players enter)
//   → settle_tournament (VRF reveals secret)
//   → submit_guess (players compete)
//   → close_tournament (admin closes)

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
use phase4_tournament::instruction::{
    CloseTournament, CreateTournament, JoinTournament, SettleTournament, SubmitGuess,
};

// ─── Constants ─────────────────────────────────────────────────────────

const SWITCHBOARD_DEVNET_PID: &str = "Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2";
const RANDOMNESS_DISCRIMINATOR: [u8; 8] = [10, 66, 229, 135, 220, 239, 217, 114];
const RANDOMNESS_STRUCT_SIZE: usize = 400;
const RANDOMNESS_ACCOUNT_SIZE: usize = 8 + RANDOMNESS_STRUCT_SIZE; // 408
const SETTLE_SLOT: u64 = 200;

// ─── Helpers ───────────────────────────────────────────────────────────

fn setup_svm() -> (LiteSVM, Keypair, Pubkey) {
    let program_id = phase4_tournament::id();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();

    let bytes = include_bytes!("../../../target/deploy/phase4_tournament.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    (svm, admin, program_id)
}

fn get_tournament_pda(admin: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(&[b"tournament", admin.as_ref()], program_id);
    pda
}

fn get_player_pda(tournament: &Pubkey, player: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(
        &[b"player", tournament.as_ref(), player.as_ref()],
        program_id,
    );
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

fn build_randomness_account_data(secret_value_byte: u8) -> Vec<u8> {
    let mut data = vec![0u8; RANDOMNESS_ACCOUNT_SIZE];
    data[..8].copy_from_slice(&RANDOMNESS_DISCRIMINATOR);
    let s = &mut data[8..];
    s[136..144].copy_from_slice(&SETTLE_SLOT.to_le_bytes());
    s[144] = secret_value_byte;
    data
}

fn expected_secret(value_byte: u8) -> u8 {
    (value_byte % 100) + 1
}

fn create_randomness_account_in_svm(svm: &mut LiteSVM, pubkey: &Pubkey, secret_value_byte: u8) {
    let switchboard_pid: Pubkey = SWITCHBOARD_DEVNET_PID.parse().unwrap();
    let data = build_randomness_account_data(secret_value_byte);
    let account = SolanaAccount {
        lamports: 1_000_000,
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

fn read_tournament(svm: &LiteSVM, pda: &Pubkey) -> phase4_tournament::state::Tournament {
    let account = svm.get_account(pda).unwrap();
    let mut data: &[u8] = &account.data;
    phase4_tournament::state::Tournament::try_deserialize(&mut data).unwrap()
}

fn read_player_entry(svm: &LiteSVM, pda: &Pubkey) -> phase4_tournament::state::PlayerEntry {
    let account = svm.get_account(pda).unwrap();
    let mut data: &[u8] = &account.data;
    phase4_tournament::state::PlayerEntry::try_deserialize(&mut data).unwrap()
}

fn fund_player(svm: &mut LiteSVM) -> Keypair {
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 2_000_000_000).unwrap();
    player
}

// ─── Instruction Builders ──────────────────────────────────────────────

fn build_create_tournament_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    tournament_pda: &Pubkey,
    randomness_account: &Pubkey,
) -> Instruction {
    let system_program_id = Pubkey::from([0u8; 32]);
    Instruction::new_with_bytes(
        *program_id,
        &CreateTournament {}.data(),
        vec![
            AccountMeta::new(*tournament_pda, false),
            AccountMeta::new_readonly(*randomness_account, false),
            AccountMeta::new(*admin, true),
            AccountMeta::new_readonly(system_program_id, false),
        ],
    )
}

fn build_settle_tournament_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    tournament_pda: &Pubkey,
    randomness_account: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &SettleTournament {}.data(),
        vec![
            AccountMeta::new(*tournament_pda, false),
            AccountMeta::new_readonly(*randomness_account, false),
            AccountMeta::new_readonly(*admin, true),
        ],
    )
}

fn build_join_tournament_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    tournament_pda: &Pubkey,
    player_entry_pda: &Pubkey,
) -> Instruction {
    let system_program_id = Pubkey::from([0u8; 32]);
    Instruction::new_with_bytes(
        *program_id,
        &JoinTournament {}.data(),
        vec![
            AccountMeta::new(*tournament_pda, false),
            AccountMeta::new(*player_entry_pda, false),
            AccountMeta::new(*player, true),
            AccountMeta::new_readonly(system_program_id, false),
        ],
    )
}

fn build_submit_guess_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    tournament_pda: &Pubkey,
    player_entry_pda: &Pubkey,
    guess: u8,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &SubmitGuess { guess }.data(),
        vec![
            AccountMeta::new(*tournament_pda, false),
            AccountMeta::new(*player_entry_pda, false),
            AccountMeta::new_readonly(*player, true),
        ],
    )
}

fn build_close_tournament_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    tournament_pda: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &CloseTournament {}.data(),
        vec![
            AccountMeta::new(*tournament_pda, false),
            AccountMeta::new(*admin, true),
        ],
    )
}

// ─── Full Setup Helper ────────────────────────────────────────────────

/// Sets up a complete tournament ready for guessing.
/// Returns (svm, admin, players, program_id, tournament_pda, randomness_pubkey, secret)
fn setup_full_tournament(
    secret_value_byte: u8,
    num_players: usize,
) -> (LiteSVM, Keypair, Vec<Keypair>, Pubkey, Pubkey, Pubkey, u8) {
    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();
    let secret = expected_secret(secret_value_byte);

    // Step 1: Create fake randomness account
    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    // Step 2: Create tournament
    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Step 3: Join players
    let mut players = Vec::new();
    for _ in 0..num_players {
        let player = fund_player(&mut svm);
        let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);
        let join_ix =
            build_join_tournament_ix(&program_id, &player.pubkey(), &tournament_pda, &player_pda);
        send_ix(&mut svm, join_ix, &player).unwrap();
        players.push(player);
    }

    // Step 4: Warp to settle slot and settle
    svm.warp_to_slot(SETTLE_SLOT);
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), secret_value_byte);

    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, settle_ix, &admin).unwrap();

    // Verify state
    let tournament = read_tournament(&svm, &tournament_pda);
    assert!(tournament.is_settled);
    assert_eq!(tournament.secret_number, secret);
    assert_eq!(tournament.player_count, num_players as u8);

    (
        svm,
        admin,
        players,
        program_id,
        tournament_pda,
        randomness_keypair.pubkey(),
        secret,
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_create_tournament() {
    eprintln!("\n━━━ test_create_tournament ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, ix, &admin);
    assert!(res.is_ok(), "Create tournament should succeed");

    let tournament = read_tournament(&svm, &tournament_pda);
    assert_eq!(tournament.admin, admin.pubkey());
    assert_eq!(tournament.secret_hash, [0u8; 32]);
    assert_eq!(tournament.secret_number, 0);
    assert!(!tournament.is_settled);
    assert_eq!(tournament.max_tries_per_player, 10);
    assert_eq!(tournament.player_count, 0);
    assert_eq!(tournament.max_players, 16);
    assert!(!tournament.is_finished);
    assert_eq!(tournament.randomness_account, randomness_keypair.pubkey());

    eprintln!("  ✓ test_create_tournament passed");
}

#[test]
fn test_settle_tournament() {
    eprintln!("\n━━━ test_settle_tournament ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Warp to settle slot
    svm.warp_to_slot(SETTLE_SLOT);
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_ok(), "Settle should succeed");

    let tournament = read_tournament(&svm, &tournament_pda);
    assert!(tournament.is_settled);
    assert_eq!(tournament.secret_number, expected_secret(41)); // (41 % 100) + 1 = 42
    assert_ne!(tournament.secret_hash, [0u8; 32]);

    eprintln!("  ✓ test_settle_tournament passed");
}

#[test]
fn test_join_tournament() {
    eprintln!("\n━━━ test_join_tournament ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 50);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Join a player
    let player = fund_player(&mut svm);
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    let join_ix =
        build_join_tournament_ix(&program_id, &player.pubkey(), &tournament_pda, &player_pda);
    let res = send_ix(&mut svm, join_ix, &player);
    assert!(res.is_ok(), "Join should succeed");

    // Verify tournament state
    let tournament = read_tournament(&svm, &tournament_pda);
    assert_eq!(tournament.player_count, 1);

    // Verify player entry
    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.player, player.pubkey());
    assert_eq!(entry.tournament, tournament_pda);
    assert_eq!(entry.guess_count, 0);
    assert_eq!(entry.best_distance, u8::MAX);
    assert!(!entry.found_exact);

    eprintln!("  ✓ test_join_tournament passed");
}

#[test]
fn test_multiple_players_join() {
    eprintln!("\n━━━ test_multiple_players_join ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 50);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Join 5 players
    for _ in 0..5 {
        let player = fund_player(&mut svm);
        let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);
        let join_ix =
            build_join_tournament_ix(&program_id, &player.pubkey(), &tournament_pda, &player_pda);
        send_ix(&mut svm, join_ix, &player).unwrap();
    }

    let tournament = read_tournament(&svm, &tournament_pda);
    assert_eq!(tournament.player_count, 5);

    eprintln!("  ✓ test_multiple_players_join passed");
}

#[test]
fn test_submit_guess_correct() {
    eprintln!("\n━━━ test_submit_guess_correct ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(41, 1); // secret = 42

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        secret, // guess the exact number
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_ok(), "Guess should succeed");

    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.guess_count, 1);
    assert_eq!(entry.best_distance, 0);
    assert!(entry.found_exact);

    eprintln!("  ✓ test_submit_guess_correct passed (secret={secret}, guessed in 1 attempt)");
}

#[test]
fn test_submit_guess_too_small() {
    eprintln!("\n━━━ test_submit_guess_too_small ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(41, 1); // secret = 42

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    let guess = if secret > 1 { secret - 1 } else { 1 };
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        guess,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_ok());

    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.guess_count, 1);
    assert_eq!(entry.best_distance, 1); // 1 away from secret
    assert!(!entry.found_exact);

    eprintln!("  ✓ test_submit_guess_too_small passed");
}

#[test]
fn test_submit_guess_too_big() {
    eprintln!("\n━━━ test_submit_guess_too_big ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(41, 1); // secret = 42

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    let guess = if secret < 100 { secret + 1 } else { 100 };
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        guess,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_ok());

    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.guess_count, 1);
    assert_eq!(entry.best_distance, 1);
    assert!(!entry.found_exact);

    eprintln!("  ✓ test_submit_guess_too_big passed");
}

#[test]
fn test_best_distance_tracks_minimum() {
    eprintln!("\n━━━ test_best_distance_tracks_minimum ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(41, 1); // secret = 42

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    // Guess 1 — distance = 41
    let guess1_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        1,
    );
    send_ix(&mut svm, guess1_ix, player).unwrap();

    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.best_distance, 41);

    // Guess 50 — distance = 8 (better)
    let guess2_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        50,
    );
    send_ix(&mut svm, guess2_ix, player).unwrap();

    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.best_distance, 8);
    assert_eq!(entry.guess_count, 2);

    // Guess exact — distance = 0 (best)
    let guess3_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        secret,
    );
    send_ix(&mut svm, guess3_ix, player).unwrap();

    let entry = read_player_entry(&svm, &player_pda);
    assert_eq!(entry.best_distance, 0);
    assert!(entry.found_exact);
    assert_eq!(entry.guess_count, 3);

    eprintln!("  ✓ test_best_distance_tracks_minimum passed");
}

#[test]
fn test_no_attempts_remaining() {
    eprintln!("\n━━━ test_no_attempts_remaining ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, _secret) =
        setup_full_tournament(41, 1); // secret = 42

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    // Use all 10 attempts
    for i in 1..=10 {
        let guess_ix = build_submit_guess_ix(
            &program_id,
            &player.pubkey(),
            &tournament_pda,
            &player_pda,
            i, // deliberately wrong guesses
        );
        let res = send_ix(&mut svm, guess_ix, player);
        if i <= 10 {
            assert!(res.is_ok(), "Guess {i} should succeed");
        }
    }

    // 11th guess should fail
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        50,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(
        res.is_err(),
        "11th guess should fail — no attempts remaining"
    );

    eprintln!("  ✓ test_no_attempts_remaining passed");
}

#[test]
fn test_guess_before_settle_fails() {
    eprintln!("\n━━━ test_guess_before_settle_fails ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    // Create but don't settle
    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Join a player
    let player = fund_player(&mut svm);
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);
    let join_ix =
        build_join_tournament_ix(&program_id, &player.pubkey(), &tournament_pda, &player_pda);
    send_ix(&mut svm, join_ix, &player).unwrap();

    // Try to guess before settle — should fail
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        42,
    );
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err(), "Guess before settle should fail");

    eprintln!("  ✓ test_guess_before_settle_fails passed");
}

#[test]
fn test_double_settle_fails() {
    eprintln!("\n━━━ test_double_settle_fails ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // First settle
    svm.warp_to_slot(SETTLE_SLOT);
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, settle_ix.clone(), &admin).unwrap();

    // Second settle — should fail
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_err(), "Double settle should fail");

    eprintln!("  ✓ test_double_settle_fails passed");
}

#[test]
fn test_unauthorized_settle() {
    eprintln!("\n━━━ test_unauthorized_settle ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Non-admin tries to settle
    let imposter = fund_player(&mut svm);
    svm.warp_to_slot(SETTLE_SLOT);
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &imposter.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &imposter);
    assert!(res.is_err(), "Non-admin settle should fail");

    eprintln!("  ✓ test_unauthorized_settle passed");
}

#[test]
fn test_invalid_guess_range() {
    eprintln!("\n━━━ test_invalid_guess_range ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, _secret) =
        setup_full_tournament(41, 1);

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    // Guess 0 — should fail
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        0,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_err(), "Guess 0 should fail");

    // Guess 101 — should fail
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        101,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_err(), "Guess 101 should fail");

    eprintln!("  ✓ test_invalid_guess_range passed");
}

#[test]
fn test_close_tournament() {
    eprintln!("\n━━━ test_close_tournament ━━━");

    let (mut svm, admin, _players, program_id, tournament_pda, _, _) = setup_full_tournament(41, 3);

    let close_ix = build_close_tournament_ix(&program_id, &admin.pubkey(), &tournament_pda);
    let res = send_ix(&mut svm, close_ix, &admin);
    assert!(res.is_ok(), "Close should succeed");

    // Tournament account should be gone
    let account = svm.get_account(&tournament_pda);
    assert!(account.is_none(), "Tournament account should be closed");

    eprintln!("  ✓ test_close_tournament passed");
}

#[test]
fn test_unauthorized_close() {
    eprintln!("\n━━━ test_unauthorized_close ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, _) = setup_full_tournament(41, 1);

    // Non-admin tries to close
    let close_ix = build_close_tournament_ix(&program_id, &players[0].pubkey(), &tournament_pda);
    let res = send_ix(&mut svm, close_ix, &players[0]);
    assert!(res.is_err(), "Non-admin close should fail");

    eprintln!("  ✓ test_unauthorized_close passed");
}

#[test]
fn test_wrong_randomness_account_fails() {
    eprintln!("\n━━━ test_wrong_randomness_account_fails ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();
    let wrong_randomness = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);
    create_randomness_account_in_svm(&mut svm, &wrong_randomness.pubkey(), 99);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Try to settle with wrong randomness account
    svm.warp_to_slot(SETTLE_SLOT);
    update_randomness_account_in_svm(&mut svm, &wrong_randomness.pubkey(), 99);

    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &wrong_randomness.pubkey(), // wrong!
    );
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_err(), "Wrong randomness account should fail");

    eprintln!("  ✓ test_wrong_randomness_account_fails passed");
}

#[test]
fn test_randomness_not_ready_fails() {
    eprintln!("\n━━━ test_randomness_not_ready_fails ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Don't warp to SETTLE_SLOT — clock slot won't match reveal_slot
    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    let res = send_ix(&mut svm, settle_ix, &admin);
    assert!(res.is_err(), "Settle before randomness ready should fail");

    eprintln!("  ✓ test_randomness_not_ready_fails passed");
}

#[test]
fn test_join_before_settle_allowed() {
    eprintln!("\n━━━ test_join_before_settle_allowed ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Join before settle — should succeed
    let player = fund_player(&mut svm);
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);
    let join_ix =
        build_join_tournament_ix(&program_id, &player.pubkey(), &tournament_pda, &player_pda);
    let res = send_ix(&mut svm, join_ix, &player);
    assert!(res.is_ok(), "Join before settle should be allowed");

    let tournament = read_tournament(&svm, &tournament_pda);
    assert_eq!(tournament.player_count, 1);

    eprintln!("  ✓ test_join_before_settle_allowed passed");
}

#[test]
fn test_join_after_settle_allowed() {
    eprintln!("\n━━━ test_join_after_settle_allowed ━━━");

    let (mut svm, admin, program_id) = setup_svm();
    let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
    let randomness_keypair = Keypair::new();

    create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let create_ix = build_create_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, create_ix, &admin).unwrap();

    // Settle first
    svm.warp_to_slot(SETTLE_SLOT);
    update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), 41);

    let settle_ix = build_settle_tournament_ix(
        &program_id,
        &admin.pubkey(),
        &tournament_pda,
        &randomness_keypair.pubkey(),
    );
    send_ix(&mut svm, settle_ix, &admin).unwrap();

    // Join after settle — should succeed
    let player = fund_player(&mut svm);
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);
    let join_ix =
        build_join_tournament_ix(&program_id, &player.pubkey(), &tournament_pda, &player_pda);
    let res = send_ix(&mut svm, join_ix, &player);
    assert!(res.is_ok(), "Join after settle should be allowed");

    eprintln!("  ✓ test_join_after_settle_allowed passed");
}

#[test]
fn test_multi_player_competition() {
    eprintln!("\n━━━ test_multi_player_competition ━━━");

    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(41, 3); // secret = 42

    // Player 1: guesses wrong (distance 10)
    let p1 = &players[0];
    let p1_pda = get_player_pda(&tournament_pda, &p1.pubkey(), &program_id);
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &p1.pubkey(),
        &tournament_pda,
        &p1_pda,
        secret - 10,
    );
    send_ix(&mut svm, guess_ix, p1).unwrap();

    // Player 2: guesses correctly in 1 attempt
    let p2 = &players[1];
    let p2_pda = get_player_pda(&tournament_pda, &p2.pubkey(), &program_id);
    let guess_ix =
        build_submit_guess_ix(&program_id, &p2.pubkey(), &tournament_pda, &p2_pda, secret);
    send_ix(&mut svm, guess_ix, p2).unwrap();

    // Player 3: guesses wrong (distance 5)
    let p3 = &players[2];
    let p3_pda = get_player_pda(&tournament_pda, &p3.pubkey(), &program_id);
    let guess_ix = build_submit_guess_ix(
        &program_id,
        &p3.pubkey(),
        &tournament_pda,
        &p3_pda,
        secret + 5,
    );
    send_ix(&mut svm, guess_ix, p3).unwrap();

    // Verify results
    let p1_entry = read_player_entry(&svm, &p1_pda);
    let p2_entry = read_player_entry(&svm, &p2_pda);
    let p3_entry = read_player_entry(&svm, &p3_pda);

    assert_eq!(p1_entry.best_distance, 10);
    assert!(!p1_entry.found_exact);

    assert_eq!(p2_entry.best_distance, 0);
    assert!(p2_entry.found_exact);
    assert_eq!(p2_entry.guess_count, 1);

    assert_eq!(p3_entry.best_distance, 5);
    assert!(!p3_entry.found_exact);

    eprintln!("  ✓ test_multi_player_competition passed");
    eprintln!("    Player 1: distance=10, Player 2: WON (1 attempt), Player 3: distance=5");
}

#[test]
fn test_boundary_value_1() {
    eprintln!("\n━━━ test_boundary_value_1 ━━━");

    // value_byte=0 → secret = (0 % 100) + 1 = 1
    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(0, 1);
    assert_eq!(secret, 1);

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        1,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_ok());

    let entry = read_player_entry(&svm, &player_pda);
    assert!(entry.found_exact);

    eprintln!("  ✓ test_boundary_value_1 passed (secret=1)");
}

#[test]
fn test_boundary_value_100() {
    eprintln!("\n━━━ test_boundary_value_100 ━━━");

    // value_byte=99 → secret = (99 % 100) + 1 = 100
    let (mut svm, _admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(99, 1);
    assert_eq!(secret, 100);

    let player = &players[0];
    let player_pda = get_player_pda(&tournament_pda, &player.pubkey(), &program_id);

    let guess_ix = build_submit_guess_ix(
        &program_id,
        &player.pubkey(),
        &tournament_pda,
        &player_pda,
        100,
    );
    let res = send_ix(&mut svm, guess_ix, player);
    assert!(res.is_ok());

    let entry = read_player_entry(&svm, &player_pda);
    assert!(entry.found_exact);

    eprintln!("  ✓ test_boundary_value_100 passed (secret=100)");
}

#[test]
fn test_settle_boundary_values() {
    eprintln!("\n━━━ test_settle_boundary_values ━━━");

    // Test multiple value_bytes → correct secret derivation
    for value_byte in [0u8, 1, 50, 99, 100, 150, 200, 255] {
        let expected = (value_byte % 100) + 1;

        let (mut svm, admin, program_id) = setup_svm();
        let tournament_pda = get_tournament_pda(&admin.pubkey(), &program_id);
        let randomness_keypair = Keypair::new();

        create_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), value_byte);

        let create_ix = build_create_tournament_ix(
            &program_id,
            &admin.pubkey(),
            &tournament_pda,
            &randomness_keypair.pubkey(),
        );
        send_ix(&mut svm, create_ix, &admin).unwrap();

        svm.warp_to_slot(SETTLE_SLOT);
        update_randomness_account_in_svm(&mut svm, &randomness_keypair.pubkey(), value_byte);

        let settle_ix = build_settle_tournament_ix(
            &program_id,
            &admin.pubkey(),
            &tournament_pda,
            &randomness_keypair.pubkey(),
        );
        send_ix(&mut svm, settle_ix, &admin).unwrap();

        let tournament = read_tournament(&svm, &tournament_pda);
        assert_eq!(
            tournament.secret_number, expected,
            "value_byte={value_byte} should give secret={expected}"
        );
    }

    eprintln!("  ✓ test_settle_boundary_values passed (8 value_bytes tested)");
}

#[test]
fn test_full_tournament_session() {
    eprintln!("\n━━━ test_full_tournament_session ━━━");

    let (mut svm, admin, players, program_id, tournament_pda, _, secret) =
        setup_full_tournament(41, 3); // secret = 42

    eprintln!("    Secret number: {secret}");

    // Player 1: binary search down
    let p1 = &players[0];
    let p1_pda = get_player_pda(&tournament_pda, &p1.pubkey(), &program_id);
    for guess in [50, 25, 37, 43, 40, 41, 42] {
        let ix = build_submit_guess_ix(&program_id, &p1.pubkey(), &tournament_pda, &p1_pda, guess);
        send_ix(&mut svm, ix, p1).unwrap();
    }
    let p1_entry = read_player_entry(&svm, &p1_pda);
    assert!(p1_entry.found_exact);
    eprintln!("    Player 1: found in {} guesses", p1_entry.guess_count);

    // Player 2: lucky first guess
    let p2 = &players[1];
    let p2_pda = get_player_pda(&tournament_pda, &p2.pubkey(), &program_id);
    let ix = build_submit_guess_ix(&program_id, &p2.pubkey(), &tournament_pda, &p2_pda, secret);
    send_ix(&mut svm, ix, p2).unwrap();
    let p2_entry = read_player_entry(&svm, &p2_pda);
    assert!(p2_entry.found_exact);
    assert_eq!(p2_entry.guess_count, 1);
    eprintln!(
        "    Player 2: found in {} guesses (lucky!)",
        p2_entry.guess_count
    );

    // Player 3: never finds it (uses all 10 wrong guesses)
    let p3 = &players[2];
    let p3_pda = get_player_pda(&tournament_pda, &p3.pubkey(), &program_id);
    for guess in 1..=10u8 {
        // 1-10 are all wrong since secret=42
        let ix = build_submit_guess_ix(&program_id, &p3.pubkey(), &tournament_pda, &p3_pda, guess);
        send_ix(&mut svm, ix, p3).unwrap();
    }
    let p3_entry = read_player_entry(&svm, &p3_pda);
    assert!(!p3_entry.found_exact);
    assert_eq!(p3_entry.guess_count, 10);
    assert_eq!(p3_entry.best_distance, 32); // |10 - 42| = 32
    eprintln!(
        "    Player 3: exhausted attempts, best_distance={}",
        p3_entry.best_distance
    );

    // Close tournament
    let close_ix = build_close_tournament_ix(&program_id, &admin.pubkey(), &tournament_pda);
    let res = send_ix(&mut svm, close_ix, &admin);
    assert!(res.is_ok());

    eprintln!("  ✓ test_full_tournament_session passed");
    eprintln!("    Winner: Player 2 (1 guess) > Player 1 (7 guesses) > Player 3 (no exact match)");
}
