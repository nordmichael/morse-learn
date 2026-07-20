# Chapter 2 — The alphabet

> **You'll build:** the Morse lookup table and its two conversion functions.
> **You'll learn:** `Option`, pattern matching, `&str` vs `String`, iterators,
> `const`, and arrays vs `Vec`.

## The problem

Morse maps letters to dot/dash patterns: `e` is `.`, `t` is `-`, `s` is `...`.
We need conversion in both directions — to play a letter as audio, and to decode
what a learner keyed.

The interesting question is what happens on a lookup that fails. What is
`to_morse('5')`? There is no Morse code for `5` in our table. Most languages
answer with `null`, an empty string, an exception, or a sentinel. Rust answers
with a type, and that difference is the subject of this chapter.

## The table

Replace `crates/morse-core/src/alphabet.rs`:

```rust
//! Morse alphabet: a static lookup table plus conversion helpers.
//!
//! This is the portable heart of the trainer. Everything here is `const`/pure,
//! has no dependencies, and works identically on every platform.

/// Letters in the order the trainer teaches them. This is *not* alphabetical:
/// it roughly follows letter frequency and Morse simplicity, so early lessons
/// use short, common codes (`e = .`, `t = -`, `a = .-`, ...).
pub const LEARNING_ORDER: [char; 26] = [
    'e', 't', 'a', 'i', 'm', 's', 'o', 'h', 'n', 'c', 'r', 'd', 'u', 'k', 'l',
    'f', 'b', 'p', 'g', 'j', 'v', 'q', 'w', 'x', 'y', 'z',
];

/// `(letter, morse)` pairs. `.` is a dot (dit), `-` is a dash (dah).
const TABLE: [(char, &str); 26] = [
    ('a', ".-"),    ('b', "-..."),  ('c', "-.-."),  ('d', "-.."),
    ('e', "."),     ('f', "..-."),  ('g', "--."),   ('h', "...."),
    ('i', ".."),    ('j', ".---"),  ('k', "-.-"),   ('l', ".-.."),
    ('m', "--"),    ('n', "-."),    ('o', "---"),   ('p', ".--."),
    ('q', "--.-"),  ('r', ".-."),   ('s', "..."),   ('t', "-"),
    ('u', "..-"),   ('v', "...-"),  ('w', ".--"),   ('x', "-..-"),
    ('y', "-.--"),  ('z', "--.."),
];
```

### `const` and the type `[char; 26]`

`const` is a compile-time constant. It is not a variable that happens not to
change — it is inlined at each use site, and it has no address at runtime.

The type `[char; 26]` is an **array**: 26 `char`s, contiguous, with the length
part of the type. `[char; 25]` is a *different type*. That is unusual if you are
coming from a language where arrays are just sized-at-runtime lists, and it is
what makes the array free — no heap allocation, no length field, no pointer
chasing.

Rust's growable list is `Vec<T>`, which does allocate. Rule of thumb: fixed and
known at compile time → array; grows at runtime → `Vec`. `LEARNING_ORDER` is
exactly 26 letters forever, so an array is right.

A `char` in Rust is a **Unicode scalar value** — 4 bytes, holding any character,
not a byte. `'a'` is a `char`; `"a"` is a string. Single and double quotes are
not interchangeable as they are in some languages.

### `&str`, and the lifetime you're not writing

`(char, &str)` is a **tuple** — a fixed-size, mixed-type group. Access by
position (`pair.0`) or by destructuring, which we will do shortly.

`&str` (pronounced "string slice") is a *borrowed view* of UTF-8 text: a pointer
and a length. It does not own its bytes. `String` is the owned, heap-allocated,
growable counterpart.

This is the most common early stumbling block in Rust, so make it concrete:

```rust
let owned: String = String::from("hello");  // owns a heap buffer
let borrowed: &str = &owned;                // points into that buffer
let literal: &str = "hello";                // points into the binary itself
```

`"-.-."` in our table is a string literal, baked into the executable. It lives
for the entire program, so its type is really `&'static str` — a borrowed string
slice with the `'static` lifetime. In a `const` you may write `&str` and Rust
infers `'static`, which is why the table compiles without lifetime annotations.

The guideline that will serve you for a long time: **take `&str` in function
parameters, return `String` when you must produce new text.** A `&str` parameter
accepts both a `String` and a literal at no cost; returning `String` is honest
about the allocation.

## `Option`: the type that replaced null

Now the lookups:

