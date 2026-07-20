# Chapter 4 — The trainer

> **You'll build:** the state machine that scores answers, unlocks letters, and
> picks practice words.
> **You'll learn:** ownership, moves, borrows, `&mut self`, `Vec`, and enums that
> carry meaning.

This is the chapter where Rust stops feeling like a familiar language with odd
syntax. Everything so far has been pure functions over `Copy` data. Now we have
mutable state that several parts of the program want to touch, and that is
exactly what ownership exists to govern.

## Ownership, before we need it

Three rules:

1. Every value has exactly one **owner**.
2. When the owner goes out of scope, the value is dropped.
3. Ownership can be **moved**, or temporarily **borrowed**.

The consequence people hit first:

```rust
let a = String::from("hello");
let b = a;              // ownership MOVES to b
println!("{}", a);      // error: borrow of moved value: `a`
```

`a` is no longer usable. Not because it was freed — because `b` owns the buffer
now, and allowing both names to reach it is how you get double-frees and
use-after-free. Rust makes it a compile error instead.

This did not come up in Chapter 3 because `Timing` is `Copy`: two `u32`s are
duplicated bit-for-bit, so `let b = a` copies and both stay valid. `String` owns
a heap allocation, so it cannot be `Copy` — duplicating it would need a decision
about whether to copy the buffer, and Rust refuses to make that decision
invisibly. `.clone()` is how you ask for it explicitly.

**Borrowing** avoids the move:

```rust
let a = String::from("hello");
let b = &a;             // borrow — a still owns it
println!("{} {}", a, b); // both fine
```

And the rule that governs borrows:

> You may have **either** any number of shared borrows (`&T`) **or** exactly one
> mutable borrow (`&mut T`) — never both at once.

That single rule eliminates data races at compile time, and it is also why
iterator-invalidation bugs cannot occur: you cannot hold a `&` into a `Vec` while
something else holds a `&mut` to push to it.

Keep that rule in mind. It will produce a compile error later in this chapter,
and the error will be *correct*.

## The state

Create `crates/morse-core/src/trainer.rs`:

```rust
//! The trainer state machine.
//!
//! Owns all the learning logic with none of the rendering. A UI layer — Dioxus
//! on web/mobile, or a test — drives it by reading the current target letter and
//! calling `submit(...)`.

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
/// Consecutive mistakes on one letter before its Morse pattern is revealed.
pub const STRUGGLE_MISTAKES: u32 = 3;

/// The result of submitting an answer for the current letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judgement {
    Correct,
    Incorrect,
}

/// How much of a hint the UI should surface for the current letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintLevel {
    /// The letter is already learned — no hint needed.
    None,
    /// Not yet learned: show the mnemonic for the letter.
    Picture,
    /// Struggling: also reveal the Morse pattern.
    Pattern,
}
```

`use crate::alphabet::{self, LEARNING_ORDER}` imports both the module itself
(that is what `self` means here, letting us write `alphabet::to_morse(...)`) and
one item from it directly. `crate::` roots the path at this crate.

### Enums that carry meaning

`Judgement` could have been `bool`. Compare the call sites:

```rust
if trainer.submit('s') { ... }                          // true means... what?
if trainer.submit('s') == Judgement::Correct { ... }    // unambiguous
```

A `bool` return forces every reader to remember the polarity, and nothing stops
you passing it to a function expecting a different `bool`. `Judgement` cannot be
confused with anything else, reads at the call site, and — the part that pays off
later — can grow a third variant with the compiler pointing at every place that
needs updating.

`HintLevel` shows the same idea with three states. Representing it as
`show_hint: bool, show_pattern: bool` would admit the nonsense combination
"pattern but no hint". The enum makes that state unrepresentable — the same
principle as Chapter 3's private fields, applied to a different shape.

**Rust enums are sum types**, closer to a tagged union than to a C enum. They can
carry data per variant (`Option::Some(T)` is exactly this), and `match` over them
is exhaustive.

## The struct

```rust
/// The full trainer state.
#[derive(Debug, Clone)]
pub struct Trainer {
    /// Score per letter, indexed the same as `LEARNING_ORDER`.
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
    /// The word is fully answered and is being shown as complete.
    word_complete: bool,
    rng: Rng,
}
```

No `Copy` this time, and it would not be allowed: `Vec` and `String` own heap
allocations. `Clone` is still fine — it deep-copies.

Three ownership-relevant choices here:

`scores: [i32; 26]` is a fixed array because there are always exactly 26 letters.
No allocation, and it copies cheaply for saving.

`letters_in_play: Vec<char>` grows as the learner unlocks letters, so it must be
a `Vec`.

`current_word: String` — owned, not `&str`. This one is worth dwelling on. A
`&str` would be a *borrow of something else*, and the compiler would demand to
know what that something is and prove it outlives the `Trainer`. Since the word
comes from a `&'static` table we could technically write `&'static str`, but the
moment we wanted a generated or user-supplied word, the type would have to change
and every caller with it. **When a struct needs to hold text over time, own it.**
Borrowed fields in long-lived structs are a common early mistake that leads
straight into lifetime annotations you did not need.

## Supporting cast

Two small modules first. `crates/morse-core/src/rng.rs`:

```rust
//! A tiny, dependency-free pseudo-random generator.
//!
//! We only need randomness to shuffle the word pool, and seeding deterministically
//! makes the trainer reproducible in tests.

/// A minimal xorshift64* pseudo-random generator.
///
/// Not cryptographically secure — do not use for anything security-sensitive.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Any non-zero seed works; zero is nudged to a constant so the generator
    /// never gets stuck at all-zeroes.
    pub fn new(seed: u64) -> Self {
        Rng { state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed } }
    }

    /// Next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Return `items` in a new random order (Fisher–Yates).
    pub fn shuffled<T: Clone>(&mut self, items: &[T]) -> Vec<T> {
        let mut out = items.to_vec();
        for i in (1..out.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            out.swap(i, j);
        }
        out
    }
}
```

Three things to notice. `next_u64` takes **`&mut self`** — it changes `state`, so
it needs a mutable borrow. `wrapping_mul` is explicit about overflow: Rust panics
on integer overflow in debug builds, so when you *want* wraparound you say so.
And `shuffled<T: Clone>` is **generic with a trait bound** — it works for any `T`
that can be cloned, which it must be to build the output `Vec`. Underscores in
`0x9E37_79B9_7F4A_7C15` are just readability; the compiler ignores them.

`crates/morse-core/src/words.rs`:

```rust
//! Practice word pool.

/// All practice words, ordered by which letter each one introduces.
pub const WORDS: &[&str] = &[
    "tee", "at", "eat", "tea", "ate", "tat", "it", "tie", "tit",
    "me", "time", "am", "team", "meet", "met", "ma", "meat", "item",
    "is", "as", "its", "see", "same", "set", "miss", "site", "test",
    "to", "so", "some", "most", "too", "toe", "toast", "moss",
    "the", "that", "this", "he", "has", "she", "them", "home",
    "in", "on", "not", "an", "one", "no", "than", "into", "then",
];
```

> The real `words.rs` has several hundred entries, generated from a word-frequency
> list so that every letter in the learning order has words using only the letters
> introduced so far. This short list is enough to build against.

`&[&str]` is a **slice** of string slices — a borrowed view of a contiguous
sequence. Unlike `[&str; 51]`, the length is not in the type, so you can extend
the list without touching the type.

## Construction

```rust
impl Trainer {
    /// Start a fresh session.
    pub fn new(seed: u64) -> Self {
        Self::restore([0; 26], seed)
    }

    /// Restore from previously saved per-letter scores. Unlocks every learned
    /// letter plus the next one, then picks a first word.
    pub fn restore(scores: [i32; 26], seed: u64) -> Self {
        let mut trainer = Trainer {
            scores,
            letters_in_play: Vec::new(),
            current_word: String::new(),
            letter_index: 0,
            consecutive_correct: 0,
            mistakes_on_letter: 0,
            word_complete: false,
            rng: Rng::new(seed),
        };
        trainer.recompute_letters_in_play();
        trainer.current_word = trainer.pick_word();
        trainer
    }
```

`[0; 26]` is array-repeat syntax: 26 zeros.

`scores,` on its own is **field init shorthand** — the parameter is already named
`scores`, so `scores: scores` shortens.

The last line is `trainer` with **no semicolon**. In Rust the final expression of
a block is its value; `return trainer;` is equivalent but less idiomatic. This
also explains a common early error: adding a semicolon to the last line of a
function changes its value to `()` and produces a type mismatch.

Note the two-phase construction — build, then fix up. `pick_word` needs a
partially-built `Trainer` (it reads `letters_in_play` and mutates `rng`), so it
cannot run before the struct exists.

## Reading state

