# Chapter 3 — Timing

> **You'll build:** WPM-based Morse timing, including Farnsworth spacing.
> **You'll learn:** structs, `impl` blocks, methods, `derive`, privacy as
> invariant-enforcement, and when to use integers vs floats.

## The problem

Morse speed is measured in words per minute, but everything the app actually
does needs milliseconds: how long to hold a tone, how long a silence means
"letter finished", how long a press must be to count as a dash.

The standard uses the word PARIS, which is exactly 50 dit-units long:

| Element | Units |
|---------|-------|
| dit (dot) | 1 |
| dah (dash) | 3 |
| gap between symbols in a letter | 1 |
| gap between letters | 3 |
| gap between words | 7 |

At *W* words per minute you send *W* PARIS-lengths in 60 seconds, so one dit is
`60 / (50 × W)` seconds — that is, **`1200 / W` milliseconds**. At 20 WPM a dit
is 60ms.

Then there is a wrinkle that matters for teaching. A beginner at 5 WPM hears
each letter drawn out into a slow drone, and learns the *wrong thing*: they learn
to count dots rather than recognise a rhythm. Speeding them up later means
unlearning it.

**Farnsworth timing** fixes this: send each character at a fast speed — a real,
recognisable rhythm — but stretch the gaps *between* characters so the learner
still has thinking time. Characters at 18 WPM with an effective pace of 8 WPM
gives you real 18 WPM letters with long pauses. As you improve you close the gaps
rather than speeding up the letters, so the rhythm you learned on day one is the
rhythm you use at full speed.

That is the logic this chapter builds.

## A struct with private fields

Create `crates/morse-core/src/timing.rs`:

```rust
//! Morse timing: everything about *speed*.
//!
//! All durations derive from a words-per-minute setting. Pure integer-
//! millisecond logic, shared by audio playback and key decoding alike.

/// Slowest speed the UI should allow.
pub const MIN_WPM: u32 = 5;
/// Fastest speed the UI should allow (the learner's target ceiling).
pub const MAX_WPM: u32 = 40;

/// A resolved timing: the character speed plus the (equal or slower) effective
/// speed used for gap padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    char_wpm: u32,
    effective_wpm: u32,
}
```

Note what is missing: the fields are **not `pub`**. Outside this module, nobody
can read or write `char_wpm`. That is deliberate, and it is the central design
idea of this chapter — we will come back to it once the constructors are in place.

### `derive`, and what those four traits mean

`#[derive(...)]` asks the compiler to write trait implementations for you. The
four here are the standard set for a small value type, and each does real work:

**`Debug`** enables `{:?}` formatting. Without it, `println!("{:?}", timing)`
does not compile, and — more painfully — `assert_eq!` cannot print the values
when a test fails. Derive `Debug` on essentially everything.

**`Clone`** allows explicit duplication via `.clone()`.

**`Copy`** changes assignment semantics entirely. Normally, assigning a value
*moves* it and the original becomes unusable (Chapter 4 covers why). A `Copy`
type is duplicated instead, and both remain valid:

```rust
let a = Timing::new(20);
let b = a;          // copies
println!("{:?}", a); // still fine — without Copy this would not compile
```

`Copy` is only available for types that are cheap to duplicate bit-for-bit: no
heap allocation, no destructor. Two `u32`s qualify. A `String` never can. Making
`Timing` `Copy` means we can pass it around freely without `&` or `.clone()`
noise, which matters because it gets passed *everywhere*.

**`PartialEq`/`Eq`** enable `==`. The split between them is a real distinction
worth knowing: `PartialEq` is equality that may not be reflexive, and it exists
because of floating point — `f64::NAN != f64::NAN`, so `f64` is `PartialEq` but
not `Eq`. Our fields are `u32`, which is properly reflexive, so we get both.
`assert_eq!` needs `PartialEq`.

## Constructors and the invariant

```rust
impl Timing {
    /// Standard timing at `wpm` (character speed == effective speed).
    /// `wpm` is clamped to `[MIN_WPM, MAX_WPM]`.
    pub fn new(wpm: u32) -> Self {
        let w = wpm.clamp(MIN_WPM, MAX_WPM);
        Timing { char_wpm: w, effective_wpm: w }
    }

    /// Farnsworth timing: characters sent at `char_wpm`, gaps stretched so the
    /// overall pace feels like `effective_wpm`. If `effective_wpm >= char_wpm`
    /// this is just standard timing.
    pub fn farnsworth(char_wpm: u32, effective_wpm: u32) -> Self {
        let c = char_wpm.clamp(MIN_WPM, MAX_WPM);
        let e = effective_wpm.clamp(MIN_WPM, c);
        Timing { char_wpm: c, effective_wpm: e }
    }
}
```

An `impl` block holds a type's functions. It is separate from the struct
definition, and you may have several — including, as Chapter 8 shows, in a
different file.

**`Self`** (capital S) means "the type this `impl` is for". `-> Self` and
`-> Timing` are identical here; `Self` survives a rename.

