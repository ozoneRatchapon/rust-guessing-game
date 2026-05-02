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

fn setup_svm() -> (LiteSVM, Keypair, Pubkey) {
    let program_id = on_chain::id();
    let admin = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/on_chain.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 2_000_000_000).unwrap();
    (svm, admin, program_id)
}

fn get_game_pda(admin: &Pubkey, program_id: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(&[b"game", admin.as_ref()], program_id);
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

fn read_game(svm: &LiteSVM, game_pda: &Pubkey) -> on_chain::state::Game {
    let account = svm.get_account(game_pda).unwrap();
    let mut data: &[u8] = &account.data;
    on_chain::state::Game::try_deserialize(&mut data).unwrap()
}

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
