// Phase 1 LiteSVM Tests — Commit-Reveal Guessing Game
//
// These tests run the actual compiled BPF program in-memory using LiteSVM.
// No network, no devnet, no TypeScript — just `cargo test`.
//
// Flow: admin commits blake3(secret) → admin reveals secret → player guesses

use {
    anchor_lang::{
        AccountDeserialize, InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

// ─── Helpers ───────────────────────────────────────────────────────────

/// Create a fresh LiteSVM instance with the Phase 1 program loaded and admin funded.
fn setup_svm() -> (LiteSVM, Keypair, Pubkey) {
    let program_id = on_chain::id();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();

    // Load the compiled BPF program into LiteSVM
    let bytes = include_bytes!("../../../target/deploy/on_chain.so");
    svm.add_program(program_id, bytes).unwrap();

    // Fund admin with 2 SOL so they can pay for transactions
    svm.airdrop(&admin.pubkey(), 2_000_000_000).unwrap();

    (svm, admin, program_id)
}

/// Derive the game PDA from admin's pubkey: seeds = [b"game", admin]
fn get_game_pda(admin: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(&[b"game", admin.as_ref()], program_id);
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

// ─── Instruction Builders ──────────────────────────────────────────────

/// Build an `initialize` instruction: admin creates game and commits blake3(secret) on-chain.
/// The secret number is NOT stored — only its hash.
fn build_initialize_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    game_pda: &Pubkey,
    secret_number: u8,
) -> Instruction {
    let system_program_id = Pubkey::from([0u8; 32]);
    Instruction::new_with_bytes(
        *program_id,
        &on_chain::instruction::Initialize { secret_number }.data(),
        on_chain::accounts::Initialize {
            game: *game_pda,
            admin: *admin,
            system_program: system_program_id,
        }
        .to_account_metas(None),
    )
}

/// Build a `reveal` instruction: admin reveals the secret, program verifies blake3(secret) == stored hash.
/// Only then is the actual secret_number stored in the game account.
fn build_reveal_ix(
    program_id: &Pubkey,
    admin: &Pubkey,
    game_pda: &Pubkey,
    secret_number: u8,
) -> Instruction {
    Instruction::new_with_bytes(
        *program_id,
        &on_chain::instruction::Reveal { secret_number }.data(),
        on_chain::accounts::Reveal {
            game: *game_pda,
            admin: *admin,
        }
        .to_account_metas(None),
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
        &on_chain::instruction::Guess { guess }.data(),
        on_chain::accounts::Guess {
            game: *game_pda,
            player: *player,
        }
        .to_account_metas(None),
    )
}

/// Read the game account from LiteSVM and deserialize it into the Game struct.
fn read_game(svm: &LiteSVM, game_pda: &Pubkey) -> on_chain::state::Game {
    let account = svm.get_account(game_pda).unwrap();
    let mut data: &[u8] = &account.data;
    on_chain::state::Game::try_deserialize(&mut data).unwrap()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

/// Test: Initialize a new game with secret=42.
/// Verifies: admin stored, hash is blake3(42), secret=0 (not revealed yet), defaults correct.
#[test]
fn test_initialize() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, ix, &admin);
    assert!(res.is_ok());

    let game = read_game(&svm, &game_pda);
    assert_eq!(game.admin, admin.pubkey());
    let expected_hash = blake3::hash(&42u8.to_le_bytes());
    assert_eq!(game.secret_hash, *expected_hash.as_bytes());
    assert_eq!(game.secret_number, 0);
    assert!(!game.is_revealed);
    assert_eq!(game.attempts, 0);
    assert_eq!(game.max_tries, 10);
    assert!(!game.is_finished);
}

/// Test: Reveal the secret after initialize.
/// Verifies: is_revealed=true, secret_number=42 (now stored).
#[test]
fn test_reveal() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, reveal_ix, &admin);
    assert!(res.is_ok());

    let game = read_game(&svm, &game_pda);
    assert!(game.is_revealed);
    assert_eq!(game.secret_number, 42);
}

/// Test: Player guesses the exact secret (42) → game finished, 1 attempt.
#[test]
fn test_guess_correct() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();

    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();

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
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();

    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();

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
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();

    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();

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
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();

    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 10_000_000_000).unwrap();

    for i in 0..10 {
        let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 1);
        let res = send_ix(&mut svm, guess_ix, &player);
        assert!(res.is_ok(), "Guess iteration {i} failed");
    }

    let game = read_game(&svm, &game_pda);
    assert!(game.is_finished);
    assert_eq!(game.attempts, 10);

    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 1);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err());
}

/// Test: Non-admin tries to reveal → rejected.
#[test]
fn test_unauthorized_reveal() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let impostor = Keypair::new();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &impostor.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, reveal_ix, &impostor);
    assert!(res.is_err());
}

/// Test: Admin reveals a different secret than committed → rejected (hash mismatch).
#[test]
fn test_hash_mismatch() {
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);

    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();

    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 43);
    let res = send_ix(&mut svm, reveal_ix, &admin);
    assert!(res.is_err());
}