Neither function takes `self`, which makes them **associated functions** rather
than methods — called as `Timing::new(20)`. Rust has no dedicated constructor
syntax and no `new` keyword; `new` is a naming convention, nothing more. You are
free to have several constructors with meaningful names, which is exactly what we
have done.

### Why the fields are private

Look closely at `farnsworth`:

```rust
let c = char_wpm.clamp(MIN_WPM, MAX_WPM);
let e = effective_wpm.clamp(MIN_WPM, c);
```

The second clamp uses `c` as its upper bound, not `MAX_WPM`. This enforces an
**invariant**: `effective_wpm` is never greater than `char_wpm`. Farnsworth
timing only makes sense as "characters fast, gaps slow"; the reverse is
meaningless, and — as you will see below — would produce a negative duration.

Because the fields are private, **there is no way to construct a `Timing` that
violates this**. Every path in goes through `new` or `farnsworth`, and both
clamp. The rest of the module can then rely on `effective_wpm <= char_wpm`
without checking, because it is guaranteed by construction.

Had we written `pub char_wpm: u32`, any caller could do
`t.effective_wpm = 9999` and every downstream calculation would have to defend
itself. This is the core value of Rust's privacy: not hiding, but making illegal
states *unrepresentable*.

Since the fields are private, expose reads:

```rust
    /// The character speed in WPM.
    pub fn char_wpm(&self) -> u32 {
        self.char_wpm
    }

    /// The effective (overall) speed in WPM.
    pub fn effective_wpm(&self) -> u32 {
        self.effective_wpm
    }
```

A method and a field may share a name — different namespaces, no conflict. This
reads naturally and is common in Rust.

`&self` is a **shared borrow**: the method can read but not modify, and the
caller keeps ownership. Chapter 4 introduces `&mut self`. The three receiver
forms are `&self` (read), `&mut self` (modify), and `self` (consume).

## Deriving the durations

```rust
    /// Duration of one dit (dot), in milliseconds. The base unit.
    pub fn dit_ms(&self) -> u32 {
        1200 / self.char_wpm
    }

    /// Duration of one dah (dash) — three dits.
    pub fn dah_ms(&self) -> u32 {
        3 * self.dit_ms()
    }

    /// Gap between symbols within a character — one dit. Always at character
    /// speed (Farnsworth never stretches intra-character spacing).
    pub fn symbol_gap_ms(&self) -> u32 {
        self.dit_ms()
    }
```

`1200 / self.char_wpm` is **integer division** — it truncates. At 7 WPM,
`1200 / 7` is 171, not 171.43. For audio timing that is inaudible, and integers
keep the whole pipeline exact and comparison-friendly. Note this cannot divide by
zero: `char_wpm` is clamped to at least `MIN_WPM`, another invariant paying off.

Notice `dah_ms` and `symbol_gap_ms` are defined *in terms of* `dit_ms` rather
than recomputing from WPM. One definition of the base unit, no chance of drift.

## Where floats earn their place

Farnsworth spacing needs real arithmetic:

```rust
    /// Gap between characters — 3 dits at full speed, stretched under Farnsworth.
    pub fn char_gap_ms(&self) -> u32 {
        (3.0 * self.farnsworth_unit_ms()).round() as u32
    }

    /// Gap between words — 7 dits at full speed, stretched under Farnsworth.
    pub fn word_gap_ms(&self) -> u32 {
        (7.0 * self.farnsworth_unit_ms()).round() as u32
    }

    /// The length of one *spacing* unit in ms. Equal to a dit at standard speed;
    /// larger when Farnsworth padding is active.
    fn farnsworth_unit_ms(&self) -> f64 {
        let c = self.char_wpm as f64;
        let e = self.effective_wpm as f64;
        // Time (seconds) the 31 character-units of PARIS occupy at char speed.
        let char_time = 31.0 * 1.2 / c;
        // Total time (seconds) one PARIS word may take at effective speed.
        let total_time = 60.0 / e;
        // Remaining time is shared over PARIS's 19 spacing units.
        let spacing = total_time - char_time;
        let unit_s = if spacing > 0.0 {
            spacing / 19.0
        } else {
            1.2 / c // effective >= char: fall back to standard spacing
        };
        unit_s * 1000.0
    }
```

The arithmetic: PARIS's 50 units split into 31 units of actual character
elements and 19 units of spacing. Send the characters at speed `c`, allow the
whole word to take `60/e` seconds, and whatever time is left over gets shared
across those 19 spacing units. That stretched unit is what the character and word
gaps are built from.

Three things worth pulling out:

**`fn` without `pub`.** `farnsworth_unit_ms` is an implementation detail —
callers want gaps in milliseconds, not spacing units. Private by default means
you can change or delete it freely.

**`as f64` is an explicit cast.** Rust performs *no* implicit numeric
conversions. `self.char_wpm as f64` is required; without it, mixing `u32` and
`f64` is a type error. Verbose, but there is no silently-lost precision.

