// The trainer state machine.
//
// This owns *all* of the learning logic ported from the original game
// (game-space.js, game-state.js, config.js) with none of the rendering. A UI
// layer — Dioxus on web/mobile, or a test — drives it by reading the current
// target letter and calling `submit(...)`. Because it is pure Rust with no I/O,
// the exact same logic runs on every platform and is trivially unit-testable.

use crate::alphabet::{self, LEARNING_ORDER};
use crate::rng::Rng;
use crate::words::WORDS;

/// A letter is considered "learned" once its score reaches this value.
pub const LEARNED_THRESHOLD: i32 = 2;
/// Consecutive correct answers required (with the newest letter learned) before
/// the next letter is unlocked.
pub const CONSECUTIVE_CORRECT: u32 = 3;
/// Scores are clamped to `[-SCORE_LIMIT, SCORE_LIMIT]`.
pub const SCORE_LIMIT: i32 = LEARNED_THRESHOLD + 2;
/// How many letters are unlocked at the very start.
pub const STARTING_LETTERS: usize = 3;

/// The result of submitting an answer for the current letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judgement {
    /// The answer matched the target letter.
    Correct,
    /// The answer did not match.
    Incorrect,
}

/// How much of a hint the UI should surface for the current letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintLevel {
    /// The letter is already learned — no hint needed.
    None,
    /// Not yet learned: show the mnemonic picture for the letter.
    Picture,
    /// A few mistakes in: also reveal the Morse pattern.
    Pattern,
}

/// The score for a single letter, plus whether it counts as learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LetterProgress {
    pub letter: char,
    pub score: i32,
    pub learned: bool,
}

/// The full trainer state. Construct with [`Trainer::new`] or restore progress
/// with [`Trainer::restore`].
#[derive(Debug, Clone)]
pub struct Trainer {
    /// Score per letter, indexed the same as [`LEARNING_ORDER`].
    scores: [i32; 26],
    /// Letters currently available in the practice pool, in learning order.
    letters_in_play: Vec<char>,
    /// The word currently being practised.
    current_word: String,
    /// Index of the letter within `current_word` the user must type next.
    letter_index: usize,
    /// Consecutive correct answers (resets on any mistake).
    consecutive_correct: u32,
    /// Mistakes made on the *current* letter (resets when it's answered).
    mistakes_on_letter: u32,
    rng: Rng,
}

impl Trainer {
    /// Start a fresh session. `seed` makes word selection reproducible; pass a
    /// clock-derived value in the app for variety, or a fixed value in tests.
    pub fn new(seed: u64) -> Self {
        Self::restore([0; 26], seed)
    }

    /// Restore a session from previously saved per-letter scores (indexed like
    /// [`LEARNING_ORDER`]). Unlocks every learned letter plus the next one, then
    /// picks a first word.
    pub fn restore(scores: [i32; 26], seed: u64) -> Self {
        let mut trainer = Trainer {
            scores,
            letters_in_play: Vec::new(),
            current_word: String::new(),
            letter_index: 0,
            consecutive_correct: 0,
            mistakes_on_letter: 0,
            rng: Rng::new(seed),
        };
        trainer.recompute_letters_in_play();
        trainer.current_word = trainer.pick_word();
        trainer
    }

    // ---- Reading current state (used by the UI) ---------------------------

    /// The word currently being practised.
    pub fn current_word(&self) -> &str {
        &self.current_word
    }

    /// Index of the letter to type next within [`Trainer::current_word`].
    pub fn letter_index(&self) -> usize {
        self.letter_index
    }

    /// The specific letter the user must enter right now.
    pub fn target_letter(&self) -> char {
        self.current_word
            .chars()
            .nth(self.letter_index)
            .unwrap_or(' ')
    }

