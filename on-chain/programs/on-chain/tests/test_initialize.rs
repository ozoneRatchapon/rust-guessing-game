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
    eprintln!("\n━━━ test_initialize ━━━");

    // Step 1: Create LiteSVM with Phase 1 program loaded, admin funded with 2 SOL
    let (mut svm, admin, program_id) = setup_svm();
    eprintln!("  Step 1: Setup LiteSVM + admin funded");

    // Step 2: Derive game PDA from admin pubkey
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    eprintln!("  Step 2: Game PDA = {}", game_pda);

    // Step 3: Build and send initialize instruction with secret=42
    let ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, ix, &admin);
    assert!(res.is_ok());
    eprintln!("  Step 3: initialize(secret=42) → OK");

    // Step 4: Read game account and verify all fields
    let game = read_game(&svm, &game_pda);
    assert_eq!(game.admin, admin.pubkey());
    let expected_hash = blake3::hash(&42u8.to_le_bytes());
    assert_eq!(game.secret_hash, *expected_hash.as_bytes());
    assert_eq!(game.secret_number, 0);
    assert!(!game.is_revealed);
    assert_eq!(game.attempts, 0);
    assert_eq!(game.max_tries, 10);
    assert!(!game.is_finished);
    eprintln!("  Step 4: Game verified — admin ok, hash=blake3(42), secret=0, not revealed");
    eprintln!("  ✓ test_initialize passed");
}

/// Test: Reveal the secret after initialize.
/// Verifies: is_revealed=true, secret_number=42 (now stored).
#[test]
fn test_reveal() {
    eprintln!("\n━━━ test_reveal ━━━");

    // Step 1: Setup and initialize game with secret=42
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    eprintln!("  Step 1: initialize(secret=42) → OK");

    // Step 2: Admin reveals the secret
    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, reveal_ix, &admin);
    assert!(res.is_ok());
    eprintln!("  Step 2: reveal(42) → OK");

    // Step 3: Verify game state — secret now stored
    let game = read_game(&svm, &game_pda);
    assert!(game.is_revealed);
    assert_eq!(game.secret_number, 42);
    eprintln!("  Step 3: Game verified — is_revealed=true, secret_number=42");
    eprintln!("  ✓ test_reveal passed");
}

/// Test: Player guesses the exact secret (42) → game finished, 1 attempt.
#[test]
fn test_guess_correct() {
    eprintln!("\n━━━ test_guess_correct ━━━");

    // Step 1: Setup game with secret=42, reveal it
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();
    eprintln!("  Step 1: init(secret=42) + reveal → secret now on-chain");

    // Step 2: Fund a player
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();
    eprintln!("  Step 2: Player funded");

    // Step 3: Player guesses 42 (exact match!)
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_ok());
    eprintln!("  Step 3: guess(42) → CORRECT!");

    // Step 4: Verify game finished with 1 attempt
    let game = read_game(&svm, &game_pda);
    assert!(game.is_finished);
    assert_eq!(game.attempts, 1);
    eprintln!("  Step 4: Game verified — is_finished=true, attempts=1");
    eprintln!("  ✓ test_guess_correct passed");
}

/// Test: Player guesses below the secret (10 < 50) → game continues, 1 attempt.
#[test]
fn test_guess_too_small() {
    eprintln!("\n━━━ test_guess_too_small ━━━");

    // Step 1: Setup game with secret=50, reveal it
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();
    eprintln!("  Step 1: init(secret=50) + reveal → OK");

    // Step 2: Fund player
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();
    eprintln!("  Step 2: Player funded");

    // Step 3: Player guesses 10 (below secret)
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 10);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_ok());
    eprintln!("  Step 3: guess(10) → TOO SMALL (secret=50)");

    // Step 4: Game continues, 1 attempt used
    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 1);
    eprintln!("  Step 4: Game continues — is_finished=false, attempts=1");
    eprintln!("  ✓ test_guess_too_small passed");
}

