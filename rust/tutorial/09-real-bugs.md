# Chapter 9 — Real bugs

> **You'll learn:** how to read a panic, why `RefCell` moves borrow checking to
> runtime, and the three categories of bug Rust's type system does *not* catch.

Every chapter so far worked the first time. That is not what building software is
like, and a tutorial where nothing breaks teaches you only half the job.

These three bugs are real. They happened in this repository, in this order, and
each one is a different category of thing the compiler cannot see.

---

## Bug 1 — the panic that named someone else's file

### The symptom

The app compiled cleanly. `cargo check` passed for wasm and native. All 37 tests
passed. Then, holding the space bar:

```
09:57:33 [web] panicked at dioxus-core-0.7.9\src\r
called `Option::unwrap()` on a `None` value

09:57:33 [web] panicked at dioxus-core-0.7.9\src\r
RefCell already borrowed

09:57:33 [web] wasm-bindgen: imported JS function that was not marked as `catch` threw an error
```

Two panics, both inside `dioxus-core`, neither in our code. The file path is
truncated to `src\r`. There is no line number we can act on and no mention of any
function we wrote.

This is the hardest kind of panic to read, and the technique for it is worth
learning properly.

### Step 1: which of our code runs at that moment?

The panic fires on space bar release. Working backwards through what that path
touches:

```
key_up()  ->  schedule_autocommit()  ->  spawn(async move { ... })
```

`spawn` is the only framework call in that chain. Everything else is our own
plain Rust.

### Step 2: read the library source

The panic says `Option::unwrap()` on `None`, somewhere in a `dioxus-core` file
starting with `r`. Cargo keeps dependency sources locally, so we can just look:

```sh
ls ~/.cargo/registry/src/*/dioxus-core-0.7.9/src/ | grep '^r'
```

```
reactive_context.rs
render_error.rs
root_wrapper.rs
runtime.rs
```

`runtime.rs` is the likely one. Grep it for unwraps:

```sh
grep -n "unwrap()" runtime.rs
```

```
223:        self.scope_stack.borrow().last().copied().unwrap()
```

And what calls that? Following `spawn` down:

```rust
pub fn spawn(fut: impl Future<Output = ()> + 'static) -> Task {
    Runtime::with_current_scope(|cx| cx.spawn(fut))
}

pub(crate) fn with_current_scope<R>(callback: impl FnOnce(&Scope) -> R) -> R {
    Self::with(|rt| Self::with_scope(rt.current_scope_id(), callback))
}

pub fn current_scope_id(&self) -> ScopeId {
    self.scope_stack.borrow().last().copied().unwrap()   // <- line 223
}
```

There it is. `spawn` asks for the current scope. `current_scope_id` reads the top
of a stack and unwraps. **If the stack is empty, this panics.**

### Step 3: why was the stack empty?

Look again at how the keyboard handler was registered in Chapter 7:

```rust
use_hook(move || {
    platform::on_keydown(move |key| key_down(ctx, &key));
    platform::on_keyup(move |key| key_up(ctx, &key));
});
```

`on_keyup` registers a **raw DOM callback** via `Closure::wrap`. When you release
the space bar, the *browser* calls that closure directly. Dioxus is not involved
in the call at all.

Dioxus pushes a scope onto `scope_stack` while rendering a component and while
dispatching one of its own events. A raw browser callback happens outside both,
so the stack is empty — and `spawn` unwraps `None`.

The second panic, `RefCell already borrowed`, is a *cascade*. The first panic
unwound while `Runtime::with` held a borrow of the runtime thread-local, so the
next event's `borrow_mut()` found it still borrowed. Fix the first and the second
disappears. When you see two panics microseconds apart, suspect this — chasing the
second one first wastes hours.

### The fix

We need the handlers to run inside the scope. Capture it during `use_hook`, which
*does* run inside the component's scope, and re-enter it on each call:

```rust
// These are raw DOM callbacks, so the browser invokes them with no Dioxus
// scope on the stack — `spawn` (the straight-key auto-commit) would panic
// looking for the current scope. Capture this scope while we're still
// inside it and re-enter it for the duration of each handler.
#[cfg(target_arch = "wasm32")]
use_hook(move || {
    let runtime = dioxus::core::Runtime::current();
    let scope = dioxus::core::current_scope_id();
    let up_runtime = runtime.clone();
    platform::on_keydown(move |key| runtime.in_scope(scope, || key_down(ctx, &key)));
    platform::on_keyup(move |key| up_runtime.in_scope(scope, || key_up(ctx, &key)));
});
```

