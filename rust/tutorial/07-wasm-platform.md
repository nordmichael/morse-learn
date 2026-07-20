# Chapter 7 — The browser

> **You'll build:** the platform layer — clock, storage, and keyboard — for both
> web and native.
> **You'll learn:** `cfg` conditional compilation, `web-sys`, `wasm-bindgen`, and
> how to design a boundary that keeps platform code from leaking everywhere.

The app needs four things from the outside world: a random seed, a millisecond
clock, persistent storage, and keyboard events. Every one differs by platform.

The temptation is to sprinkle `if cfg!(web)` through the UI. Don't. Instead,
**one module** — `platform.rs` — presents a single API to the rest of the app and
implements it twice. Everything above it stays platform-agnostic.

## `cfg`: compiling different code per target

You met `#[cfg(test)]` in Chapter 1. Same mechanism, different condition:

```rust
#[cfg(target_arch = "wasm32")]
pub fn seed() -> u64 {
    (js_sys::Date::now() as u64) ^ 0x9E37_79B9_7F4A_7C15
}

#[cfg(not(target_arch = "wasm32"))]
pub fn seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}
```

Two functions, same name and signature, exactly one compiled per target. Callers
write `platform::seed()` and never think about it.

This is compile-time, not runtime — the other branch is not in the binary at all,
so there is no cost and no dead code shipped. It also means **the excluded branch
is not type-checked**, which has a practical consequence: `cargo check` alone
proves nothing about the other platform. Check both:

```sh
cargo check -p morse-app --target wasm32-unknown-unknown
cargo check -p morse-app
```

Notice the native `seed()` also demonstrates Chapter 5's guidance. `duration_since`
returns a `Result` (the clock could be before the epoch), and rather than
panicking on an absurd edge case we `.map(...).unwrap_or(constant)`. A bad seed
means slightly less random word order — not worth a crash.

## Declaring browser dependencies

```toml
# Web build needs the browser clock (for a random seed) and localStorage (to
# persist progress). These only compile into the WASM target.
[target.'cfg(target_arch = "wasm32")'.dependencies]
js-sys = "0.3"
wasm-bindgen = "0.2"
gloo-timers = { version = "0.4", features = ["futures"] }
web-sys = { version = "0.3", features = [
    "Window",
    "Storage",
    "Document",
    "EventTarget",
    "Event",
    "KeyboardEvent",
    "AudioContext",
    "BaseAudioContext",
    "AudioNode",
    "AudioParam",
    "AudioDestinationNode",
    "GainNode",
    "OscillatorNode",
    "OscillatorType",
] }
```

Target-specific dependencies are not downloaded or built for other targets, so a
desktop build never sees `web-sys`.

That feature list is not boilerplate. `web-sys` covers the entire web API, and
compiling all of it would be enormously slow, so **every type is behind a
feature flag**. You enable precisely what you use. The error when you forget is
`cannot find type 'GainNode' in module 'web_sys'` — which looks like a missing
import and is actually a missing feature. Expect to hit this; the fix is always
"add the type name to the list".

The three crates divide up as: `js-sys` for JavaScript built-ins (`Date`,
`Array`), `web-sys` for browser APIs (`window`, `Storage`, `AudioContext`),
`wasm-bindgen` for the machinery that lets Rust and JS call each other.

## Storage, and `Option` chains in the wild

```rust
const STORAGE_KEY: &str = "morse_scores";

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[cfg(target_arch = "wasm32")]
pub fn load_scores() -> [i32; 26] {
    let mut scores = [0; 26];
    if let Some(raw) = local_storage().and_then(|s| s.get_item(STORAGE_KEY).ok().flatten()) {
        for (slot, part) in scores.iter_mut().zip(raw.split(',')) {
            if let Ok(v) = part.trim().parse::<i32>() {
                *slot = v;
            }
        }
    }
    scores
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_scores() -> [i32; 26] {
    [0; 26]
}
```

`local_storage()` packs four failure modes into one line:

- `web_sys::window()` returns `Option<Window>` — there may be no window (a worker)
- `?` returns `None` early, the Chapter 5 operator on `Option`
- `.local_storage()` returns `Result<Option<Storage>, JsValue>` — it can throw
  *and* be absent
- `.ok()` discards the error, `.flatten()` collapses `Option<Option<_>>`

Every one of these is a real browser condition: private browsing disables
storage, sandboxed iframes throw on access, workers have no `window`. The type
system forced us to acknowledge all four, and the combinators let us dismiss them
all in one expression.

The native version returns zeros. Progress does not persist on desktop, which is
fine — desktop is a development convenience here, and being explicit beats
pretending.

Saving mirrors it:

```rust
#[cfg(target_arch = "wasm32")]
pub fn save_scores(scores: &[i32; 26]) {
    let encoded = scores.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(STORAGE_KEY, &encoded);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_scores(_scores: &[i32; 26]) {}
```

`let _ =` deliberately discards the `Result`: a full storage quota must not crash
the app. Chapter 5's table, applied.

Note the parameter name `_scores` in the native stub. A leading underscore
suppresses the unused-variable warning while documenting what the parameter is.
Naming it `_` would work too but reads worse.

## Generic settings

The same shape, for on/off flags:

```rust
/// Load a persisted on/off setting, falling back to `default` when unset.
#[cfg(target_arch = "wasm32")]
pub fn load_flag(key: &str, default: bool) -> bool {
    match local_storage().and_then(|s| s.get_item(key).ok().flatten()) {
        Some(v) => v == "1",
        None => default,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_flag(_key: &str, default: bool) -> bool {
    default
}

/// Persist an on/off setting.
#[cfg(target_arch = "wasm32")]
pub fn save_flag(key: &str, value: bool) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, if value { "1" } else { "0" });
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_flag(_key: &str, _value: bool) {}
```