/// Test: Player guesses above the secret (90 > 50) → game continues, 1 attempt.
#[test]
fn test_guess_too_big() {
    eprintln!("\n━━━ test_guess_too_big ━━━");

    // Step 1: Setup game with secret=50, reveal it
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 50);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();
    eprintln!("  Step 1: init(secret=50) + reveal → OK");

    // Step 2: Fund player
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 1_000_000_000).unwrap();
    eprintln!("  Step 2: Player funded");

    // Step 3: Player guesses 90 (above secret)
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 90);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_ok());
    eprintln!("  Step 3: guess(90) → TOO BIG (secret=50)");

    // Step 4: Game continues, 1 attempt used
    let game = read_game(&svm, &game_pda);
    assert!(!game.is_finished);
    assert_eq!(game.attempts, 1);
    eprintln!("  Step 4: Game continues — is_finished=false, attempts=1");
    eprintln!("  ✓ test_guess_too_big passed");
}

/// Test: 10 wrong guesses → game finished, 11th guess rejected.
#[test]
fn test_guess_game_over() {
    eprintln!("\n━━━ test_guess_game_over ━━━");

    // Step 1: Setup game with secret=42, reveal it
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, reveal_ix, &admin).unwrap();
    eprintln!("  Step 1: init(secret=42) + reveal → OK");

    // Step 2: Fund player
    let player = Keypair::new();
    svm.airdrop(&player.pubkey(), 10_000_000_000).unwrap();
    eprintln!("  Step 2: Player funded");

    // Step 3: Guess wrong 10 times (guess=1, secret=42)
    for i in 0..10 {
        let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 1);
        let res = send_ix(&mut svm, guess_ix, &player);
        assert!(res.is_ok(), "Guess iteration {i} failed");
        eprintln!("  Step 3.{i}: guess(1) → TOO SMALL (secret=42)");
    }

    // Step 4: Verify game is over after 10 attempts
    let game = read_game(&svm, &game_pda);
    assert!(game.is_finished);
    assert_eq!(game.attempts, 10);
    eprintln!("  Step 4: Game over — is_finished=true, attempts=10");

    // Step 5: 11th guess should fail
    let guess_ix = build_guess_ix(&program_id, &player.pubkey(), &game_pda, 1);
    let res = send_ix(&mut svm, guess_ix, &player);
    assert!(res.is_err());
    eprintln!("  Step 5: guess(1) 11th attempt → REJECTED (game over)");
    eprintln!("  ✓ test_guess_game_over passed");
}

/// Test: Non-admin tries to reveal → rejected.
#[test]
fn test_unauthorized_reveal() {
    eprintln!("\n━━━ test_unauthorized_reveal ━━━");

    // Step 1: Setup game with secret=42
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    eprintln!("  Step 1: init(secret=42) → OK");

    // Step 2: Fund an impostor
    let impostor = Keypair::new();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();
    eprintln!("  Step 2: Impostor funded");

    // Step 3: Impostor tries to reveal
    let reveal_ix = build_reveal_ix(&program_id, &impostor.pubkey(), &game_pda, 42);
    let res = send_ix(&mut svm, reveal_ix, &impostor);
    assert!(res.is_err());
    eprintln!("  Step 3: impostor.reveal(42) → REJECTED (not admin)");
    eprintln!("  ✓ test_unauthorized_reveal passed");
}

/// Test: Admin reveals a different secret than committed → rejected (hash mismatch).
#[test]
fn test_hash_mismatch() {
    eprintln!("\n━━━ test_hash_mismatch ━━━");

    // Step 1: Setup game, commit blake3(42)
    let (mut svm, admin, program_id) = setup_svm();
    let game_pda = get_game_pda(&admin.pubkey(), &program_id);
    let init_ix = build_initialize_ix(&program_id, &admin.pubkey(), &game_pda, 42);
    send_ix(&mut svm, init_ix, &admin).unwrap();
    eprintln!("  Step 1: init(secret=42) → committed blake3(42)");

    // Step 2: Admin tries to reveal 43 instead (hash won't match)
    let reveal_ix = build_reveal_ix(&program_id, &admin.pubkey(), &game_pda, 43);
    let res = send_ix(&mut svm, reveal_ix, &admin);
    assert!(res.is_err());
    eprintln!("  Step 2: reveal(43) → REJECTED (blake3(43) != blake3(42))");
    eprintln!("  ✓ test_hash_mismatch passed");
}
