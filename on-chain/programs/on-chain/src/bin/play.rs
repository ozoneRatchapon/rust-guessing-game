#![cfg(feature = "play")]

use {
    anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::io::{self, Write},
};

use on_chain::state::Game;

const SECRET: u8 = 42;

fn read_guess(prompt: &str) -> u8 {
    loop {
        print!("{prompt}");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        match input.trim().parse::<u8>() {
            Ok(n) if (1..=100).contains(&n) => return n,
            Ok(_) => println!("  Please enter a number between 1 and 100."),
            Err(_) => println!("  Invalid input. Please enter a number."),
        }
    }
}

fn send_ix(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    accounts: impl ToAccountMetas,
    data: impl InstructionData,
    signer: &Keypair,
) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata> {
    let ix = solana_instruction::Instruction::new_with_bytes(
        *program_id,
        &data.data(),
        accounts.to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[signer]).unwrap();
    let result = svm.send_transaction(tx);
    svm.expire_blockhash();
    result
}

fn read_game(svm: &LiteSVM, game_pda: &Pubkey) -> Game {
    let account = svm.get_account(game_pda).unwrap();
    Game::try_deserialize(&mut &account.data[..]).unwrap()
}

fn main() {
    println!("========================================");
    println!("  Solana On-Chain Guessing Game");
    println!("  Phase 1: Commit-Reveal Edition");
    println!("========================================");
    println!();
    println!("How it works:");
    println!("  1. ADMIN commits blake3(secret) on-chain");
    println!("  2. ADMIN reveals the secret (program verifies hash)");
    println!("  3. YOU guess the number (1-100)");
    println!("  4. You have 10 attempts");
    println!();
    println!("Running on LiteSVM (lightweight Solana VM)");
    println!("executing the real compiled BPF program.");
    println!();

    let program_id = on_chain::id();
    let admin = Keypair::new();
    let player = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/on_chain.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 2_000_000_000).unwrap();
    svm.airdrop(&player.pubkey(), 2_000_000_000).unwrap();

    let (game_pda, _) =
        Pubkey::find_program_address(&[b"game", admin.pubkey().as_ref()], &program_id);

    // Step 1: Initialize
    println!("--- Step 1: Admin initializes game ---");
    let hash = blake3::hash(&SECRET.to_le_bytes());
    println!("Committing blake3 hash of secret...");
    println!("  Hash: {}", hex::encode(hash.as_bytes()));

    let init_accounts = on_chain::accounts::Initialize {
        game: game_pda,
        admin: admin.pubkey(),
        system_program: Pubkey::from([0u8; 32]),
    };
    send_ix(
        &mut svm,
        &program_id,
        init_accounts,
        on_chain::instruction::Initialize {
            secret_number: SECRET,
        },
        &admin,
    )
    .unwrap();

    let game = read_game(&svm, &game_pda);
    println!("Game created!");
    println!("  Admin: {}", game.admin);
    println!("  Secret number: ??? (stored as 0 on-chain)");
    println!("  Max tries: {}", game.max_tries);
    println!();

    // Step 2: Reveal
    println!("--- Step 2: Admin reveals secret ---");
    println!("Program verifies: blake3(secret) == stored_hash ...");

    let reveal_accounts = on_chain::accounts::Reveal {
        game: game_pda,
        admin: admin.pubkey(),
    };
    send_ix(
        &mut svm,
        &program_id,
        reveal_accounts,
        on_chain::instruction::Reveal {
            secret_number: SECRET,
        },
        &admin,
    )
    .unwrap();

    let game = read_game(&svm, &game_pda);
    println!("Hash verified! Secret is now on-chain.");
    println!("  is_revealed: {}", game.is_revealed);
    println!("  (You still don't know the number!)");
    println!();

    // Step 3: Guess loop
    println!("--- Step 3: Your turn to guess ---");
    println!("Guess the number between 1 and 100.");
    println!("You have {} attempts. Good luck!\n", game.max_tries);

    let max_tries = game.max_tries;
    let mut attempts = 0;

    loop {
        if attempts >= max_tries {
            println!();
            println!("========================================");
            println!("  GAME OVER! No more attempts!");
            println!("  The secret was: {SECRET}");
            println!("========================================");
            break;
        }

        let guess = read_guess(&format!(
            "[Attempt {}/{}] Your guess: ",
            attempts + 1,
            max_tries
        ));

        let guess_accounts = on_chain::accounts::Guess {
            game: game_pda,
            player: player.pubkey(),
        };
        let result = send_ix(
            &mut svm,
            &program_id,
            guess_accounts,
            on_chain::instruction::Guess { guess },
            &player,
        );

        match result {
            Ok(meta) => {
                attempts += 1;
                let game = read_game(&svm, &game_pda);
                let remaining = max_tries - attempts;

                let is_correct = meta.logs.iter().any(|l| l.contains("Correct"));
                let is_too_small = meta.logs.iter().any(|l| l.contains("too small"));
                let is_too_big = meta.logs.iter().any(|l| l.contains("too big"));
                let is_game_over = meta.logs.iter().any(|l| l.contains("Game over"));

                match (is_correct, is_too_small, is_too_big, is_game_over) {
                    (true, _, _, _) => {
                        println!("  >> CORRECT! You guessed {guess} in {attempts} attempts!\n");
                        println!("========================================");
                        println!("  YOU WIN!");
                        println!("  Secret: {SECRET} | Attempts: {attempts}");
                        println!("========================================");
                        break;
                    }
                    (_, true, _, _) => {
                        println!("  >> {guess} is too small! ({remaining} left)");
                    }
                    (_, _, true, _) => {
                        println!("  >> {guess} is too big! ({remaining} left)");
                    }
                    (_, _, _, true) => {
                        println!("  >> GAME OVER! The secret was {SECRET}");
                        break;
                    }
                    _ => {}
                }

                if game.is_finished {
                    println!("\n========================================");
                    println!("  GAME OVER! Secret was: {SECRET}");
                    println!("========================================");
                    break;
                }
            }
            Err(e) => {
                println!("  >> Transaction failed: {:?}", e.err);
            }
        }
    }

    // Final state
    println!("\n--- Final Game State ---");
    let game = read_game(&svm, &game_pda);
    println!("  Secret: {}", game.secret_number);
    println!("  Attempts: {}", game.attempts);
    println!("  Finished: {}", game.is_finished);
    println!("  Revealed: {}", game.is_revealed);
}
