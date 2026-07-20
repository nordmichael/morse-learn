# Chapter 8 — Audio and keying

> **You'll build:** Morse audio, straight-key input, and the adaptive speed coach.
> **You'll learn:** implementing traits on types from another module, `VecDeque`,
> `std::mem::take`, thread-locals, and the pure-schedule pattern.

Morse is a *sound* skill. Everything so far has been silent, and a silent Morse
trainer teaches the wrong thing — you learn to count dots instead of recognising
rhythms.

## Separating the schedule from the sound

The temptation is to write a `play_letter` that calls Web Audio directly. Resist
it, for the same reason as Chapter 1: that function would be untestable and
web-only.

Instead, split it. `morse-core` computes **when the beeps happen**; the platform
plays them. Create `crates/morse-core/src/audio.rs`:

```rust
//! Audio schedule generation.
//!
//! Turns a Morse string into a list of tone bursts: when each beep starts and
//! how long it lasts. Contains no audio code — the platform layer feeds this to
//! Web Audio on the web, or a native tone generator on mobile. Keeping the
//! schedule pure means the exact same rhythm plays everywhere and can be
//! unit-tested with no sound card.

use crate::timing::Timing;

/// A single beep: a tone starting at `start_ms` after playback begins, lasting
/// `len_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tone {
    pub start_ms: u32,
    pub len_ms: u32,
}

/// Build the tone schedule for a single character's Morse code (e.g. `"-.-"`).
/// Unknown characters in `code` are ignored.
pub fn schedule_code(code: &str, timing: &Timing) -> Vec<Tone> {
    let mut tones = Vec::new();
    let mut cursor = 0u32;
    let mut first = true;
    for symbol in code.chars() {
        let len = match symbol {
            '.' => timing.dit_ms(),
            '-' => timing.dah_ms(),
            _ => continue,
        };
        if !first {
            cursor += timing.symbol_gap_ms(); // gap before this symbol
        }
        tones.push(Tone { start_ms: cursor, len_ms: len });
        cursor += len;
        first = false;
    }
    tones
}
```

Here `Tone`'s fields *are* `pub`. Contrast with `Timing` in Chapter 3, whose
fields were private to protect an invariant. `Tone` has no invariant — it is a
plain data carrier, two independent numbers. **Make fields public when the type
is data; keep them private when the type has rules.**

`match` as an expression assigns `len` directly, and `continue` inside a match arm
skips unknown characters. That is `match` doing control flow and value production
at once, which is idiomatic and worth getting comfortable with.

Testing this needs no browser:

```rust
    #[test]
    fn letter_a_is_dit_gap_dah() {
        let t = Timing::new(20); // dit 60, dah 180, gap 60
        let tones = schedule_code(".-", &t);
        assert_eq!(
            tones,
            vec![
                Tone { start_ms: 0, len_ms: 60 },    // dit
                Tone { start_ms: 120, len_ms: 180 }, // gap(60) after dit(60) -> dah
            ]
        );
    }
```

That is the payoff. The exact rhythm of every letter is asserted in a test that
runs in microseconds. Verifying this by ear would be slow and unreliable.

## Extending a type from another module

Straight-key input needs to classify a press by duration. That is timing logic,
but it belongs conceptually with keying. Rust lets us have both — create
`crates/morse-core/src/keyer.rs`:

```rust
//! Keying: turning key presses into Morse symbols.

use crate::timing::Timing;

/// A single Morse element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    Dit,
    Dah,
}

impl Symbol {
    /// The character used in Morse strings (`.` or `-`).
    pub fn as_char(self) -> char {
        match self {
            Symbol::Dit => '.',
            Symbol::Dah => '-',
        }
    }
}

/// What a silence between presses means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gap {
    /// Short silence: still inside the same character.
    Symbol,
    /// Medium silence: the character is complete.
    Character,
    /// Long silence: a word break.
    Word,
}

impl Timing {
    /// Classify a straight-key press by its duration. The dit/dah boundary sits
    /// at two dits (halfway between a 1-unit dit and a 3-unit dah).
    pub fn classify_press(&self, duration_ms: u32) -> Symbol {
        if duration_ms < 2 * self.dit_ms() {
            Symbol::Dit
        } else {
            Symbol::Dah
        }
    }

