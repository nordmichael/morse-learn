# Morse Learn — Rust + Dioxus rewrite

A clean-room rewrite of the original Google Creative Lab Morse trainer as a
cross-platform Rust app. One codebase ships to **web, Android, iOS, and
desktop** via [Dioxus](https://dioxuslabs.com), and all the learning logic lives
in a small, dependency-free, fully-tested core crate.

The original Phaser/JavaScript version is still in the repository root and is
used only as the behavioural spec; nothing here depends on it.

## Why this stack

| Goal | How it's met |
| --- | --- |
| Learn Rust | The whole app is Rust. The core crate is pure logic (enums, pattern matching, traits, iterators) — the ideal place to get fluent. |
| One phone app + one web app | Dioxus renders the same components to WASM (web), and to native WebViews on Android/iOS via `dx`. |
| Maintainable | Game rules are isolated in `morse-core` behind a small API and covered by unit tests, independent of any UI or platform. |

## Layout

```
rust/
├── Cargo.toml                  # workspace
└── crates/
    ├── morse-core/             # portable logic — NO UI, NO platform deps
    │   └── src/
    │       ├── alphabet.rs      # Morse table + conversions
    │       ├── words.rs         # practice word pool (ported from words.js)
    │       ├── rng.rs           # tiny dependency-free PRNG (for word shuffling)
    │       └── trainer.rs       # scoring, letter progression, hints, word choice
    └── morse-app/              # Dioxus UI (web / mobile / desktop)
        ├── src/
        │   ├── main.rs          # components + rsx
        │   └── platform.rs      # seed + localStorage, cfg-gated per target
        └── assets/              # css, mnemonic images, dot/dash sounds
```

The architecture is deliberately **"Rust core, thin shell"**: the core knows the
game; the shell knows the screen. You can add a native SwiftUI/Kotlin front-end
later and reuse `morse-core` unchanged (via `uniffi`), or swap Dioxus out, without
touching a single game rule.

## Prerequisites

```sh
# Rust (already installed if you can read this)
rustup target add wasm32-unknown-unknown        # web target

# The Dioxus CLI drives builds/serving for every platform
cargo install dioxus-cli --version 0.6.3 --locked   # provides `dx`
```

## Run the core tests (no browser needed)

This is the fast inner loop while learning Rust — pure logic, instant feedback:

```sh
cd rust
cargo test -p morse-core
```

## Run as a web app

```sh
cd rust/crates/morse-app
dx serve --platform web
# open the printed http://localhost:8080
```

Build a static, deployable bundle (drop the output on any static host — GitHub
Pages, Netlify, Cloud Run, the existing App Engine setup, etc.):

```sh
dx build --platform web --release
# artifacts in target/dx/morse-app/release/web/public
```

Because it's a static WASM bundle, adding a web manifest + service worker turns
it into an installable **PWA** with no extra tooling.

## Build the phone app

Dioxus 0.6 builds native mobile apps through the same CLI. You need the platform
SDKs installed (Android Studio / Xcode).

```sh
# Android (needs Android SDK + NDK; emulator or device attached)
dx serve --platform android

# iOS (macOS + Xcode only)
dx serve --platform ios
```

`dx bundle --platform android --release` / `--platform ios` produce the
installable `.apk`/`.aab` and `.app`/`.ipa` for the stores.

## How the trainer works (ported rules)

- Letters are taught in frequency order: `e t a i m s o h n c r d u k l f b p g j v q w x y z`.
- Each letter has a score. Correct `+1` (max `+4`), wrong `−1` (min `−4`); a
  letter is **learned** at score `≥ 2`.
- You start with the first 3 letters. The next letter unlocks once the newest
  one is learned **and** you answer 3 correct in a row.
- Practice words are drawn only from letters currently in play; if the newest
  letter isn't learned yet, the word is forced to contain it.
- While a letter is unlearned, a mnemonic picture is shown; after several
  mistakes the Morse pattern is revealed too.

All of this is in `morse-core/src/trainer.rs` and exercised by `cargo test`.

## Suggested next steps

1. **Audio** — play the dot/dash samples on tap (they're already in `assets/`).
2. **Keyboard input** on web/desktop (`.`/`-`, Enter to commit).
3. **Timing-based input** — detect dot vs dash from press duration, like a real
   straight key, entirely in `morse-core` so every platform benefits.
4. **PWA manifest + service worker** for offline install.
5. **Native shells via `uniffi`** if you later want fully-native mobile UI.