`Runtime::in_scope` pushes the scope, runs the closure, pops it. `spawn` now finds
what it expects.

The `runtime.clone()` is needed because the two closures each need their own
handle — `Runtime` is an `Rc`, so cloning is a refcount bump, not a deep copy.

This fixed a second problem we had not noticed. The signal writes in `key_down`
were *also* happening outside the reactive context, so Dioxus's dirty-tracking was
unreliable — the UI sometimes did not re-render after a keystroke. That had been
dismissed as a rendering quirk. It was the same bug.

### The lessons

**A panic inside a library usually means you violated an invariant it did not
state in its types.** `spawn` requires a scope; nothing in its signature says so.
When a library panics, the question is rarely "is the library broken" and usually
"what contract am I breaking?"

**Read the dependency source.** It is on your disk. Ten minutes in `runtime.rs`
beat any amount of guessing.

**FFI boundaries are where framework guarantees stop.** Any callback the browser
invokes directly — event listeners, `setTimeout`, `IntersectionObserver` — runs
outside your framework's context. This applies equally in JavaScript frameworks;
it is just more visible here because the failure is a panic rather than a subtly
stale render.

**`cargo check` proves nothing about runtime context.** Types cannot express
"there must be a scope on the stack right now."

---

## Bug 2 — the logic bug that no test failed on

### The symptom

A learner reported that letters they had already learned, then forgotten, never
showed a hint no matter how many times they missed them. The letters most in need
of help got the least.

Here is the original `hint_level`:

```rust
pub fn hint_level(&self) -> HintLevel {
    if self.score_of(self.target_letter()) >= LEARNED_THRESHOLD {
        HintLevel::None
    } else if self.mistakes_on_letter >= 4 {
        HintLevel::Pattern
    } else {
        HintLevel::Picture
    }
}
```

Read the branches in order. If a letter is learned, return `None` — **and stop**.
The mistake count is never consulted for a learned letter. A letter you learned
last week and have now forgotten returns `HintLevel::None` on the tenth
consecutive miss.

Every test passed. There were tests for hints, and they covered the fresh-letter
path and the learned path — but no test asked "what happens when a *learned*
letter is missed repeatedly," because nobody thought of that state.

### The fix

Reorder, so struggling wins:

```rust
/// `STRUGGLE_MISTAKES` consecutive misses on the same letter reveal its
/// pattern *even if the letter was already learned* — a learned letter can
/// still go stale, and that is exactly when the reminder is worth most.
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

Three lines moved. The fix also folded two thresholds (the old `>= 4` for
unlearned letters) into one named constant, which is less to explain and less to
get wrong.

And a test for the state nobody had considered:

```rust
#[test]
fn a_learned_letter_still_gets_a_hint_when_it_goes_stale() {
    let mut t = Trainer::new(1);
    let target = t.target_letter();
    for _ in 0..LEARNED_THRESHOLD {
        t.bump_score(target, 1);
    }
    assert_eq!(t.hint_level(), HintLevel::None);

    let wrong = if target == 'e' { 't' } else { 'e' };
    for _ in 0..STRUGGLE_MISTAKES {
        t.submit(wrong);
    }
    assert_eq!(t.hint_level(), HintLevel::Pattern);
}
```

### The lessons

**Rust's type system has nothing to say about branch order.** Both versions
compile, both are total, both return a valid `HintLevel`. This is an ordinary
logic bug and no language feature prevents it.

**Coverage is not the same as thinking about states.** Every line of
`hint_level` was executed by the existing tests. The bug lived in a *combination*
of states — learned *and* struggling — that no test constructed.

**When you find a bug like this, the test you write matters more than the fix.**
The fix is three lines. The test is what stops it coming back, and its name
documents the state that was overlooked.

---

## Bug 3 — the API that made the feature impossible

### The symptom

The request: show a check-mark beside a completed word before loading the next
one.

The original `register_correct`:

```rust
self.letter_index += 1;
if self.letter_index >= self.current_word.chars().count() {
    // Word finished — try to unlock a new letter, then fetch the next word.
    self.maybe_unlock_letter();
    self.recompute_letters_in_play();
    self.current_word = self.pick_word();
    self.letter_index = 0;
}
```

Finishing the last letter unlocks, picks a new word, and resets the index — all
inside the same call. By the time `submit` returns, **the completed word no longer
exists anywhere in the state**. There is no moment the UI can render.

This is not a crash. It is not even wrong, on its own terms — the state machine is
correct. It simply does not expose the state the feature needs.

### The fix

Split the transition in two:

```rust
self.letter_index += 1;
if self.letter_index >= self.current_word.chars().count() {
    // Word finished. Hold here rather than swapping the word out from
    // under the learner — `next_word` does the advance once the UI has
    // shown the completed word.
    self.word_complete = true;
}
```

```rust
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