**The `if spacing > 0.0` guard.** When `effective_wpm == char_wpm` there is no
padding to distribute, and floating-point rounding could make `spacing` a hair
negative — which would produce negative gaps and, downstream, a panic when cast
to `u32`. The fallback returns the standard dit-based unit. The invariant
`effective_wpm <= char_wpm` means `spacing` can never be *substantially*
negative; this guard handles the boundary case.

Integers where exactness matters, floats where ratios do, converting once at the
edge. That is the pattern.

## Tests that encode the standard

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dit_length_matches_the_paris_standard() {
        assert_eq!(Timing::new(20).dit_ms(), 60); // 1200/20
        assert_eq!(Timing::new(40).dit_ms(), 30); // ceiling
        assert_eq!(Timing::new(5).dit_ms(), 240);
    }

    #[test]
    fn wpm_is_clamped_to_the_supported_range() {
        assert_eq!(Timing::new(1000).char_wpm(), MAX_WPM);
        assert_eq!(Timing::new(0).char_wpm(), MIN_WPM);
    }

    #[test]
    fn standard_timing_has_the_classic_ratios() {
        let t = Timing::new(20);
        assert_eq!(t.dah_ms(), 3 * t.dit_ms());
        assert_eq!(t.symbol_gap_ms(), t.dit_ms());
        assert_eq!(t.char_gap_ms(), 3 * t.dit_ms());
        assert_eq!(t.word_gap_ms(), 7 * t.dit_ms());
    }

    #[test]
    fn farnsworth_stretches_gaps_but_not_symbols() {
        // Characters at 20 wpm, overall pace only 8 wpm.
        let t = Timing::farnsworth(20, 8);
        let std = Timing::new(20);
        // A dit is unchanged (still character speed)...
        assert_eq!(t.dit_ms(), std.dit_ms());
        assert_eq!(t.symbol_gap_ms(), std.symbol_gap_ms());
        // ...but the gaps between characters and words are noticeably longer.
        assert!(t.char_gap_ms() > std.char_gap_ms());
        assert!(t.word_gap_ms() > std.word_gap_ms());
    }

    #[test]
    fn farnsworth_collapses_to_standard_when_effective_equals_char() {
        let f = Timing::farnsworth(18, 18);
        let s = Timing::new(18);
        assert_eq!(f.char_gap_ms(), s.char_gap_ms());
        assert_eq!(f.word_gap_ms(), s.word_gap_ms());
    }
}
```

These are worth studying as tests, not just as Rust.

`standard_timing_has_the_classic_ratios` asserts *relationships* rather than
magic numbers. It stays true if someone changes the WPM in the test.

`farnsworth_stretches_gaps_but_not_symbols` captures the entire point of the
feature in four assertions: symbols unchanged, gaps longer. If a refactor ever
stretched the dits, this fails immediately — and that bug would otherwise be
subtle, audible only as "the letters sound wrong somehow".

`farnsworth_collapses_to_standard_when_effective_equals_char` pins the boundary
case, which is exactly where the `if spacing > 0.0` fallback lives. Boundary
conditions are where the bugs are; test them by name.

## Wire it up

`lib.rs`:

```rust
pub mod alphabet;
pub mod timing;

pub use alphabet::{from_morse, to_morse, LEARNING_ORDER};
pub use timing::{Timing, MAX_WPM, MIN_WPM};
```

## Checkpoint

```sh
cargo test -p morse-core
```

```
test result: ok. 9 passed; 0 failed
```

Four from Chapter 2, five from this one.

## Exercises

1. *(compiler)* Make `char_wpm` public (`pub char_wpm: u32`) and, in a test,
   build a `Timing` directly with `Timing { char_wpm: 0, effective_wpm: 0 }`.
   It compiles. Now call `dit_ms()` and watch it panic on divide-by-zero. Revert.
   You have just demonstrated why the fields are private — the invariant was
   never enforced by the *type*, only by the constructors, and making fields
   public routes around them.

2. *(compiler)* Remove `Copy` from the derive list. Find the errors this causes
   in the tests. Read one closely: it will say a value was *moved*. That word is
   the whole subject of Chapter 4.

3. *(build)* Add `element_ms(&self, symbol: char) -> Option<u32>` returning the
   dit or dah duration for `'.'` or `'-'`, and `None` otherwise. This is the
   Chapter 2 `Option` lesson applied to a method.

4. *(build)* Add `paris_ms(&self) -> u32`, the time to send "PARIS" once at the
   current timing. Test that `Timing::new(20).paris_ms()` is close to 3000ms
   (20 WPM = 20 words in 60s). You will need a tolerance rather than equality —
   why?

5. *(think)* `dit_ms` returns `u32`. At `MIN_WPM` (5) a dit is 240ms. What is the
   longest duration this type can express, and could any realistic Morse timing
   overflow it? Now consider `char_gap_ms` at 5 WPM effective with 40 WPM
   characters — work out roughly how large that gets.

---

Next: [Chapter 4 — The trainer](04-trainer.md), where mutable state forces us to
confront ownership properly.
