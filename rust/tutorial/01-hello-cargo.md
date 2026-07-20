# Chapter 1 — Hello, cargo

> **You'll build:** a two-crate workspace with a passing test.
> **You'll learn:** crates, workspaces, modules, `cargo test`, and why the split
> between logic and UI matters more than any other decision in this project.

## The decision that shapes everything

Before writing code, one architectural choice. This app will eventually run in a
browser as WebAssembly, and on Android, iOS, and desktop as native code. The
naive structure is one crate containing everything.

We are going to use two:

```
morse-tutorial/
├── Cargo.toml              # the workspace
└── crates/
    ├── morse-core/         # pure logic. No UI. No browser. No platform.
    └── morse-app/          # the Dioxus UI and platform glue
```

`morse-core` will hold the Morse alphabet, the timing math, the scoring rules,
the word selection, the drill definitions — everything that is *about Morse code*.
It will have **zero dependencies** and know nothing about screens, browsers, or
audio hardware.

`morse-app` will hold everything that is about *being an app*: rendering,
clicking, playing sound, saving to disk.

Three things fall out of that split, and they compound:

**Tests run instantly.** `morse-core` has no browser, so its tests are plain
`cargo test` — no headless Chrome, no wasm harness, no async. By the end of this
tutorial you will have around 40 tests that run in well under a second. Tests
that are fast get run; tests that need a browser get skipped.

**The logic compiles everywhere unchanged.** Not "ported to" each platform —
literally the same code. WebAssembly, ARM64 Android, x86 desktop all compile the
same `morse-core`.

**The hard parts stay testable.** Farnsworth timing (Chapter 3) is fiddly
arithmetic. The letter-unlock rules (Chapter 4) are subtle. Both are pure
functions here, so you can unit-test them exhaustively instead of clicking
through a UI to find out whether you got them right.

The general principle: **push logic down into a layer that has no dependencies,
and keep the platform at the edges.** It is good advice in any language. Rust
makes it especially rewarding because a dependency-free crate compiles fast,
cross-compiles without drama, and tests without a harness.

## Creating the workspace

```sh
mkdir morse-tutorial && cd morse-tutorial
mkdir crates
cargo new --lib crates/morse-core
cargo new --bin crates/morse-app
```

`--lib` makes a **library crate** — code meant to be used by other code, with
`src/lib.rs` as its root. `--bin` makes a **binary crate** — a program with a
`main()`, rooted at `src/main.rs`.

Now the workspace file. Create `Cargo.toml` in `morse-tutorial/`:

```toml
[workspace]
resolver = "2"
members = ["crates/morse-core", "crates/morse-app"]

[workspace.package]
edition = "2021"
license = "Apache-2.0"
```

A **workspace** is several crates that build together, share one `Cargo.lock`,
and share one `target/` directory. `cargo test` at the root tests all of them.

`resolver = "2"` selects the modern dependency-feature resolver. Version 1 had a
genuinely annoying flaw: it unified features across build-time and runtime
dependencies, so a feature enabled for a build script could leak into your final
binary. Resolver 2 fixed it. You want it; it is the default for edition 2021
onward but is worth stating explicitly in a workspace root.

**Edition** is Rust's mechanism for evolving without breaking old code. Editions
(2015, 2018, 2021, 2024) can change syntax and idioms, and crates of different
editions interoperate freely. Pinning `edition = "2021"` means your code keeps
compiling when newer editions land. This is not semantic versioning of the
compiler — a 2021-edition crate compiles fine on the newest toolchain.

## Connecting the crates

`morse-app` needs to use `morse-core`. Edit `crates/morse-app/Cargo.toml`:

```toml
[package]
name = "morse-app"
version = "0.1.0"
edition = "2021"

[dependencies]
morse-core = { path = "../morse-core" }
```

A **path dependency** points at a directory instead of crates.io. Cargo rebuilds
`morse-core` whenever it changes.

Note the name change across the boundary: the package is `morse-core` with a
hyphen, but in Rust code you write `morse_core` with an underscore. Hyphens are
not legal in Rust identifiers, so Cargo translates. This surprises everyone once.

Now make `crates/morse-core/src/lib.rs` say something worth importing:

```rust
//! morse-core: the portable, UI-free heart of the Morse trainer.

/// Greet, temporarily, so we have something to call.
pub fn hello() -> &'static str {
    "morse-core is alive"
}
```

Two comment forms appear here and they are not the same as `//`:

- `//!` documents **the thing it is inside** — here, the crate itself.
- `///` documents **the thing that follows** — here, the function.

Both are *doc comments*: `cargo doc --open` renders them into HTML, and code
examples inside them are compiled and run by `cargo test`. Plain `//` comments
are invisible to tooling. Use `///` on anything public.

`pub` makes the function visible outside the crate. Rust is private by default —
at every level, not just crate boundaries. A field, function, or module with no
`pub` is visible only within its own module and that module's descendants. This
default is doing real work: it means "what can the outside world touch?" has a
precise, compiler-checked answer.

Call it from `crates/morse-app/src/main.rs`:

```rust
fn main() {
    println!("{}", morse_core::hello());
}
```

```sh
cargo run -p morse-app
```

```
morse-core is alive
```

`-p` selects a package in the workspace. Without it, cargo has to guess, and in a
workspace with several binaries it will refuse to.

## Your first test

Replace `lib.rs` with something testable:

```rust
//! morse-core: the portable, UI-free heart of the Morse trainer.

/// The number of letters the trainer teaches.
pub const ALPHABET_LEN: usize = 26;

/// Whether a character is a letter the trainer can teach (`a`–`z`).
pub fn is_teachable(c: char) -> bool {
    c.is_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teaches_lowercase_letters_only() {
        assert!(is_teachable('a'));
        assert!(is_teachable('z'));
        assert!(!is_teachable('A'));
        assert!(!is_teachable('5'));
        assert!(!is_teachable(' '));
    }
}
```

