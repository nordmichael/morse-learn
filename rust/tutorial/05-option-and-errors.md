# Chapter 5 — No unwrap()

> **You'll build:** score serialisation, the logic behind saving progress.
> **You'll learn:** `Option` combinators, `Result`, `?`, `let else`, and how to
> decide when panicking is the right answer.

Rust has no exceptions. Nothing unwinds past your function unless you panic, and
panicking is for bugs, not for expected failures. Everything that can fail says
so in its return type.

That sounds burdensome. In practice it means failure paths are visible in the
signature and the compiler will not let you forget one — and the standard library
gives you enough combinators that handling them is rarely more than a line.

## The two types

**`Option<T>`** — a value may be absent. Absence is not an error; it is a normal
outcome. `to_morse('5')` is `None` because `5` has no Morse code, and that is
simply a fact.

**`Result<T, E>`** — an operation may fail, and there is something to say about
*why*:

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

The rule of thumb: if the caller would ask "why did that fail?", use `Result`. If
the only sensible question is "was there one?", use `Option`.

`"abc".parse::<i32>()` returns `Result` — the failure has a reason (invalid
digit, overflow). `vec.first()` returns `Option` — the only reason is that the
vector was empty, and the name already says that.

## Building something that can fail

Progress is saved as a comma-separated string: `"2,1,0,-1,..."`. Add to
`crates/morse-core/src/trainer.rs`:

```rust
/// Encode per-letter scores for persistence.
pub fn encode_scores(scores: &[i32; 26]) -> String {
    scores
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Decode per-letter scores. Unparseable or missing entries stay at zero, so a
/// corrupt or truncated save degrades to partial progress rather than failing.
pub fn decode_scores(raw: &str) -> [i32; 26] {
    let mut scores = [0; 26];
    for (slot, part) in scores.iter_mut().zip(raw.split(',')) {
        if let Ok(v) = part.trim().parse::<i32>() {
            *slot = v;
        }
    }
    scores
}
```

Several things earn their place here.

**`collect::<Vec<_>>()`** uses turbofish syntax to name the target type inline.
The `_` lets Rust infer the element type. You could equally write
`let parts: Vec<String> = ...`.

**`iter_mut()`** yields `&mut i32` — mutable references to each slot — so `*slot = v`
writes through into the array.

**`zip`** pairs two iterators and stops at the shorter one. That single choice
handles both malformed cases for free: a save with 10 entries fills 10 slots and
leaves the rest zero; a save with 40 fills 26 and ignores the rest. No length
check, no bounds error.

**`if let Ok(v)`** takes the value when parsing succeeded and skips otherwise. The
error is deliberately discarded — a corrupt digit means "we don't know this
letter's score", and zero is the honest answer.

This function **cannot fail**, which is why it returns `[i32; 26]` and not
`Result`. That is a design decision, not laziness: what would a caller do with an
error? Refuse to start? Losing some progress is strictly better than refusing to
run, so the failure is absorbed where it happens.

Tests:

```rust
    #[test]
    fn scores_round_trip() {
        let mut scores = [0; 26];
        scores[0] = 2;
        scores[5] = -1;
        scores[25] = 4;
        assert_eq!(decode_scores(&encode_scores(&scores)), scores);
    }

    #[test]
    fn decoding_survives_corrupt_input() {
        assert_eq!(decode_scores("")[0], 0);
        assert_eq!(decode_scores("1,two,3")[0], 1);
        assert_eq!(decode_scores("1,two,3")[1], 0); // unparseable -> 0
        assert_eq!(decode_scores("1,two,3")[2], 3); // recovery continues
        assert_eq!(decode_scores("9,9,9")[25], 0);  // short input -> rest zero
    }
```

That second test is the valuable one. It states the corruption policy as
executable documentation: bad entries become zero, and parsing keeps going.

## The combinator vocabulary

You will reach for these constantly. Each replaces a `match` you would otherwise
write by hand.

```rust
let code: Option<&str> = to_morse('s');

code.unwrap_or("")                    // value, or a default
code.unwrap_or_else(|| expensive())   // default computed only if needed
code.unwrap_or_default()              // default via the Default trait
code.map(|c| c.len())                 // transform the value, keep None
code.and_then(|c| first_symbol(c))    // chain another Option-returning call
code.filter(|c| !c.is_empty())        // Some -> None if predicate fails
code.is_some() / code.is_none()       // just ask
code.ok_or("no such letter")          // Option -> Result
```

The two most confused are `map` and `and_then`:

- `map` when your closure returns a plain value: `Option<T>` → `Option<U>`
- `and_then` when your closure itself returns an `Option`: `Option<T>` → `Option<U>`,
  flattening

Using `map` where you need `and_then` gives you `Option<Option<U>>`, and the
error message says exactly that. If you know `flatMap` from another language,
`and_then` is it.

Here is a real chain from this project's browser storage layer (Chapter 7):

```rust
local_storage()                               // Option<Storage>
    .and_then(|s| s.get_item(KEY)             //   Result<Option<String>, JsValue>
        .ok()                                 //   Option<Option<String>>
        .flatten())                           //   Option<String>
```

