// morse-core: the portable, UI-free heart of the Morse trainer.
//
// This crate contains everything platform-independent: the Morse alphabet, the
// practice word pool, WPM-based timing, audio-schedule generation, key decoding
// (two-key and straight-key), the drill modes, and the trainer state machine
// (scoring, letter progression, hints, word selection).
//
// It has no platform dependencies, so the identical logic compiles to WASM for
// the web build and to native libraries for Android/iOS. All game rules and
// timing live here and are covered by unit tests you can run with `cargo test`.

pub mod alphabet;
pub mod audio;
pub mod coach;
pub mod keyer;
pub mod mnemonics;
pub mod mode;
pub mod rng;
pub mod timing;
pub mod trainer;
pub mod words;

pub use alphabet::{from_morse, to_morse, LEARNING_ORDER};
pub use audio::{schedule_code, schedule_text, total_duration_ms, Tone};
pub use coach::{SpeedAdvice, SpeedCoach};
pub use keyer::{Gap, Keyer, Symbol};
pub use mnemonics::{mnemonic, Mnemonic, Syllable};
pub use mode::{Answer, Drill, InputMethod, Prompt};
pub use timing::{Timing, MAX_WPM, MIN_WPM};
pub use trainer::{HintLevel, Judgement, LetterProgress, Trainer};
pub use words::WORDS;
