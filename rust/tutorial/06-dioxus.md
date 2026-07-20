# Chapter 6 — Dioxus

> **You'll build:** a running Morse trainer in the browser.
> **You'll learn:** traits, closures, `Copy` as an API design tool, reactive
> signals, and components.

`morse-core` is a complete learning engine that nobody can use. Time for a UI.

We are using [Dioxus](https://dioxuslabs.com/), a React-shaped framework for
Rust: components are functions, state lives in signals, and re-rendering happens
when state changes. If you have written React, most of this will feel familiar —
and the places it *differs* are where Rust's ownership rules assert themselves.

## Setup

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli
```

`crates/morse-app/Cargo.toml`:

```toml
[package]
name = "morse-app"
version = "0.1.0"
edition = "2021"

[dependencies]
morse-core = { path = "../morse-core" }
dioxus = { version = "0.7", features = ["router"] }

[features]
default = ["web"]
web = ["dioxus/web"]
desktop = ["dioxus/desktop"]
mobile = ["dioxus/mobile"]
```

**Features** are Cargo's conditional compilation for dependencies. `default = ["web"]`
means a plain `cargo build` produces the web build; `--no-default-features
--features desktop` targets desktop instead. The same source, three platforms.

`crates/morse-app/Dioxus.toml`:

```toml
[application]
name = "morse-app"

[web.app]
title = "Morse Learn"

[web.watcher]
reload_html = true
```

## Traits, briefly

Before signals, the concept underneath them.

A **trait** is a set of behaviours a type can implement — closest to an interface,
but with an important difference: you can implement your own trait for someone
else's type, and someone else's trait for your type.

```rust
trait Describe {
    fn describe(&self) -> String;
}

impl Describe for morse_core::Judgement {
    fn describe(&self) -> String {
        match self {
            Judgement::Correct => "got it".to_string(),
            Judgement::Incorrect => "not quite".to_string(),
        }
    }
}
```

That is legal even though `Judgement` is in another crate, as long as the *trait*
is yours. (The restriction — one of the two must be local — is the **orphan rule**,
and it exists so two crates cannot supply conflicting implementations.) Chapter 8
uses this to add methods to `Timing` from a different module entirely.

You have been using traits since Chapter 1. `#[derive(Debug, Clone, Copy)]`
implements three of them. `assert_eq!` requires `PartialEq`. `.iter()` comes from
`IntoIterator`. Traits are how Rust does polymorphism, and `derive` is how you get
the boilerplate for free.

## Signals: state that is `Copy`

Dioxus state lives in a `Signal<T>`:

```rust
let mut count = use_signal(|| 0);

count.set(5);              // write
let n = count();           // read (calling it is shorthand for .read())
count.with_mut(|c| *c += 1); // mutate in place
```

`use_signal` is a **hook**, and hooks carry the same rule as React: call them
unconditionally, in the same order, every render. Never inside an `if` or a loop.
Dioxus identifies hooks by call order, so a conditional hook silently corrupts
state.

Here is the design detail that shapes the whole UI: **`Signal<T>` is `Copy`,
regardless of what `T` is.**

`Signal<Trainer>` is `Copy` even though `Trainer` contains a `String`, a `Vec`,
and cannot itself be `Copy`. The signal is a lightweight handle — an index into
an arena the framework owns — so copying it duplicates the handle, not the data.

That single property is why the UI code is not drowning in `.clone()`. Every
closure that touches state needs its own copy of the handle, and there are dozens
of closures.

## Bundling state

With signals being `Copy`, we can put them all in one struct and make *that*
`Copy` too:

```rust
use dioxus::prelude::*;
use morse_core::{
    Answer, Drill, InputMethod, Judgement, Prompt, SpeedCoach, Timing, Trainer,
    MAX_WPM, MIN_WPM,
};

mod platform;

/// All mutable session state, bundled so event handlers and the keyboard
/// listener can share it. `Signal` is `Copy`, so `Ctx` is `Copy` too and can be
/// captured freely by closures without cloning.
#[derive(Clone, Copy, PartialEq)]
struct Ctx {
    trainer: Signal<Trainer>,
    buffer: Signal<String>,
    flash: Signal<Option<Judgement>>,
    drill: Signal<Drill>,
    method: Signal<InputMethod>,
    wpm: Signal<u32>,
    eff_wpm: Signal<u32>,
    coach: Signal<SpeedCoach>,
    auto_speed: Signal<bool>,
    show_hints: Signal<bool>,
}

impl Ctx {
    /// The active timing from the character/effective speeds.
    fn timing(&self) -> Timing {
        Timing::farnsworth((self.wpm)(), (self.eff_wpm)())
    }
}
```

`use dioxus::prelude::*` is the conventional bulk import. Preludes are a
deliberate exception to Rust's usual "import what you use" style; frameworks
provide them because you genuinely need thirty names.

`(self.wpm)()` needs the parentheses because `self.wpm` is a field holding a
callable, not a method. Without them Rust looks for a *method* named `wpm`.

Passing `Ctx` to a function moves nothing and clones nothing — it copies ten
small handles. Any function can take `ctx: Ctx` and mutate shared state through
it. Achieving that in most languages means a reference-counted context object and
careful lifetime management; here it is a `Copy` struct.

## Your first component

```rust
fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut ctx = Ctx {
        trainer: use_signal(|| Trainer::new(platform::seed())),
        buffer: use_signal(String::new),
        flash: use_signal(|| Option::<Judgement>::None),
        drill: use_signal(|| Drill::HearCodeSendCode),
        method: use_signal(|| InputMethod::TwoKey),
        wpm: use_signal(|| 15u32),
        eff_wpm: use_signal(|| 15u32),
        coach: use_signal(SpeedCoach::default),
        auto_speed: use_signal(|| false),
        show_hints: use_signal(|| true),
    };

    let target = ctx.trainer.read().target_letter();
    let word = ctx.trainer.read().current_word().to_string();

    rsx! {
        main { class: "app",
            div { class: "word", "{word}" }
            div { class: "target", "{target}" }
        }
    }
}
```

A **component** is a function returning `Element`. `#[component]` generates the
props plumbing.

`use_signal(String::new)` passes the function itself rather than a closure
calling it — `use_signal(|| String::new())` is equivalent but noisier. Same for
`SpeedCoach::default`.

`Option::<Judgement>::None` is turbofish again: `None` alone does not say what it
is `None` *of*, so we spell it out.

`.to_string()` on `current_word()` is the borrow rule from Chapter 4 in a new
setting. `ctx.trainer.read()` returns a guard borrowing the signal's contents; if
we held a `&str` into it while rendering, the guard would still be alive and the
next write would panic at runtime. Copying to an owned `String` releases it
immediately. **Get data out of signal guards quickly** — Chapter 9 shows what
happens when you do not.

### `rsx!`

`rsx!` is a macro compiling markup-ish syntax to Rust:

```rust
rsx! {
    div { class: "word", "{word}" }
}
```

Attributes are `name: value`. Children follow. Bare strings are text nodes, and
`"{word}"` interpolates like `format!`. It is a macro, so it is type-checked at
compile time — a typo'd variable is a compile error, not a blank screen.

## Running it

```sh
cd crates/morse-app
dx serve
```

Open `http://localhost:8080`. A word and a target letter. Not much, but it is
Rust running in a browser.

## Closures and `move`

Interaction means event handlers, and handlers mean closures — which is where
ownership shows up again.

```rust
button {
    onclick: move |_| {
        ctx.buffer.with_mut(|b| b.push('.'));
    },
    "•"
}
```

`move` forces the closure to **take ownership** of what it captures. Without it,
Rust captures by reference — and the closure outlives this render, so a reference
would dangle. The compiler rejects it and suggests `move`.

Because `Ctx` is `Copy`, `move` copies the handles. `ctx` stays usable afterwards,
so every one of a dozen handlers can `move` the same `ctx`.

Rust has three closure traits, and which one you get is inferred:

- **`Fn`** — captures by shared reference, callable many times
- **`FnMut`** — captures mutably, callable many times
- **`FnOnce`** — consumes captures, callable once

You rarely name these except in function signatures. Chapter 7's keyboard
registration takes `impl FnMut(String) -> bool + 'static`, which reads as: any
closure that takes a `String`, returns `bool`, can be called repeatedly, and
holds no borrows that could expire.

## Wiring the drill

```rust
/// Commit the keyed Morse in the buffer as an answer.
fn commit_code(mut ctx: Ctx) {
    if (ctx.buffer)().is_empty() || ctx.trainer.read().word_complete() {
        return;
    }
    let judgement = ctx.trainer.write().submit_morse(&(ctx.buffer)());
    after_answer(ctx, judgement);
}

/// The shared tail of every answer: flash the judgement, persist progress, feed
/// the speed coach, then either celebrate a finished word or cue the next target.
fn after_answer(mut ctx: Ctx, judgement: Judgement) {
    ctx.flash.set(Some(judgement));
    ctx.buffer.set(String::new());
    platform::save_scores(&ctx.trainer.read().scores());
    record_and_ramp(ctx, judgement == Judgement::Correct);

    let finished_word = ctx.trainer.read().word_complete();
    if finished_word {
        schedule_word_advance(ctx);
    } else {
        play_if_audio(ctx);
    }
}
```

Note `let finished_word = ...` on its own line rather than
`if ctx.trainer.read().word_complete() { ... }`. Binding first guarantees the read
guard is dropped before the branch runs. Inside that branch we call functions that
take `&mut` on the same signal, and holding a read guard across a write is a
**runtime** panic in Dioxus, not a compile error — signals check borrows
dynamically. Chapter 9 dissects exactly this failure.

This is a genuine gap in the compile-time guarantees: interior mutability moves
borrow checking to runtime. The habit that keeps you safe is to keep guards
short-lived and never hold one across a call that might write.

## The completion pause

Chapter 4's `word_complete` state exists so the UI can show a check-mark. Here is
the other half:

```rust
/// How long a finished word stays on screen, with its check-mark, before the
/// next one loads.
const WORD_DONE_MS: u32 = 800;

/// Hold the completed word on screen for a beat, then advance to the next one.
fn schedule_word_advance(mut ctx: Ctx) {
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(WORD_DONE_MS).await;
        ctx.trainer.write().next_word();
        ctx.buffer.set(String::new());
        ctx.flash.set(None);
        play_if_audio(ctx);
    });
}
```

`spawn` starts an async task tied to the component's lifetime — cancelled
automatically if the component unmounts. `async move` again takes ownership of
captures, and again `Copy` makes that free.

Rendering it:

```rust
let word_done = ctx.trainer.read().word_complete();

rsx! {
    div { class: "slots",
        for (i, text, class) in masked {
            span { key: "{i}", class, "{text}" }
        }
        if word_done {
            span { class: "wordmark", "✓" }
        }
    }
}
```

`rsx!` supports `for` and `if` directly. `key: "{i}"` gives list items stable
identity across renders, exactly as in React. `class,` is field shorthand — the
variable is already named `class`.

## Components with props

Split out anything reusable:

```rust
#[component]
fn Hint(letter: char, level: morse_core::HintLevel) -> Element {
    use morse_core::HintLevel;
    if level == HintLevel::None {
        return rsx! { div { class: "hint hidden" } };
    }
    let Some(m) = morse_core::mnemonic(letter) else {
        return rsx! { div { class: "hint hidden" } };
    };
    let show_marks = level == HintLevel::Pattern;
    rsx! {
        div { class: "hint",
            div { class: "mnem",
                for (i, syl) in m.syllables.iter().enumerate() {
                    div { key: "{i}", class: if syl.dah { "syl dah" } else { "syl dit" },
                        span { class: "syl-word", "{syl.text}" }
                        if show_marks {
                            span { class: "syl-mark", if syl.dah { "—" } else { "•" } }
                        }
                    }
                }
            }
        }
    }
}
```

Called as `Hint { letter: target, level: hint }`. Function parameters are props;
`#[component]` builds the struct behind them. That struct derives `PartialEq`,
which is how Dioxus skips re-rendering when props have not changed — and it is
why `Ctx` derives `PartialEq` too.

The `let else` from Chapter 5 appears here in its natural habitat: no mnemonic
means render nothing, and the happy path stays un-nested.

## Assets

```rust
const STYLE: Asset = asset!("/assets/style.css");

rsx! {
    document::Link { rel: "stylesheet", href: STYLE }
    // ...
}
```

`asset!` registers a file at compile time, hashes it for cache-busting, and
copies it into the bundle. A missing file is a **compile error**, not a 404 in
production.

## Checkpoint

```sh
dx serve
```

A trainer in the browser: it shows a word, accepts keyed Morse, scores it, tracks
progress, and shows a check-mark on word completion. No sound yet — that is next.

`cargo test` still passes, and `morse-core` still has no idea a UI exists.

## Exercises

1. *(compiler)* Remove `move` from an `onclick` closure. Read the error about
   borrowed content escaping. Then remove `Copy` from `Ctx`'s derive and see how
   many closures break at once — a good illustration of how much that one trait
   is carrying.

2. *(compiler)* Write `if ctx.trainer.read().word_complete() { ctx.trainer.write().next_word(); }`
   with the read inline. It **compiles**. Run it and watch it panic at runtime.
   This is the most important exercise in the chapter: the borrow checker did not
   save you, because signals check at runtime.

3. *(build)* Add a settings control that switches `Drill` via
   `Drill::all().iter()`. Use `onclick` to `ctx.drill.set(d)`.

4. *(build)* Make the flash class reflect the last judgement — green for correct,
   grey for incorrect — using `match (ctx.flash)()`.

5. *(think)* `Ctx` holds ten signals. The alternative is one
   `Signal<AppState>` holding a struct. What re-renders under each design when
   `buffer` changes? Which is easier to get wrong?

---

Next: [Chapter 7 — The browser](07-wasm-platform.md), where `cfg` lets one
codebase target the browser, desktop, and mobile.