```sh
cargo test -p morse-core
```

```
running 1 test
test tests::teaches_lowercase_letters_only ... ok

test result: ok. 1 passed; 0 failed
```

Unpacking that test block, because this pattern appears in every file from here on:

**`#[cfg(test)]`** is conditional compilation. `cfg` means *configuration*: the
attribute says "only compile this when the `test` configuration is active."
During a normal build the entire module vanishes — no cost in your shipped
binary. We will use `cfg` again in Chapter 7 for something more interesting:
compiling different code for the browser and for desktop.

**`mod tests`** declares a module. Modules are Rust's namespaces, and unlike many
languages they do not have to map to files — this one is inline.

**`use super::*`** imports everything from the parent module. Test modules are
children of the code they test, so `super` is the module above. Without this you
would write `super::is_teachable(...)` every time.

**`#[test]`** marks a test function. `cargo test` finds and runs them.

**`assert!`** panics if its argument is false. The `!` means macro, not function
— macros can do things functions cannot, like capturing the source text of their
arguments to produce a good failure message. You will also use `assert_eq!`,
which prints both values when they differ:

```
assertion `left == right` failed
  left: Some('c')
 right: Some('d')
```

That message is why `assert_eq!(a, b)` beats `assert!(a == b)`. Reach for it
whenever you are comparing values.

## Tests live next to the code

Notice the test is in the *same file* as the function it tests. Coming from most
languages this looks wrong — surely tests belong in `tests/`?

Rust supports both, and they mean different things:

- **Unit tests** go in a `#[cfg(test)] mod tests` beside the code. They can reach
  private functions, because they are inside the module.
- **Integration tests** go in a top-level `tests/` directory. They link your
  crate the way an external user would, so they can only touch `pub` items.

Testing private internals from inside is not a workaround — it is the intended
design. `morse-core` will keep its struct fields private and test the logic
through the module's own tests. Chapter 4 does exactly this, reaching a private
`consecutive_correct` field that no outside caller can see.

## Splitting into modules

One `lib.rs` will not hold a whole trainer. Rust maps modules to files. Create
`crates/morse-core/src/alphabet.rs`:

```rust
//! The Morse alphabet and letter ordering.

/// The number of letters the trainer teaches.
pub const ALPHABET_LEN: usize = 26;

/// Whether a character is a letter the trainer can teach (`a`–`z`).
pub fn is_teachable(c: char) -> bool {
    c.is_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teaches_lowercase_letters_only() {
        assert!(is_teachable('a'));
        assert!(!is_teachable('A'));
        assert!(!is_teachable('5'));
    }
}
```

And reduce `lib.rs` to a table of contents:

```rust
//! morse-core: the portable, UI-free heart of the Morse trainer.
//!
//! Everything platform-independent lives here: the alphabet, timing, key
//! decoding, drill modes, and the trainer state machine. No platform
//! dependencies, so the identical logic compiles to WebAssembly for the web
//! and to native code for Android, iOS, and desktop.

pub mod alphabet;

pub use alphabet::{is_teachable, ALPHABET_LEN};
```

`pub mod alphabet;` means: there is a module named `alphabet`, find it in
`alphabet.rs`, and make it public. Rust has no `#include` and no build-file
listing of sources — the module tree *is* declared in code, and a file nobody
declares is simply not compiled. If you create a file and nothing happens, this
is why.

`pub use` **re-exports**. Callers can now write `morse_core::is_teachable(...)`
instead of `morse_core::alphabet::is_teachable(...)`. This lets you organise
internals into as many modules as you like while presenting a flat, convenient
public API. The real `morse-core` does this for every module — ten modules
internally, one clean surface outside.

```sh
cargo test -p morse-core
```

Still green. Same test, now in its own module.

## Checkpoint

```sh
cargo test
```

```
running 1 test
test alphabet::tests::teaches_lowercase_letters_only ... ok

test result: ok. 1 passed; 0 failed
```

Note the test name gained its module path. With no `-p`, cargo tests every crate
in the workspace — `morse-app` has no tests yet, so it reports zero.

Your tree:

```
morse-tutorial/
├── Cargo.toml
├── Cargo.lock
└── crates/
    ├── morse-core/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       └── alphabet.rs
    └── morse-app/
        ├── Cargo.toml
        └── src/main.rs
```

## Exercises

1. *(compiler)* Delete `pub` from `is_teachable` in `alphabet.rs` and run
   `cargo test`. Read the error carefully — it names both the privacy problem and
   the exact fix. Notice the test still passes at first glance: the test is
   *inside* the module, so it can see private items. It is `lib.rs`'s `pub use`
   that breaks. Put `pub` back.

2. *(compiler)* Rename `alphabet.rs` to `letters.rs` without touching `lib.rs`.
   The error tells you Rust cannot find the module and suggests where it looked.
   This is the "file nobody declares is not compiled" rule, seen from the other
   side. Rename it back.

3. *(build)* Add `is_teachable_uppercase` accepting `A`–`Z`, with a test. Then
   consider: is a second function the right design, or should `is_teachable`
   handle both cases? Chapter 2 takes the second approach and explains why.

4. *(think)* `ALPHABET_LEN` is `usize`, not `u32` or `i32`. `usize` is the type
   of "a size or index on this machine" — 64 bits on modern hardware, 32 on wasm.
   Why would a length be that type rather than a fixed-width integer? What breaks
   if an index is a different type from a length? (Chapter 2 makes this concrete.)

---

Next: [Chapter 2 — The alphabet](02-alphabet.md), where a lookup table teaches
`Option`, pattern matching, and the `&str`/`String` distinction.