```rust
/// The Morse pattern for an ASCII letter, e.g. `to_morse('s') == Some("...")`.
/// Returns `None` for anything that isn't `a`..=`z` (case-insensitive).
pub fn to_morse(letter: char) -> Option<&'static str> {
    let letter = letter.to_ascii_lowercase();
    TABLE
        .iter()
        .find(|(c, _)| *c == letter)
        .map(|(_, code)| *code)
}

/// The letter for a Morse pattern, e.g. `from_morse("...") == Some('s')`.
/// Returns `None` for any sequence that isn't a valid single-letter code.
pub fn from_morse(code: &str) -> Option<char> {
    TABLE
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(letter, _)| *letter)
}
```

`Option<T>` is an enum with exactly two shapes:

```rust
enum Option<T> {
    Some(T),   // a value
    None,      // no value
}
```

That is the whole thing. It is not compiler magic; you could write it yourself.
Its power is that **`Option<&str>` and `&str` are different types**, so you
cannot use a possibly-missing value as though it were present. There is no
`null` in Rust. A `&str` always points at a real string; if absence is possible,
the type says `Option<&str>` and the compiler makes you handle it.

Every null-pointer bug you have shipped was a value whose type claimed it was
always there. Rust makes that claim checkable.

Note the return type: `Option<&'static str>`, not `Option<String>`. We return a
pointer into the table — no allocation, no copy. The `'static` says the data
outlives any caller, which is true: it is in the binary. If we returned
`Option<String>` we would allocate and copy on every lookup for no benefit.

### Reading `to_morse` piece by piece

```rust
let letter = letter.to_ascii_lowercase();
```

**Shadowing.** The new `letter` replaces the old one for the rest of the scope.
Not mutation — a new binding with the same name, and it could even have a
different type. Idiomatic Rust uses this freely for "normalise then use", and it
avoids inventing names like `letter_lower`.

```rust
TABLE.iter()
```

`.iter()` yields references to each element: `&(char, &str)`. **Iterators in
Rust are lazy** — nothing happens until something consumes them. `.iter()` alone
does no work.

```rust
.find(|(c, _)| *c == letter)
```

`find` takes a closure and returns the first element matching it, as an
`Option`. If nothing matches, `None` — the failure case appears here, from the
standard library, without us writing it.

`|(c, _)| ...` is a closure whose parameter is **destructured**: the tuple is
unpacked into `c` and `_`. `_` means "there is a value here and I am ignoring
it". Being explicit about ignoring is a small thing that makes intent obvious.

`*c` **dereferences**. `find` hands the closure a reference to the element, and
destructuring gives us `c: &char`. Comparing `&char` to `char` is a type error,
so `*c` gets the `char` out. Expect to fumble `*` and `&` early — the compiler
will tell you which one you need, and the instinct arrives faster than you expect.

```rust
.map(|(_, code)| *code)
```

`find` gave us `Option<&(char, &str)>` — the whole pair. We want just the code.
`Option::map` transforms the value inside if there is one, and passes `None`
through untouched:

- `Some(('s', "...")).map(...)` → `Some("...")`
- `None.map(...)` → `None`

This is the same `map` you know from arrays, on a container that holds zero or
one items. The `None` case needs no branch — that is the point of combinators.

### The imperative version, for comparison

If the chained version is doing too much at once, this is the same function
written out:

```rust
pub fn to_morse(letter: char) -> Option<&'static str> {
    let letter = letter.to_ascii_lowercase();
    for (c, code) in TABLE.iter() {
        if *c == letter {
            return Some(code);
        }
    }
    None
}
```

Identical behaviour and identical performance. Write whichever you find clearer;
the chained form is more idiomatic, and reading it fluently comes with exposure.

## Using an `Option`

Four ways to get at the value, roughly in order of how often you should reach
for them:

**`match`** — handle both cases explicitly:

```rust
match to_morse('s') {
    Some(code) => println!("s is {code}"),
    None => println!("no code for that"),
}
```

`match` is **exhaustive**: forget `None` and it will not compile. That
exhaustiveness is the safety net. When you add a variant to an enum later, every
`match` that needs updating becomes a compile error rather than a silent
fall-through — Chapter 4 leans on this hard.

**`if let`** — when only one case is interesting:

```rust
if let Some(code) = to_morse('s') {
    println!("s is {code}");
}
```

**Combinators** — `map`, `unwrap_or`, `and_then`, `filter`, when you are
transforming rather than branching:

```rust
let code = to_morse('5').unwrap_or("");   // "" if absent
```

**`unwrap` / `expect`** — take the value, panic if `None`:

```rust
let code = to_morse('s').expect("every letter has a code");
```

