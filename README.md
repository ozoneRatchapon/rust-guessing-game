# Rust Guessing Game

A simple, interactive command-line game built with **Rust**. This project is based on the second chapter of [The Rust Programming Language](https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html).

## Objective
The game generates a secret random number between **1 and 100**. Your goal is to guess the number in as few attempts as possible. After each guess, the game will tell you if your guess was too small, too big, or correct!

## Features
- **Random Number Generation**: Uses the `rand` crate for high-quality randomness.
- **Input Validation**: Gracefully handles non-numeric inputs, allowing you to keep playing if you accidentally type a letter.
- **Interactive Feedback**: Provides instant "Too small", "Too big", or "You win!" responses.
- **Infinite Loop**: The game continues until you guess the correct number.

## How to Run

### Prerequisites
Ensure you have [Rust and Cargo](https://www.rust-lang.org/tools/install) installed on your system.

### Steps
1. **Clone the repository** (if applicable):
   ```bash
   git clone <repository-url>
   cd rust-guessing-game
   ```
2. **Run the game**:
   ```bash
   cargo run
   ```

## Built With
- **Rust** - The programming language used.
- **Cargo** - Rust's package manager and build system.
- **rand** crate - For generating the secret number.

## Learnings
While building this project, I explored several key Rust concepts:
- Handling user input with `std::io`.
- Variables and Mutability (`let mut`).
- Using external crates via `Cargo.toml`.
- Pattern matching with `match`.
- Control flow using `loop` and `break`.
- Error handling with `Result` and `Expect`.

---
*Created as part of the Turbine task series.*