```rust
    /// The word currently being practised.
    pub fn current_word(&self) -> &str {
        &self.current_word
    }

    /// The specific letter the user must enter right now.
    pub fn target_letter(&self) -> char {
        self.current_word.chars().nth(self.letter_index).unwrap_or(' ')
    }

    /// The Morse pattern for the target letter (e.g. `"..."`).
    pub fn target_morse(&self) -> &'static str {
        alphabet::to_morse(self.target_letter()).unwrap_or("")
    }

    /// Index of the letter to type next.
    pub fn letter_index(&self) -> usize {
        self.letter_index
    }

    /// Which letters are unlocked and in the practice pool.
    pub fn letters_in_play(&self) -> &[char] {
        &self.letters_in_play
    }

    /// The raw per-letter scores, for persistence.
    pub fn scores(&self) -> [i32; 26] {
        self.scores
    }
```

`current_word(&self) -> &str` returns a **borrow of the struct's own data**. The
lifetime is elided but real: the returned `&str` may not outlive the `&self` it
came from. The compiler enforces this, so you cannot hold the string after the
trainer is dropped. Returning `String` here would allocate a copy on every call —
and the UI calls it on every render.

`letters_in_play(&self) -> &[char]` returns a slice rather than `&Vec<char>`. A
slice is the more general type: callers get indexing and iteration without being
told the backing store is a `Vec`. Prefer `&[T]` over `&Vec<T>` in signatures.

`scores(&self) -> [i32; 26]` returns **by value**, copying — arrays of `Copy`
types are `Copy`. 104 bytes is cheap, and the caller gets an independent snapshot
to save.

`chars().nth(i)` is worth a note: `String` is UTF-8, so it cannot be indexed by
integer — `s[3]` does not compile. A byte offset into UTF-8 might land
mid-character, so Rust makes you say what you mean: `.chars()` for characters,
`.bytes()` for bytes. Our words are ASCII, so this is O(n) over a handful of
characters and does not matter.

## Mutation, and the borrow checker's first objection

```rust
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
            self.word_complete = true;
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
```

`&mut self` is an **exclusive borrow**: while it is held, nothing else may touch
the `Trainer` at all — not even to read.

Look at the first two lines of `register_correct`:

```rust
let letter = self.target_letter();
self.bump_score(letter, 1);
```

Why the temporary? Try to inline it:

```rust
self.bump_score(self.target_letter(), 1);   // error!
```

```
error[E0502]: cannot borrow `*self` as immutable because it is also borrowed as mutable
```

`bump_score` needs `&mut self`, and evaluating the argument needs `&self`. That
is a shared and an exclusive borrow at the same moment — precisely the rule from
the start of this chapter. Binding to a local first ends the shared borrow before
the mutable one begins.

This will happen to you regularly at first, and the fix is almost always the same:
pull the read out into a `let`. `char` is `Copy`, so `letter` is an independent
value and no borrow survives.

## Scoring and unlocking

```rust
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
```

`index_of` takes no `self` — an associated function, called `Self::index_of(c)`.
It does not need instance state, so it does not ask for it.

`|&c| c == letter` destructures the reference **in the pattern**, so `c` is a
`char` directly. Equivalent to `|c| *c == letter`. Both are common.

Note `bump_score` silently ignores unknown letters via `if let`. That is
deliberate: a space or punctuation has no score slot, and there is nothing to do
about it. Chapter 5 examines when silent tolerance is right and when it hides a
bug.

```rust
    /// Unlock the next letter when the newest in play is learned and the user
    /// has a streak of at least `CONSECUTIVE_CORRECT`.
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
    /// first `STARTING_LETTERS`, and never fewer than we already had unlocked.
    fn recompute_letters_in_play(&mut self) {
        let learned_count = LEARNING_ORDER
            .iter()
            .enumerate()
            .filter(|(i, _)| self.scores[*i] >= LEARNED_THRESHOLD)
            .count();

        let len = learned_count
            .max(STARTING_LETTERS)
            .max(self.letters_in_play.len())
            .min(LEARNING_ORDER.len());
        self.letters_in_play = LEARNING_ORDER[..len].to_vec();
    }
```

`LEARNING_ORDER[..next_len]` is a **range slice** — a borrowed view of the first
`next_len` elements, no copying. `.to_vec()` then allocates an owned `Vec`, which
we need because we are storing it.