    /// Classify a silence into symbol / character / word gaps.
    pub fn classify_gap(&self, duration_ms: u32) -> Gap {
        let unit = self.dit_ms();
        if duration_ms < 2 * unit {
            Gap::Symbol
        } else if duration_ms < 5 * unit {
            Gap::Character
        } else {
            Gap::Word
        }
    }
}
```

Look at that second `impl Timing` — in `keyer.rs`, not `timing.rs`. **You may add
methods to a type anywhere in the same crate.** This is not monkey-patching:
these methods can only touch `Timing`'s *public* API, and they are only visible
where `Timing` is in scope.

It is the right call here because these methods are about keying, not about
timing. Putting them in `timing.rs` would mean `timing.rs` had to know what a
`Symbol` is, dragging a dependency the wrong way. Cross-crate, the orphan rule
from Chapter 6 would forbid this, and you would use an extension trait instead.

`as_char(self)` takes `self` **by value**, not `&self`, because `Symbol` is
`Copy` and two bytes. For small `Copy` types, by-value is simpler and no slower.

`Symbol::Dit`/`Dah` rather than `bool` is the Chapter 4 lesson again: at a call
site, `Symbol::Dah` says what it means and `true` does not.

## Accumulating symbols

```rust
/// Accumulates symbols into the Morse code for the character being entered.
#[derive(Debug, Default, Clone)]
pub struct Keyer {
    current: String,
}

impl Keyer {
    pub fn new() -> Self {
        Keyer::default()
    }

    /// The Morse entered so far for the in-progress character (e.g. `"-."`).
    pub fn current(&self) -> &str {
        &self.current
    }

    /// Append a symbol directly. Use this for two-key/paddle input.
    pub fn push(&mut self, symbol: Symbol) {
        self.current.push(symbol.as_char());
    }

    /// Append a symbol decided by a straight-key press duration.
    pub fn press(&mut self, duration_ms: u32, timing: &Timing) {
        self.push(timing.classify_press(duration_ms));
    }

    /// Report a silence. Returns `Some(code)` (and clears the buffer) when the
    /// gap is long enough to end the character; `None` while still in progress.
    pub fn gap(&mut self, duration_ms: u32, timing: &Timing) -> Option<String> {
        match timing.classify_gap(duration_ms) {
            Gap::Symbol => None,
            Gap::Character | Gap::Word => self.take(),
        }
    }

    /// Take the current character's code and clear the buffer. `None` if empty.
    pub fn take(&mut self) -> Option<String> {
        if self.current.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.current))
        }
    }
}
```

Two things worth stopping on.

**`#[derive(Default)]`** generates a `Default` impl using each field's default —
here, an empty `String`. `Keyer::new()` just calls it.

**`std::mem::take`** is the interesting one. We want to return the `String` and
leave the field empty. We cannot move out of `&mut self` — that would leave the
struct with a hole, and Rust does not allow partially-moved structs behind a
reference.

`mem::take` swaps the value out and puts `Default::default()` in its place,
returning the original. One move, no allocation, no clone:

```rust
let code = std::mem::take(&mut self.current);  // self.current is now ""
```

The naive alternative, `self.current.clone()` followed by `self.current.clear()`,
allocates a whole second string for no reason. `mem::take`, `mem::swap`, and
`mem::replace` are the standard tools for moving values out from behind a mutable
reference, and they come up constantly once you notice them.

`Gap::Character | Gap::Word => self.take()` matches **either** variant with one
arm. Both end the character; only the meaning of the following silence differs.

## Playing it in the browser

Now the platform half, in `platform.rs`:

```rust
/// Sidetone frequency in Hz — a comfortable CW pitch.
const TONE_HZ: f32 = 600.0;

#[cfg(target_arch = "wasm32")]
thread_local! {
    // One AudioContext is reused for the whole app (browsers cap how many can
    // exist); created lazily on first sound and resumed on each use.
    static AUDIO_CTX: std::cell::RefCell<Option<web_sys::AudioContext>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn audio_ctx() -> Option<web_sys::AudioContext> {
    AUDIO_CTX.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = web_sys::AudioContext::new().ok();
        }
        if let Some(ctx) = slot.as_ref() {
            let _ = ctx.resume(); // browsers may start it suspended
        }
        slot.clone()
    })
}
```

`thread_local!` declares per-thread global state. Wasm is single-threaded, so
this is effectively a global — but it is the *safe* way to have one, because Rust
will not let you have mutable statics without `unsafe`.

`RefCell<T>` provides **interior mutability**: mutation through a shared
reference, with the borrow rules checked at *runtime* instead of compile time.
`borrow_mut()` panics if something already holds a borrow.

That runtime check is the trade. We genuinely need one shared, lazily-created
`AudioContext` — browsers limit how many can exist, and creating one per beep
exhausts the limit within a minute of practice. `RefCell` is how you express
that in safe Rust, and the cost is that a borrow mistake becomes a runtime panic.
Chapter 9 shows one of those panics in the wild.

The playback itself:

```rust
/// Play a single character's Morse code with the given timing.
#[cfg(target_arch = "wasm32")]
pub fn play_code(code: &str, timing: morse_core::Timing) {
    use morse_core::schedule_code;

    let tones = schedule_code(code, &timing);
    if tones.is_empty() {
        return;
    }
    let Some(ctx) = audio_ctx() else {
        return;
    };

    let start = ctx.current_time();
    let osc = ctx.create_oscillator().expect("oscillator");
    osc.set_type(web_sys::OscillatorType::Sine);
    osc.frequency().set_value(TONE_HZ);

    let gain = ctx.create_gain().expect("gain");
    gain.gain().set_value(0.0);
    let _ = osc.connect_with_audio_node(&gain);
    let _ = gain.connect_with_audio_node(&ctx.destination());

    // 5ms attack/release keeps the keying click-free.
    let edge = 0.005;
    let param = gain.gain();
    for tone in &tones {
        let on = start + tone.start_ms as f64 / 1000.0;
        let off = on + tone.len_ms as f64 / 1000.0;
        let _ = param.set_value_at_time(0.0, (on - edge).max(start));
        let _ = param.linear_ramp_to_value_at_time(0.6, on + edge);
        let _ = param.set_value_at_time(0.6, (off - edge).max(on + edge));
        let _ = param.linear_ramp_to_value_at_time(0.0, off);
    }

    let total = tones.last().map(|t| t.start_ms + t.len_ms).unwrap_or(0);
    let _ = osc.start();
    let _ = osc.stop_with_when(start + total as f64 / 1000.0 + 0.05);
}
```

The whole schedule is submitted to Web Audio *up front*, timestamped. We do not
wake up per beep — the audio thread handles it with far better timing accuracy
than any JavaScript timer could.

The 5ms ramps matter more than they look. Switching a sine wave on instantly
produces a broadband click, which at practice volume is genuinely unpleasant over
a long session. Ramping the gain over 5ms removes it. This is standard practice
in audio synthesis and a good example of domain knowledge that no amount of Rust
skill substitutes for.

`.expect("oscillator")` is Chapter 5's rule: if the browser gave us a context and
then refuses an oscillator, that is unrecoverable and worth a clear panic. The
surrounding `let _ =` calls are the opposite — a failed parameter automation
degrades the sound slightly and is not worth crashing over.

## The adaptive coach

One more piece of pure logic. Create `crates/morse-core/src/coach.rs`:

```rust
//! Adaptive speed coaching.
//!
//! Good practice keeps you at the edge of your ability: speed up when you're
//! comfortably accurate, ease off when you start missing.

use std::collections::VecDeque;

/// What the coach suggests after the latest answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedAdvice {
    Hold,
    Faster,
    Slower,
}

/// Rolling-accuracy speed coach.
#[derive(Debug, Clone)]
pub struct SpeedCoach {
    window: VecDeque<bool>,
    capacity: usize,
    raise_at: f32,
    lower_at: f32,
}

impl Default for SpeedCoach {
    fn default() -> Self {
        // Judge over the last 10 answers: speed up at >=90% accurate, slow down
        // at <=50%. Responsive but not twitchy.
        SpeedCoach::new(10, 0.9, 0.5)
    }
}

impl SpeedCoach {
    pub fn new(capacity: usize, raise_at: f32, lower_at: f32) -> Self {
        SpeedCoach {
            window: VecDeque::with_capacity(capacity.max(1)),
            capacity: capacity.max(1),
            raise_at,
            lower_at,
        }
    }

    /// Record an answer and get the resulting advice. Advice other than `Hold`
    /// is only returned once the window is full; after a change the window is
    /// cleared so the next one needs fresh evidence.
    pub fn record(&mut self, correct: bool) -> SpeedAdvice {
        self.window.push_back(correct);
        if self.window.len() > self.capacity {
            self.window.pop_front();
        }
        if self.window.len() < self.capacity {
            return SpeedAdvice::Hold;
        }

        let hits = self.window.iter().filter(|&&c| c).count();
        let accuracy = hits as f32 / self.capacity as f32;
        if accuracy >= self.raise_at {
            self.window.clear();
            SpeedAdvice::Faster
        } else if accuracy <= self.lower_at {
            self.window.clear();
            SpeedAdvice::Slower
        } else {
            SpeedAdvice::Hold
        }
    }
}
```

`VecDeque` is a double-ended queue — O(1) push and pop at *both* ends, which a
`Vec` cannot offer at the front. A rolling window pushes to the back and pops from
the front, so this is exactly the right structure.

`impl Default for SpeedCoach` is written **by hand**, not derived, because the
defaults are meaningful policy (10 answers, 90%, 50%) rather than zeroes. Deriving
would give a zero-capacity coach. When defaults carry meaning, write them out and
comment the reasoning.

`filter(|&&c| c)` has two `&`s because `iter()` over a `VecDeque<bool>` yields
`&bool` and `filter`'s closure receives `&&bool`. Pattern-matching both away gives
a plain `bool`. If that looks odd, `filter(|c| **c)` is the same thing.

Clearing the window after a change is a small but important design decision:
without it, one good streak would ramp the speed up on every subsequent answer,
because the window would stay above threshold. Requiring fresh evidence makes the
ramp stable.

## Wire it up

```rust
pub mod alphabet;
pub mod audio;
pub mod coach;
pub mod keyer;
pub mod rng;
pub mod timing;
pub mod trainer;
pub mod words;

pub use audio::{schedule_code, schedule_text, Tone};
pub use coach::{SpeedAdvice, SpeedCoach};
pub use keyer::{Gap, Keyer, Symbol};
// ...and the earlier re-exports
```

## Checkpoint

```sh
cargo test -p morse-core
dx serve
```

Around 30 tests. The app makes sound, and every rhythm it produces is verified by
a test that never opens an audio device.

## Exercises

1. *(build)* Add `schedule_text(text: &str, timing: &Timing) -> Vec<Tone>` for
   whole words, inserting character gaps between letters and word gaps at spaces.
   Test that `"ee"` produces two tones separated by a character gap.

2. *(build)* Add `total_duration_ms(tones: &[Tone]) -> u32`. Note it takes a
   slice, not a `Vec` — the Chapter 4 signature guidance.

3. *(compiler)* In `Keyer::take`, replace `std::mem::take(&mut self.current)`
   with `self.current`. Read the error about moving out of borrowed content, then
   try `self.current.clone()` — that compiles. Explain what it costs.

4. *(build)* Add a sidetone that sounds while a straight key is held: `tone_on()`
   and `tone_off()`, storing the live oscillator in a second `thread_local!`.
   Make `tone_on` idempotent — a second call while sounding should do nothing.

5. *(think)* `schedule_code` returns `Vec<Tone>`, allocating on every call, and
   it is called on every audio prompt. Is that worth optimising? What would you
   measure first, and what would you change if the measurement said yes?

---

Next: [Chapter 9 — Real bugs](09-real-bugs.md), where two bugs from this
codebase's actual history show what the compiler cannot catch.
