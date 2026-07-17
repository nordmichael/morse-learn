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
    │       ├── timing.rs        # WPM → dit/dah/gap durations (+ Farnsworth)
    │       ├── audio.rs         # Morse → tone schedule (beep start/length)
    │       ├── keyer.rs         # two-key & straight-key (press-length) decoding
    │       ├── mode.rs          # drill modes + input methods
    │       └── trainer.rs       # scoring, letter progression, hints, word choice
    └── morse-app/              # Dioxus UI (web / mobile / desktop)
        ├── src/
        │   ├── main.rs          # components + rsx
        │   └── platform.rs      # seed, localStorage, clock, Web Audio playback
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

It ships a **web app manifest** (`assets/manifest.webmanifest`, self-contained
with a data-URI icon) plus `theme-color` and Apple touch-icon/web-app meta tags,
so it's **installable** to the home screen on Android and iOS out of the box.
(Offline caching via a service worker is a follow-up — it needs a root-scoped
worker, which the asset-hashing pipeline doesn't emit yet.)

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

## Learning features

The trainer drills **both directions** of the skill plus listening, so the same
letter is practised as recognition *and* production:

| Drill | Prompt | Answer | Skill |
| --- | --- | --- | --- |
| See code → letter | dots/dashes shown | pick the letter | visual recognition |
| See letter → send | letter shown | key the code | production / sending |
| Hear → send (echo) | code played | key the code | intermediate ear→hand |
| Hear → letter | code played | pick the letter | head-copy / receiving |

- **Speed** runs from 5 to **40 WPM** (the learner's ceiling) on standard PARIS
  timing, with a separate **character** and **effective** control: lower the
  effective speed below the character speed and it switches to **Farnsworth**
  (letters at full speed, gaps stretched) — the recommended way to build speed
  without plateauing.
- **Auto speed-ramp** (optional): a rolling-accuracy coach (`morse-core::coach`)
  nudges the speed up when you're ~90%+ accurate over the last 10 answers and
  eases it back when you slip, closing the Farnsworth gap first, then raising the
  character speed. Live accuracy is shown next to the toggle.
- **Audio** is a clean 600 Hz sidetone generated from the same timing, so what
  you hear matches what you'd key.
- **Two input methods**, toggled live:
  - **Two keys** — one for dit, one for dah (paddle style).
  - **Straight key** — a single key where *press length* decides dit vs dah,
    like a real Morse key. The dit/dah threshold tracks the current WPM, and the
    letter **auto-commits after an inter-character gap** (no button needed).
- **Physical keyboard** (web/desktop): `.`/`j` = dit, `-`/`k` = dah,
  Space/Enter = commit, Backspace = clear; in recognition drills just press the
  `a`–`z` key. No clicking to focus — the whole app is playable from the keyboard.

All of the above (timing math, tone schedule, press/gap classification, drill
definitions) lives in `morse-core` and is unit-tested — the UI just renders it.

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

1. **Offline service worker** — precache the WASM + assets for full offline use
   (needs a root-scoped worker; today the installable manifest ships, but not
   offline caching, because the asset pipeline hashes paths under `/assets/`).
2. **Native shells via `uniffi`** — only if you later want a *fully-native*
   SwiftUI/Kotlin UI reusing `morse-core`. Not required for mobile: Dioxus
   already builds this same app for Android and iOS.