This crashes the program if wrong. It is right only when `None` genuinely means
a bug, and `expect` with a real message is always better than bare `unwrap` —
the message is what you get in the panic output at 2am. Chapter 5 is devoted to
choosing correctly here, and Chapter 9 shows an actual crash caused by an
`unwrap` deep inside a library.

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_letter() {
        for letter in 'a'..='z' {
            let code = to_morse(letter).expect("every letter has a code");
            assert_eq!(from_morse(code), Some(letter));
        }
    }

    #[test]
    fn known_codes() {
        assert_eq!(to_morse('e'), Some("."));
        assert_eq!(to_morse('t'), Some("-"));
        assert_eq!(to_morse('S'), Some("...")); // case-insensitive
        assert_eq!(from_morse("-.-.").unwrap(), 'c');
    }

    #[test]
    fn rejects_unknown() {
        assert_eq!(to_morse('5'), None);
        assert_eq!(from_morse(".-.-.-"), None);
    }

    #[test]
    fn learning_order_is_a_permutation_of_the_alphabet() {
        let mut sorted = LEARNING_ORDER;
        sorted.sort_unstable();
        let alphabet: Vec<char> = ('a'..='z').collect();
        assert_eq!(sorted.to_vec(), alphabet);
    }
}
```

`round_trips_every_letter` is the most valuable of these. Rather than asserting
26 hand-written pairs, it asserts a *property*: encoding then decoding returns
what you started with. It catches a typo in any entry, and it keeps working if
you change the table.

`learning_order_is_a_permutation_of_the_alphabet` guards a real hazard.
`LEARNING_ORDER` is hand-typed in a deliberately non-alphabetical order, so a
duplicated or missing letter is very easy to introduce and nearly invisible on
inspection. Sorting it and comparing to `a`–`z` catches that instantly.

Two details in that test:

```rust
let mut sorted = LEARNING_ORDER;
```

This **copies**. Arrays of `Copy` types are themselves `Copy`, so assignment
duplicates rather than moves — `LEARNING_ORDER` is untouched. Chapter 4 covers
what happens with types that are *not* `Copy`, which is where ownership starts
to bite.

`mut` is required to sort in place. Rust bindings are immutable by default;
`mut` is opt-in, and the compiler warns about `mut` you did not need.

```rust
let alphabet: Vec<char> = ('a'..='z').collect();
```

`'a'..='z'` is an **inclusive range**, and ranges over `char` are iterators.
`collect()` runs an iterator into a collection — which collection is decided by
the type annotation. Remove `: Vec<char>` and the compiler will tell you it
cannot infer the target type. This is one of the few places Rust needs your help.

```sh
cargo test -p morse-core
```

```
running 4 tests
test alphabet::tests::known_codes ... ok
test alphabet::tests::learning_order_is_a_permutation_of_the_alphabet ... ok
test alphabet::tests::rejects_unknown ... ok
test alphabet::tests::round_trips_every_letter ... ok

test result: ok. 4 passed; 0 failed
```

## Update `lib.rs`

```rust
pub mod alphabet;

pub use alphabet::{from_morse, to_morse, LEARNING_ORDER};
```

## A word on linear search

`to_morse` scans up to 26 entries. A `HashMap` would be O(1) — but a hash map
allocates, needs a dependency-free hasher decision, cannot be `const`, and for 26
contiguous entries that fit in a cache line or two, the linear scan is likely
*faster* than hashing. It also compiles to nothing at startup.

Reach for the fancy data structure when a measurement asks you to. At this size,
the array is both simpler and better.

## Checkpoint

```sh
cargo test
```

Four passing tests. You have a bidirectional Morse table with no dependencies
and no way to silently return a wrong answer for an unknown input.

## Exercises

1. *(compiler)* Change `to_morse`'s return type to `&'static str` and try to
   return `code` without the `Option`. Read the error. Then try returning `""`
   for the missing case and ask yourself how a caller distinguishes "no code"
   from "empty code" — this is exactly the ambiguity `Option` eliminates.

2. *(compiler)* Remove the `*` from `*c == letter`. The error mentions comparing
   `&char` with `char`. Now try `c == &letter` instead — that also works. Why?

3. *(build)* Add `is_valid_code(code: &str) -> bool`, true when every character
   is `.` or `-` and the string is non-empty. Test the empty string, `".-"`, and
   `".x"`. Hint: `chars().all(...)`.

4. *(build)* Add `to_morse_word(text: &str) -> String` joining each letter's code
   with spaces, skipping unknown characters. `"sos"` → `"... --- ..."`. This one
   genuinely must return `String` — you are building new text, not pointing at
   existing text. Hint: `filter_map` and `collect::<Vec<_>>().join(" ")`.

5. *(think)* `from_morse` takes `&str` but `to_morse` takes `char`. Why not
   `String` for the first? What would callers have to do differently, and how
   many allocations would a decode loop cost?

---

Next: [Chapter 3 — Timing](03-timing.md), where structs and methods turn
words-per-minute into milliseconds.