Four fallible steps — no browser, no storage available, the call failing, the key
being absent — collapse into one expression that yields `Option<String>`. Written
with nested `match` it would be a dozen lines and a pyramid. `.ok()` discards a
`Result`'s error to make it an `Option`; `.flatten()` collapses the nesting.

## `Result` and `?`

When the reason matters, use `Result`. The `?` operator makes chains readable:

```rust
fn parse_wpm(input: &str) -> Result<u32, std::num::ParseIntError> {
    let n: u32 = input.trim().parse()?;   // return Err early, or unwrap Ok
    Ok(n.clamp(MIN_WPM, MAX_WPM))
}
```

`?` means: if `Err`, return it from this function immediately; if `Ok`, evaluate
to the inner value. It is the whole of Rust's error propagation — no stack
unwinding, no hidden control flow, just an early return you can see.

`?` also works on `Option`, returning `None` early:

```rust
fn first_symbol(code: &str) -> Option<char> {
    let c = code.chars().next()?;
    if c == '.' || c == '-' { Some(c) } else { None }
}
```

You cannot mix them in one function — a function returning `Option` cannot use
`?` on a `Result`. Convert with `.ok()` or `.ok_or(...)`.

## `let else`

Sometimes you want the value or an early exit, without nesting the rest of the
function inside an `if let`:

```rust
let Some(code) = to_morse(letter) else {
    return;   // must diverge: return, break, continue, or panic
};
// `code` is in scope for the rest of the function, un-nested
```

Compare against the `if let` version, which pushes everything one level deeper
and makes the happy path progressively more indented. The real `platform.rs` uses
`let else` in exactly this shape:

```rust
let Some(ctx) = audio_ctx() else {
    return;   // no audio context available — silently do nothing
};
```

Reach for `let else` whenever the `None` case means "give up here". It keeps the
main path at the left margin, which is worth a surprising amount of readability.

## When panicking is correct

`unwrap` and `expect` panic on `None`/`Err`. They are not automatically wrong —
they are the right call when the failure would mean a **bug in your own code**
rather than a bad input.

Look at the audio code from Chapter 8:

```rust
let osc = ctx.create_oscillator().expect("oscillator");
```

If the browser hands us an `AudioContext` and then refuses to create an
oscillator from it, something is deeply broken in a way we cannot sensibly
handle. Continuing would mean silently producing no sound with no explanation.
Panicking gives a clear message at the point of failure.

Contrast with the same file's handling of storage:

```rust
if let Some(storage) = local_storage() {
    let _ = storage.set_item(STORAGE_KEY, &encoded);
}
```

Storage genuinely might not exist — private browsing, disabled cookies, a quota
that is full. Failing to save progress must not take down the app, so the error
is discarded deliberately. `let _ =` is how you say "I know this returns
something and I am ignoring it on purpose" — without it, the compiler warns about
an unused `Result`, and that warning is one of the more valuable ones.

A working set of guidelines:

| Situation | Use |
|-----------|-----|
| Caller can reasonably recover | `Result` / `Option` |
| Failure means a bug in your code | `expect("why this can't fail")` |
| Failure is expected and ignorable | `if let` / `let _ =` / `unwrap_or` |
| In a test | `unwrap`/`expect` freely — a panic *is* the failure report |
| Library code, any doubt | `Result` — let the caller decide |

Always prefer `expect("message")` to bare `unwrap()`. The cost is a few seconds
of typing; the benefit is that the panic output names the assumption that broke
instead of just a file and line.

And a warning that Chapter 9 pays off in full: **`unwrap` inside a library, on an
invariant the caller can violate, produces terrible failures.** The panic
surfaces deep in someone else's stack trace with no indication of what you did
wrong. That is precisely the bug this project hit — a Dioxus internal
`.unwrap()` on an empty scope stack, triggered by calling an API from the wrong
context. The panic message named a file inside `dioxus-core`, not the mistake in
our code.

## Checkpoint

```sh
cargo test -p morse-core
```

Around 18 tests. `morse-core` is now feature-complete for the language portion of
this tutorial: alphabet, timing, trainer, persistence. No dependencies, no
platform, everything tested.

## Exercises

1. *(build)* Rewrite `decode_scores` using `filter_map` and `enumerate` instead
   of `zip`. Which reads better? Which handles a 40-entry input more obviously?

2. *(compiler)* Change `decode_scores` to return `Result<[i32; 26], String>`,
   erroring on any unparseable entry. Then update the callers. Notice how the
   error propagates outward and someone eventually has to decide what to do —
   this is the cost `Result` imposes, and why absorbing the failure locally was
   the better call here.

3. *(build)* Write `parse_wpm_pair(s: &str) -> Option<(u32, u32)>` parsing
   `"18/8"` into character and effective WPM. Use `?` and `split_once`.

4. *(compiler)* Replace a `let else` with `if let` and observe the indentation of
   the rest of the function. Then replace it with `.expect(...)` and think about
   what the user experiences when that path is hit in production.

5. *(think)* `decode_scores` silently zeroes corrupt entries. That is right for a
   learning app. Name a kind of application where it would be seriously wrong,
   and say what the signature should be there instead.

---

Next: [Chapter 6 — Dioxus](06-dioxus.md), where the logic gets a user interface
and we meet traits, closures, and reactive signals.
