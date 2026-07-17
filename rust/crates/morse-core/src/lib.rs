// morse-core: the portable, UI-free heart of the Morse trainer.
//
// This crate contains the Morse alphabet, the practice word pool, and the
// trainer state machine (scoring, letter progression, hints, word selection).
// It has no platform dependencies, so the identical logic compiles to WASM for
// the web build and to native libraries for the Android/iOS builds. All game
// rules live here and are covered by unit tests you can run with `cargo test`.

pub mod alphabet;
pub mod rng;
pub mod trainer;
pub mod words;

pub use alphabet::{from_morse, to_morse, LEARNING_ORDER};
pub use trainer::{HintLevel, Judgement, LetterProgress, Trainer};
pub use words::WORDS;