The chained `.max().max().min()` in `recompute_letters_in_play` is doing
something subtle: the pool never shrinks. A wrong answer can knock a letter's
score back below the threshold, and without `.max(self.letters_in_play.len())`
the letter would vanish from practice mid-session. Clamping like this — rather
than an `if` ladder — makes the "never shrink, never exceed 26" intent readable
in one line.

## Word completion

When the last letter of a word is answered, the app wants to show a check-mark
before moving on. That means the trainer must **not** immediately swap the word
out — otherwise there is no state in which the UI can render "you finished this
word".

Hence `word_complete`, set by `register_correct` above, and acknowledged
separately:

```rust
    /// Whether the current word has just been completed. While this is set there
    /// is no target letter: the UI shows the finished word until it calls
    /// `next_word`.
    pub fn word_complete(&self) -> bool {
        self.word_complete
    }

    /// Acknowledge a completed word: unlock a new letter if one was earned, then
    /// pick the next word. Does nothing unless `word_complete` is set.
    pub fn next_word(&mut self) {
        if !self.word_complete {
            return;
        }
        self.maybe_unlock_letter();
        self.recompute_letters_in_play();
        self.current_word = self.pick_word();
        self.letter_index = 0;
        self.mistakes_on_letter = 0;
        self.word_complete = false;
    }
```

This is a small but real lesson in API design: **a state machine should expose
the states its consumers need to render.** The obvious implementation advances
immediately and is impossible to animate. Splitting one transition into
"complete" and "acknowledged" costs one `bool` and makes the UI's job trivial.

Now guard `submit` so a stray answer during that pause cannot corrupt the streak:

```rust
    pub fn submit(&mut self, answer: char) -> Judgement {
        self.next_word();   // an answer implicitly acknowledges a finished word
        if answer.to_ascii_lowercase() == self.target_letter() {
            self.register_correct()
        } else {
            self.register_incorrect()
        }
    }
```

Without this, an answer arriving while `word_complete` is set would be compared
against `target_letter()`, which returns `' '` — scoring a miss, and resetting
`consecutive_correct`, silently stealing a letter unlock the learner had earned.

## Hints

```rust
    /// The hint the UI should show for the current letter.
    ///
    /// `STRUGGLE_MISTAKES` consecutive misses reveal the pattern *even if the
    /// letter was already learned* — a learned letter can still go stale, and
    /// that is exactly when the reminder is worth most.
    pub fn hint_level(&self) -> HintLevel {
        if self.mistakes_on_letter >= STRUGGLE_MISTAKES {
            HintLevel::Pattern
        } else if self.score_of(self.target_letter()) >= LEARNED_THRESHOLD {
            HintLevel::None
        } else {
            HintLevel::Picture
        }
    }
```

Order matters here, and getting it wrong is a real bug this project shipped. If
the learned check comes first, a learned letter returns `None` no matter how many
times it is missed — so the letters most in need of help get the least. Put the
struggle check first.

## Picking words

```rust
    /// Pick the next practice word: shuffle the pool, take the first word made
    /// only of in-play letters, and — if the newest letter isn't learned yet —
    /// require it to contain that letter so the new letter actually gets drilled.
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
        // Fallback: any word built only from in-play letters.
        for word in WORDS {
            if word.chars().all(|c| self.letters_in_play.contains(&c)) {
                return word.to_string();
            }
        }
        newest.to_string()
    }
}
```

`*self.letters_in_play.last().unwrap_or(&'e')` reads awkwardly but each piece is
doing a job: `last()` gives `Option<&char>`, `unwrap_or(&'e')` supplies a
reference as the default (it must match the type), and `*` dereferences to a
`char`. The alternative `.copied().unwrap_or('e')` is arguably tidier — try it.