One `bool`, one method. Now the UI can render the completed word, wait 800ms, and
call `next_word()`.

### The consequence nobody would have predicted

Splitting the transition created a window in which there is **no target letter** —
`target_letter()` returns `' '`. If an answer arrives during that window, it is
compared against a space, scored as a miss, and `consecutive_correct` is reset to
zero — silently stealing a letter unlock the learner had earned.

That is a nasty bug: intermittent, invisible, and it only manifests as "the app
feels like it isn't progressing". It required guards in three places:

```rust
pub fn submit(&mut self, answer: char) -> Judgement {
    self.next_word();   // an answer implicitly acknowledges a finished word
    // ...
}
```

plus an early return in the UI's answer handlers, plus `disabled: word_done` on
the send buttons.

### The lessons

**"The code is correct" and "the code exposes what consumers need" are different
properties.** A state machine that collapses two transitions into one is not
buggy, but it is unusable by anything that wants to observe the intermediate
state. When a UI feature feels impossible, look at whether the state model has a
state for it.

**Adding a state adds every transition into and out of it.** The `bool` was the
easy part. The real work was the three guards, and none of those were in the
original request.

---

## What the compiler catches, and what it doesn't

Rust eliminates entire bug classes. Being precise about which ones is what stops
you from over-trusting it.

**Caught at compile time:** use-after-free, double-free, data races, null
dereferences, buffer overruns, unhandled enum variants, forgotten `Result`s,
type confusion, iterator invalidation.

That is a genuinely enormous list. Most of it is what makes C and C++ hard.

**Not caught:**

- **Logic errors.** Bug 2. Branch order, wrong comparison, off-by-one within
  bounds. Only tests find these.
- **Runtime borrow violations.** `RefCell`, `Signal`, and any interior mutability
  move the borrow rules to runtime by design. Bug 1's cascade was one of these.
- **Contracts not expressed in types.** "There must be a scope on the stack."
  "Call this before that." Bug 1's root cause.
- **API shape problems.** Bug 3. Everything compiles; the design is simply
  inadequate for what you need.
- **Panics you chose.** Every `unwrap`, every array index, every integer divide.
- **Deadlocks and leaks.** Both are memory-safe and both are bugs. `closure.forget()`
  in Chapter 7 leaks deliberately.

The habits that cover the gap:

**Test states, not lines.** Bug 2 had full line coverage. Enumerate the
*combinations* — learned × struggling, complete × answering.

**Keep guards short-lived.** Bind `let x = signal.read().field;` on its own line
rather than holding a guard across a call that might write.

**Prefer `expect("why")` to `unwrap()`.** When it fires at 2am, the message is
what you have.

**Read your dependencies' source.** It is on your disk, it is Rust you can
already read, and it answers questions no amount of documentation-guessing will.

---

## Where to go next

You have built a real, tested, cross-platform application, and you have debugged
it. The remaining gaps in your Rust, roughly in order of how soon you will need
them:

- **Error types** — `thiserror` for libraries, `anyhow` for applications. This
  project never needed a custom error type; most do.
- **Traits in depth** — generics, trait objects, associated types, and when `dyn`
  is worth the indirection.
- **Async** — we used `spawn` and `.await` without explaining futures, executors,
  or `Pin`. Read [Asynchronous Programming in Rust](https://rust-lang.github.io/async-book/).
- **Concurrency** — `Send`, `Sync`, `Arc<Mutex<T>>`. Wasm is single-threaded so
  this never came up, and it is where ownership pays its biggest dividend.
- **Unsafe** — what it actually permits (five things), and why you rarely need it.

The two references worth owning: [The Rust Book](https://doc.rust-lang.org/book/)
for depth on anything here, and [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
for quick pattern lookup.

And the thing that will teach you fastest: pick a feature this app does not have —
a Koch-method trainer, sending practice with a real paddle, spaced repetition
scheduling — and add it. The tests are already there to tell you when you break
something.

---

Back to the [index](README.md).