    /// The Morse pattern for the target letter (e.g. `"..."`).
    pub fn target_morse(&self) -> &'static str {
        alphabet::to_morse(self.target_letter()).unwrap_or("")
    }

    /// Which letters are unlocked and in the practice pool.
    pub fn letters_in_play(&self) -> &[char] {
        &self.letters_in_play
    }

    /// The hint the UI should show for the current letter.
    pub fn hint_level(&self) -> HintLevel {
        if self.score_of(self.target_letter()) >= LEARNED_THRESHOLD {
            HintLevel::None
        } else if self.mistakes_on_letter >= 4 {
            HintLevel::Pattern
        } else {
            HintLevel::Picture
        }
    }

    /// Progress for every letter in learning order — drives the header lights.
    pub fn progress(&self) -> Vec<LetterProgress> {
        LEARNING_ORDER
            .iter()
            .enumerate()
            .map(|(i, &letter)| LetterProgress {
                letter,
                score: self.scores[i],
                learned: self.scores[i] >= LEARNED_THRESHOLD,
            })
            .collect()
    }

    /// Fraction of the alphabet learned so far, in `0.0..=1.0`.
    pub fn completion(&self) -> f32 {
        let learned = self.scores.iter().filter(|&&s| s >= LEARNED_THRESHOLD).count();
        learned as f32 / 26.0
    }

    /// The raw per-letter scores, for persistence. Indexed like [`LEARNING_ORDER`].
    pub fn scores(&self) -> [i32; 26] {
        self.scores
    }

    // ---- Driving the session ----------------------------------------------

    /// Submit the Morse the user entered (e.g. `"..."`). Convenience wrapper
    /// that decodes to a letter first; an undecodable pattern counts as a miss.
    pub fn submit_morse(&mut self, code: &str) -> Judgement {
        match alphabet::from_morse(code) {
            Some(letter) => self.submit(letter),
            None => self.register_incorrect(),
        }
    }

    /// Submit a decoded letter as the user's answer for the current target.
    pub fn submit(&mut self, answer: char) -> Judgement {
        if answer.to_ascii_lowercase() == self.target_letter() {
            self.register_correct()
        } else {
            self.register_incorrect()
        }
    }

    fn register_correct(&mut self) -> Judgement {
        let letter = self.target_letter();
        self.bump_score(letter, 1);
        self.consecutive_correct += 1;
        self.mistakes_on_letter = 0;

        self.letter_index += 1;
        if self.letter_index >= self.current_word.chars().count() {
            // Word finished — try to unlock a new letter, then fetch the next word.
            self.maybe_unlock_letter();
            self.recompute_letters_in_play();
            self.current_word = self.pick_word();
            self.letter_index = 0;
        }
        Judgement::Correct
    }

    fn register_incorrect(&mut self) -> Judgement {
        let letter = self.target_letter();
        self.bump_score(letter, -1);
        self.consecutive_correct = 0;
        self.mistakes_on_letter += 1;
        Judgement::Incorrect
    }

    // ---- Internals ---------------------------------------------------------

    fn index_of(letter: char) -> Option<usize> {
        LEARNING_ORDER.iter().position(|&c| c == letter)
    }

    fn score_of(&self, letter: char) -> i32 {
        Self::index_of(letter).map(|i| self.scores[i]).unwrap_or(0)
    }

    fn bump_score(&mut self, letter: char, delta: i32) {
        if let Some(i) = Self::index_of(letter) {
            self.scores[i] = (self.scores[i] + delta).clamp(-SCORE_LIMIT, SCORE_LIMIT);
        }
    }

    /// Unlock the next letter when the newest in play is learned and the user
    /// has a streak of at least [`CONSECUTIVE_CORRECT`]. Mirrors the original
    /// `checkAddLetters` logic.
    fn maybe_unlock_letter(&mut self) {
        let newest_learned = self
            .letters_in_play
            .last()
            .map(|&c| self.score_of(c) >= LEARNED_THRESHOLD)
            .unwrap_or(false);

        if newest_learned && self.consecutive_correct >= CONSECUTIVE_CORRECT {
            let next_len = (self.letters_in_play.len() + 1).min(LEARNING_ORDER.len());
            self.letters_in_play = LEARNING_ORDER[..next_len].to_vec();
            self.consecutive_correct = 0;
        }
    }

    /// Rebuild the in-play pool: every learned letter, but always at least the
    /// first [`STARTING_LETTERS`], and never fewer than we already had unlocked.
    fn recompute_letters_in_play(&mut self) {
        let learned_count = LEARNING_ORDER
            .iter()
            .enumerate()
            .filter(|(i, _)| self.scores[*i] >= LEARNED_THRESHOLD)
            .count();

        // Keep the highest letter reached so we never *shrink* the pool after a
        // wrong answer knocks a letter back below the threshold.
        let len = learned_count
            .max(STARTING_LETTERS)
            .max(self.letters_in_play.len())
            .min(LEARNING_ORDER.len());
        self.letters_in_play = LEARNING_ORDER[..len].to_vec();
    }

    /// Pick the next practice word, mirroring the original `findAWord`:
    /// shuffle the pool, take the first word made only of in-play letters, and
    /// — if the newest letter isn't learned yet — require it to contain that
    /// letter so the new letter actually gets drilled.
    fn pick_word(&mut self) -> String {
        let newest = *self.letters_in_play.last().unwrap_or(&'e');
        let newest_learned = self.score_of(newest) >= LEARNED_THRESHOLD;

        for word in self.rng.shuffled(WORDS) {
            let only_in_play = word.chars().all(|c| self.letters_in_play.contains(&c));
            if !only_in_play {
                continue;
            }
            if !newest_learned && !word.contains(newest) {
                continue;
            }
            return word.to_string();
        }
        // Fallback: any word built only from in-play letters, ignoring the
        // "must contain newest" constraint (guarantees we always return one).
        for word in WORDS {
            if word.chars().all(|c| self.letters_in_play.contains(&c)) {
                return word.to_string();
            }
        }
        newest.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_three_letters_and_a_valid_word() {
        let t = Trainer::new(1);
        assert_eq!(t.letters_in_play(), &['e', 't', 'a']);
        assert!(!t.current_word().is_empty());
        assert!(t.current_word().chars().all(|c| ['e', 't', 'a'].contains(&c)));
    }

    #[test]
    fn correct_answer_advances_and_raises_score() {
        let mut t = Trainer::new(1);
        let target = t.target_letter();
        assert_eq!(t.submit(target), Judgement::Correct);
        let idx = super::Trainer::index_of(target).unwrap();
        assert_eq!(t.scores()[idx], 1);
    }

    #[test]
    fn wrong_answer_lowers_score_and_breaks_streak() {
        let mut t = Trainer::new(1);
        // Pick a letter guaranteed to be wrong.
        let wrong = if t.target_letter() == 'e' { 't' } else { 'e' };
        assert_eq!(t.submit(wrong), Judgement::Incorrect);
        let idx = super::Trainer::index_of(t.target_letter()).unwrap();
        assert_eq!(t.scores()[idx], -1);
        assert_eq!(t.consecutive_correct, 0);
    }

    #[test]
    fn scores_are_clamped() {
        let mut t = Trainer::new(1);
        for _ in 0..50 {
            let target = t.target_letter();
            t.submit(target);
        }
        assert!(t.scores().iter().all(|&s| s <= SCORE_LIMIT));
    }

    #[test]
    fn hint_disappears_once_learned() {
        let mut t = Trainer::new(1);
        assert_ne!(t.hint_level(), HintLevel::None); // fresh letter needs a hint
        // Answer the current target correctly a few times across words.
        for _ in 0..200 {
            if t.completion() > 0.1 {
                break;
            }
            let target = t.target_letter();
            t.submit(target);
        }
        // Some letters should now be learned.
        assert!(t.progress().iter().any(|p| p.learned));
    }

    #[test]
    fn always_answering_correctly_eventually_unlocks_letters() {
        let mut t = Trainer::new(12345);
        for _ in 0..500 {
            let target = t.target_letter();
            t.submit(target);
        }
        // Playing perfectly for a while must widen the pool beyond the initial 3.
        assert!(t.letters_in_play().len() > STARTING_LETTERS);
    }

    #[test]
    fn restore_round_trips_scores() {
        let mut t = Trainer::new(9);
        for _ in 0..20 {
            let target = t.target_letter();
            t.submit(target);
        }
        let saved = t.scores();
        let restored = Trainer::restore(saved, 9);
        assert_eq!(restored.scores(), saved);
    }
}