The two-stage fallback matters. The first loop may find nothing: early on, the
pool is `['e','t','a']` and we require a word containing the newest letter, which
might not exist in a short list. The second loop drops that constraint. The final
`newest.to_string()` guarantees a `String` comes back no matter what. **The
function cannot fail**, which is why it returns `String` rather than
`Option<String>` — worth doing deliberately, because a trainer with no word is a
dead trainer.

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Answer the current target correctly, acknowledging a finished word first
    /// — the same two-step the UI performs around the completion check-mark.
    fn answer_target(t: &mut Trainer) -> Judgement {
        t.next_word();
        let target = t.target_letter();
        t.submit(target)
    }

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
        let idx = Trainer::index_of(target).unwrap();
        assert_eq!(t.scores()[idx], 1);
    }

    #[test]
    fn wrong_answer_lowers_score_and_breaks_streak() {
        let mut t = Trainer::new(1);
        let wrong = if t.target_letter() == 'e' { 't' } else { 'e' };
        assert_eq!(t.submit(wrong), Judgement::Incorrect);
        let idx = Trainer::index_of(t.target_letter()).unwrap();
        assert_eq!(t.scores()[idx], -1);
        assert_eq!(t.consecutive_correct, 0);
    }

    #[test]
    fn finishing_a_word_holds_before_advancing() {
        let mut t = Trainer::new(1);
        let word = t.current_word().to_string();

        for _ in 0..word.chars().count() {
            assert!(!t.word_complete(), "should not complete mid-word");
            answer_target(&mut t);
        }

        assert!(t.word_complete());
        assert_eq!(t.current_word(), word);

        t.next_word();
        assert!(!t.word_complete());
        assert_eq!(t.letter_index(), 0);
    }

    #[test]
    fn three_mistakes_in_a_row_reveal_the_pattern() {
        let mut t = Trainer::new(1);
        let wrong = if t.target_letter() == 'e' { 't' } else { 'e' };
        t.submit(wrong);
        t.submit(wrong);
        assert_ne!(t.hint_level(), HintLevel::Pattern);
        t.submit(wrong);
        assert_eq!(t.hint_level(), HintLevel::Pattern);
    }

    #[test]
    fn always_answering_correctly_eventually_unlocks_letters() {
        let mut t = Trainer::new(12345);
        for _ in 0..500 {
            answer_target(&mut t);
        }
        assert!(t.letters_in_play().len() > STARTING_LETTERS);
    }
}
```

Two things to note.

`wrong_answer_lowers_score_and_breaks_streak` reads `t.consecutive_correct` — a
**private field**. This works because the test module is a child of `trainer`.
This is the unit-test privilege from Chapter 1, used exactly as intended: assert
on internal state that no outside caller can see, without making it `pub`.

`let word = t.current_word().to_string()` in the completion test needs the
`to_string()`. `current_word()` returns a `&str` borrowed from `t`, and the loop
below takes `&mut t` — a shared borrow alive across a mutable borrow, which the
rule forbids. Copying to an owned `String` ends the borrow. Delete `.to_string()`
and read the error; it is the clearest demonstration of the borrow rule in this
chapter.

`always_answering_correctly_eventually_unlocks_letters` uses a fixed seed, so the
`Rng` makes it fully deterministic. A test that shuffles randomly and sometimes
fails is worse than no test.

## Wire it up

```rust
pub mod alphabet;
pub mod rng;
pub mod timing;
pub mod trainer;
pub mod words;

pub use alphabet::{from_morse, to_morse, LEARNING_ORDER};
pub use timing::{Timing, MAX_WPM, MIN_WPM};
pub use trainer::{HintLevel, Judgement, Trainer};
pub use words::WORDS;
```

## Checkpoint

```sh
cargo test -p morse-core
```

Around 16 tests passing. You have a complete, tested learning engine — no UI,
no browser, no platform. Everything from here builds on this.

## Exercises

1. *(compiler)* In `register_correct`, replace the two lines with
   `self.bump_score(self.target_letter(), 1);`. Read the E0502 error in full.
   It names both borrows and points at each. Revert.

2. *(compiler)* In `finishing_a_word_holds_before_advancing`, drop the
   `.to_string()`. The error is E0502 again, from the other direction: a shared
   borrow held across a mutable one. Same rule, different shape.

3. *(build)* Add `progress(&self) -> Vec<LetterProgress>` returning per-letter
   score and learned-status, for the UI's progress lights. Define
   `LetterProgress { letter: char, score: i32, learned: bool }`. Hint:
   `LEARNING_ORDER.iter().enumerate().map(...).collect()`.

4. *(build)* Add `completion(&self) -> f32`, the fraction of letters learned.
   Watch the integer-to-float cast — `count()` gives `usize`.

5. *(think)* `submit` calls `next_word()` first, so answering during the
   completion pause applies to the *next* word. The alternative is to ignore the
   input entirely. Which is less surprising for a learner hammering the key? What
   does the UI have to do differently under each choice? (Chapter 6 shows what
   the real app does — it guards in the UI *and* keeps this backstop.)

---

Next: [Chapter 5 — No unwrap()](05-option-and-errors.md), on `Option`
combinators, `Result`, and deciding when a panic is the correct behaviour.
