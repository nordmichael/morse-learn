# Building a Morse Trainer in Rust

A tutorial that builds this repository's `rust/` app from an empty directory,
teaching Rust as each piece demands it.

This is not a tour of finished code. You start with `cargo new` and finish with a
cross-platform Morse trainer that runs in a browser. Every chapter ends with code
that compiles and tests that pass — if something is red at a checkpoint, the
chapter is not finished with you yet.

## Who this is for

You write software already. You know your way around a terminal, a compiler, and
git. You have not written much Rust.

That means this tutorial skips "what is a variable" and spends its time on the
things that actually make Rust feel foreign at first:

- **Ownership and borrowing** — the rules that replace a garbage collector
- **`Option` and `Result`** — why Rust has no `null` and no exceptions
- **Traits** — how Rust does polymorphism without inheritance
- **Lifetimes** — only where they genuinely come up (which is later than you'd think)
- **`&str` vs `String`** — the distinction that trips up every newcomer

We meet these when the app needs them, not in the abstract. Ownership shows up in
Chapter 4 because that is where a mutable state machine forces the issue, not
because it is chapter four of a language reference.

## What you'll build

A Morse code trainer that:

- Teaches 26 letters in frequency order, unlocking new ones as you improve
- Drills in both directions — hear a code and send it, see a letter and send it,
  hear a code and name it
- Generates real Morse audio in the browser from WPM-based timing
- Accepts input from on-screen keys, a physical keyboard, or a straight key where
  press *duration* decides dit vs dah
- Adapts its speed to your rolling accuracy
- Saves progress to `localStorage`
- Compiles to WebAssembly, and to native for desktop and mobile

The learning logic is a dependency-free library crate with unit tests. The UI is
a separate crate using [Dioxus](https://dioxuslabs.com/). That split is not
decoration — Chapter 1 explains why it is the single most useful architectural
decision in the project, and every later chapter benefits from it.

## Chapters

| # | Chapter | Rust you'll learn |
|---|---------|-------------------|
| 1 | [Hello, cargo](01-hello-cargo.md) | Workspaces, crates, modules, `cargo test`, your first test |
| 2 | [The alphabet](02-alphabet.md) | Arrays, `const`, `Option`, pattern matching, iterators, `&str` vs `String` |
| 3 | [Timing](03-timing.md) | Structs, `impl`, methods, privacy, integer vs float math, `derive` |
| 4 | [The trainer](04-trainer.md) | Ownership, borrowing, `&mut self`, enums with meaning, state machines |
| 5 | [No unwrap()](05-option-and-errors.md) | `Option` combinators, `Result`, `?`, `let else`, when panicking is right |
| 6 | [Dioxus](06-dioxus.md) | Traits, closures, `Copy`, signals, reactive rendering, `rsx!` |
| 7 | [The browser](07-wasm-platform.md) | `cfg`, conditional compilation, wasm-bindgen, `web-sys`, FFI boundaries |
| 8 | [Audio and keying](08-audio-keyer-persistence.md) | Trait `impl` on foreign types, `VecDeque`, `std::mem::take`, async tasks |
| 9 | [Real bugs](09-real-bugs.md) | Reading panics, `RefCell`, runtime context, what the compiler can't catch |

Chapters 1–5 are pure Rust — no UI, no browser, no async. You can do them with
nothing but `cargo test`. If you only want the language, that is a complete arc
on its own.

Chapters 6–9 add the UI and the platform layer. That is where Rust stops being a
language exercise and starts being a tool for shipping something.

## Prerequisites

Install Rust via [rustup](https://rustup.rs/):

```sh
rustc --version   # 1.75 or newer
cargo --version
```

You need the WebAssembly target from Chapter 6 onward:

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli    # provides `dx`
```

Chapters 1–5 need neither.

## How to work through this

**Type the code.** Do not copy-paste. The compiler's error messages are the best
Rust teacher available, and you only get them by making mistakes. Rust's errors
are unusually good — they frequently tell you the exact fix, and learning to read
them is a real skill worth building early.

**Build in a scratch directory,** not in `rust/`. Something like `morse-tutorial/`
alongside this repo. Then you can compare against the real thing whenever you want:

```sh
diff morse-tutorial/crates/morse-core/src/alphabet.rs \
     morse-learn/rust/crates/morse-core/src/alphabet.rs
```

**Run the checkpoint** at the end of each chapter before moving on. Each one tells
you the exact command and the expected output.

**Do the exercises.** They are marked *(compiler)* when the point is to trigger a
specific error and read it, *(build)* when you are adding real functionality, and
*(think)* when there is no code to write. The *(compiler)* ones look like busywork
and are the ones that teach the most.

## A note on the finished code

By the end, your code will closely match `rust/crates/` in this repo, but not
byte-for-byte. The tutorial simplifies in a few places and says so when it does —
the mnemonic table is 26 hand-written entries we won't type out, and the word list
is generated. Where the tutorial's version differs deliberately, there is a note
explaining the difference and why.

Chapter 9 is the exception to "build it from scratch": it dissects two bugs that
actually occurred in this codebase, with the real panic output and the real fix.
Working code teaches you what to write. Broken code teaches you what to look for,
and there is no honest way to learn that from a tutorial where everything works
the first time.