Used for the hints toggle:

```rust
show_hints: use_signal(|| platform::load_flag("show_hints", true)),
```

`if value { "1" } else { "0" }` is an expression producing a value — Rust has no
ternary operator because `if` already is one.

## Crossing into JavaScript

Keyboard events are the most interesting boundary, because they involve handing
Rust code *to* JavaScript to call later:

```rust
/// Register a global keyboard handler for `event` ("keydown" / "keyup"),
/// receiving each key's name (e.g. ".", "Enter", " ", "a"). If the handler
/// returns `true` the key is consumed and its default action suppressed. On
/// keydown, auto-repeat is ignored (so a held key fires once). The listener
/// lives for the app's lifetime.
#[cfg(target_arch = "wasm32")]
fn register_key(event: &str, ignore_repeat: bool, mut handler: impl FnMut(String) -> bool + 'static) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let closure = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        if ignore_repeat && e.repeat() {
            return;
        }
        if handler(e.key()) {
            e.prevent_default();
        }
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let _ = document.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
pub fn on_keydown(handler: impl FnMut(String) -> bool + 'static) {
    register_key("keydown", true, handler);
}

#[cfg(target_arch = "wasm32")]
pub fn on_keyup(handler: impl FnMut(String) -> bool + 'static) {
    register_key("keyup", false, handler);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn on_keydown(_handler: impl FnMut(String) -> bool + 'static) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn on_keyup(_handler: impl FnMut(String) -> bool + 'static) {}
```

There is a lot here. Taken in order:

**`impl FnMut(String) -> bool + 'static`** in the parameter position means "some
closure with this signature". `FnMut` because it will be called repeatedly and
may mutate captures. `'static` because it must outlive this function — the
browser will call it long after `register_key` returns, so it may not borrow
anything local.

**`Box<dyn FnMut(...)>`** puts the closure on the heap behind a trait object.
`impl Trait` is a compile-time-known concrete type; `dyn Trait` is dynamic
dispatch through a vtable. We need `dyn` here because `Closure::wrap` requires a
single known type regardless of which closure you passed.

**`Closure::wrap`** builds a JS-callable function from a Rust closure. This is the
actual FFI boundary — it allocates a JS shim that, when invoked, calls back into
wasm.

**`closure.forget()`** deliberately leaks it. Normally dropping a `Closure`
invalidates the JS function, so a listener registered and then dropped would fire
into freed memory — which wasm-bindgen turns into a "closure invoked after being
dropped" error. Since these listeners must live for the whole app, leaking is
correct. Calling `forget()` in a loop would be a genuine leak; once at startup is
fine, and the comment says so.

**`unchecked_ref`** is an unchecked cast to the JS function type. Named
"unchecked" honestly: no verification. It is the right call here because we just
created the value and know its type.

## Using it

Back in `main.rs`:

```rust
// Register a document-level keyboard listener once, so the app is fully
// playable from a physical keyboard without clicking to focus.
#[cfg(target_arch = "wasm32")]
use_hook(move || {
    platform::on_keydown(move |key| key_down(ctx, &key));
    platform::on_keyup(move |key| key_up(ctx, &key));
});
```

`use_hook` runs its closure **once**, on first render. Without it, every render
would add another listener and keys would fire multiple times.

> This code has a bug. It compiles, passes review, and panics the moment you
> press the space bar. Chapter 9 is about finding and fixing it — if you want to
> discover it yourself, run the app and hold space.

## Keeping the boundary honest

One detail keeps the module compiling everywhere:

```rust
use morse_core::LEARNING_ORDER;

/// Keep the import referenced on every platform so the module always compiles.
#[allow(dead_code)]
const _LETTER_COUNT_CHECK: usize = LEARNING_ORDER.len();
```

`#[allow(dead_code)]` silences the unused warning. This is a small pragmatic
hack, and worth flagging as such — an alternative would be `cfg`-gating the
import, at the cost of another conditional.

More broadly, expect **unused warnings on the platform you are not targeting**.
`TONE_HZ`, `on_keydown`, and `on_keyup` are all used on wasm and dead on native.
This project accepts six such warnings rather than papering over them with
blanket `#[allow]`s, on the grounds that a real unused item should still be
visible. That is a judgement call; the alternative is `#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]`
on each, which is noisier than the warning it suppresses.

## Checkpoint

```sh
cargo check -p morse-app --target wasm32-unknown-unknown
cargo check -p morse-app
cargo test
dx serve
```

All four should succeed. Progress now survives a page reload, and the physical
keyboard works. The web build has the browser; the native build has stubs; the
UI above knows about neither.

## Exercises

1. *(compiler)* Remove `"Storage"` from the `web-sys` feature list and build for
   wasm. The error looks like a missing import but is a missing feature. Add it
   back.

2. *(compiler)* Delete `closure.forget()`. It compiles. Run it and press a key —
   the console reports a closure invoked after being dropped. A whole class of
   FFI bug that the borrow checker cannot see.

3. *(build)* Add `pub fn now_ms() -> f64` — `js_sys::Date::now()` on wasm, `0.0`
   on native. Chapter 8 needs it for straight-key timing.

4. *(build)* Add a `load_flag`/`save_flag`-backed setting of your own, such as a
   persisted WPM. Note that WPM is a `u32`, not a `bool` — do you generalise the
   functions, add a parallel pair, or serialise through a string?

5. *(think)* `platform.rs` has two implementations of every function. At what
   point does that become unmanageable, and what would you switch to — a trait
   with per-platform `impl`s? Separate `platform/web.rs` and `platform/native.rs`
   modules? What does each cost in indirection?

---

Next: [Chapter 8 — Audio and keying](08-audio-keyer-persistence.md), where the
trainer finally makes a sound.
