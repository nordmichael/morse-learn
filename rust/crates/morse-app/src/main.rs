// Morse trainer — Dioxus UI.
//
// A clean rewrite of the original Phaser game as a Dioxus app. The same binary
// targets the web (WASM), Android, iOS, and desktop; all learning logic and
// timing live in the platform-agnostic `morse-core` crate, so this file is
// purely presentation + input plumbing.
//
// Features:
//   * Both directions + listening — four drills (see-code→letter,
//     see-letter→send, hear→send echo, hear→letter).
//   * Speed 5–40 WPM with separate character/effective (Farnsworth) control,
//     plus an optional auto speed-ramp driven by rolling accuracy.
//   * Three input methods: two-key paddle, single straight key (by press
//     length, auto-committing on a gap), and a physical keyboard.

use dioxus::prelude::*;
use morse_core::{
    Answer, Drill, InputMethod, Judgement, Prompt, SpeedAdvice, SpeedCoach, Timing, Trainer,
    MAX_WPM, MIN_WPM,
};

mod platform;

const STYLE: Asset = asset!("/assets/style.css");
const MANIFEST: Asset = asset!("/assets/manifest.webmanifest");
const ICON: Asset = asset!("/assets/icon.png");

fn main() {
    dioxus::launch(App);
}

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
    wpm: Signal<u32>,     // character speed
    eff_wpm: Signal<u32>, // effective (Farnsworth) speed, <= wpm
    key_gen: Signal<u64>, // bumped on every key event to cancel stale auto-commits
    press_start: Signal<Option<f64>>, // straight-key press timestamp (ms)
    coach: Signal<SpeedCoach>,
    auto_speed: Signal<bool>,
    show_hints: Signal<bool>, // show the rhythm-mnemonic hint (persisted)
}

impl Ctx {
    /// The active timing from the character/effective speeds. Equal speeds give
    /// standard PARIS timing; a lower effective speed gives Farnsworth spacing.
    fn timing(&self) -> Timing {
        Timing::farnsworth((self.wpm)(), (self.eff_wpm)())
    }
}

/// Play the current target's code if the active drill is an audio one.
fn play_if_audio(ctx: Ctx) {
    if (ctx.drill)().is_audio() {
        platform::play_code(ctx.trainer.read().target_morse(), ctx.timing());
    }
}

/// Feed an answer to the speed coach and, if auto speed is on, apply its advice
/// to the WPM knobs: speed up by closing the Farnsworth gap (then raising the
/// character speed once it's closed), slow down by widening the gap.
fn record_and_ramp(mut ctx: Ctx, correct: bool) {
    let advice = ctx.coach.write().record(correct);
    if !(ctx.auto_speed)() {
        return;
    }
    match advice {
        SpeedAdvice::Faster => {
            let (mut c, mut e) = ((ctx.wpm)(), (ctx.eff_wpm)());
            if e < c {
                e += 1;
            } else if c < MAX_WPM {
                c += 1;
                e += 1;
            }
            ctx.wpm.set(c.min(MAX_WPM));
            ctx.eff_wpm.set(e.min(c).min(MAX_WPM));
        }
        SpeedAdvice::Slower => {
            let e = (ctx.eff_wpm)();
            if e > MIN_WPM {
                ctx.eff_wpm.set(e - 1);
            }
        }
        SpeedAdvice::Hold => {}
    }
}

/// Commit the keyed Morse in the buffer as an answer.
fn commit_code(mut ctx: Ctx) {
    if (ctx.buffer)().is_empty() {
        return;
    }
    let judgement = ctx.trainer.write().submit_morse(&(ctx.buffer)());
    ctx.flash.set(Some(judgement));
    ctx.buffer.set(String::new());
    platform::save_scores(&ctx.trainer.read().scores());
    record_and_ramp(ctx, judgement == Judgement::Correct);
    play_if_audio(ctx);
}

/// Recognition answer: record the picked letter.
fn answer_letter(mut ctx: Ctx, letter: char) {
    let judgement = ctx.trainer.write().submit(letter);
    ctx.flash.set(Some(judgement));
    ctx.buffer.set(String::new());
    platform::save_scores(&ctx.trainer.read().scores());
    record_and_ramp(ctx, judgement == Judgement::Correct);
    play_if_audio(ctx);
}

/// Route a physical key *press* to the current drill. In send drills: **Space is
/// a straight key** — hold it and a sidetone sounds until you release (see
/// [`key_up`]); `.`/`j` key a dit, `-`/`k` a dah, Enter commits, Backspace/Escape
/// clears. In the recognition drill an `a`–`z` key answers directly. Returns
/// whether the key was handled (so its default browser action is suppressed).
fn key_down(mut ctx: Ctx, key: &str) -> bool {
    let sending = (ctx.drill)().answer() == Answer::SendCode;

    // Space held = straight-key press: start the sidetone and the press timer.
    if key == " " {
        if !sending {
            return false;
        }
        ctx.flash.set(None);
        ctx.key_gen.with_mut(|g| *g += 1); // cancel any pending auto-commit
        ctx.press_start.set(Some(platform::now_ms()));
        platform::tone_on();
        return true;
    }

    match (ctx.drill)().answer() {
        Answer::SendCode => match key {
            "." | "j" | "J" => {
                ctx.flash.set(None);
                ctx.buffer.with_mut(|b| b.push('.'));
                true
            }
            "-" | "k" | "K" => {
                ctx.flash.set(None);
                ctx.buffer.with_mut(|b| b.push('-'));
                true
            }
            "Enter" => {
                commit_code(ctx);
                true
            }
            "Backspace" | "Escape" => {
                ctx.buffer.set(String::new());
                true
            }
            _ => false,
        },
        Answer::TypeLetter => {
            let mut chars = key.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => {
                    answer_letter(ctx, c.to_ascii_lowercase());
                    true
                }
                _ => false,
            }
        }
    }
}

/// Handle a key *release*. Only Space matters: it ends a straight-key press —
/// stop the sidetone, classify the hold as a dit or dah by its duration, append
/// it, and schedule the letter's auto-commit.
fn key_up(mut ctx: Ctx, key: &str) -> bool {
    if key != " " {
        return false;
    }
    platform::tone_off();
    if let Some(started) = (ctx.press_start)() {
        let held = (platform::now_ms() - started).max(0.0) as u32;
        let symbol = ctx.timing().classify_press(held);
        ctx.buffer.with_mut(|b| b.push(symbol.as_char()));
        ctx.press_start.set(None);
        ctx.key_gen.with_mut(|g| *g += 1);
        schedule_autocommit(ctx);
        return true;
    }
    false
}

/// After a straight-key press, schedule an automatic letter commit once the key
/// has been idle for an inter-character gap — just like releasing a real key. A
/// generation counter cancels the pending commit if the learner keeps keying.
#[cfg(target_arch = "wasm32")]
fn schedule_autocommit(ctx: Ctx) {
    let generation = (ctx.key_gen)();
    let delay = ctx.timing().char_gap_ms();
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(delay).await;
        if (ctx.key_gen)() == generation && !(ctx.buffer)().is_empty() {
            commit_code(ctx);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn schedule_autocommit(_ctx: Ctx) {}

#[component]
fn App() -> Element {
    let mut ctx = Ctx {
        trainer: use_signal(|| Trainer::restore(platform::load_scores(), platform::seed())),
        buffer: use_signal(String::new),
        flash: use_signal(|| Option::<Judgement>::None),
        drill: use_signal(|| Drill::HearCodeSendCode),
        method: use_signal(|| InputMethod::TwoKey),
        wpm: use_signal(|| 15u32),
        eff_wpm: use_signal(|| 15u32),
        key_gen: use_signal(|| 0u64),
        press_start: use_signal(|| Option::<f64>::None),
        coach: use_signal(SpeedCoach::default),
        auto_speed: use_signal(|| false),
        show_hints: use_signal(|| platform::load_flag("show_hints", true)),
    };

    // Register a document-level keyboard listener once, so the app is fully
    // playable from a physical keyboard without clicking to focus.
    #[cfg(target_arch = "wasm32")]
    use_hook(move || {
        platform::on_keydown(move |key| key_down(ctx, &key));
        platform::on_keyup(move |key| key_up(ctx, &key));
    });

    // --- derived view state ----------------------------------------------

    let current = (ctx.drill)();
    let target = ctx.trainer.read().target_letter();
    let target_morse = ctx.trainer.read().target_morse();
    let active = ctx.trainer.read().letter_index();
    let word = ctx.trainer.read().current_word().to_string();
    let completion = (ctx.trainer.read().completion() * 100.0).round() as u32;
    let hint = ctx.trainer.read().hint_level();
    let buffer_text = (ctx.buffer)();
    let buffer_empty = buffer_text.is_empty();

    let flash_class = match (ctx.flash)() {
        Some(Judgement::Correct) => "flash-correct",
        Some(Judgement::Incorrect) => "flash-incorrect",
        None => "",
    };

    // The word shown as progress: letters already answered are revealed, the
    // current and upcoming ones are masked (so audio / see-code drills don't
    // give the answer away).
    let masked: Vec<(usize, String, &'static str)> = word
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let (text, class) = if i < active {
                (ch.to_string(), "slot done")
            } else if i == active {
                ("·".to_string(), "slot active")
            } else {
                ("·".to_string(), "slot")
            };
            (i, text, class)
        })
        .collect();

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }
        document::Link { rel: "manifest", href: MANIFEST }
        document::Link { rel: "icon", href: ICON }
        document::Link { rel: "apple-touch-icon", href: ICON }
        document::Meta { name: "theme-color", content: "#ef4136" }
        document::Meta { name: "apple-mobile-web-app-capable", content: "yes" }
        document::Meta { name: "mobile-web-app-capable", content: "yes" }
        document::Title { "Morse Learn" }

        main { class: "app",
            SettingsBar { ctx }

            header { class: "top",
                div { class: "bar", div { class: "bar-fill", style: "width: {completion}%;" } }
                span { class: "pct", "{completion}%" }
            }
            ProgressLights { trainer: ctx.trainer }

            div { class: "slots",
                for (i, text, class) in masked {
                    span { key: "{i}", class, "{text}" }
                }
            }

            // ---- prompt: how the target is presented ----
            section { class: "prompt {flash_class}",
                match current.prompt() {
                    Prompt::SeeLetter => rsx! { div { class: "prompt-letter", "{target}" } },
                    Prompt::SeeCode => rsx! { div { class: "prompt-code", "{target_morse}" } },
                    Prompt::HearCode => rsx! {
                        div { class: "prompt-hear",
                            button {
                                class: "play",
                                onclick: move |_| platform::play_code(ctx.trainer.read().target_morse(), ctx.timing()),
                                "🔊 play"
                            }
                            if current.shows_letter() {
                                div { class: "prompt-letter", "{target}" }
                            }
                        }
                    },
                }
            }

            if (ctx.show_hints)() {
                Hint { letter: target, level: hint }
            }

            // ---- answer area: depends on the drill ----
            match current.answer() {
                Answer::TypeLetter => rsx! {
                    LetterPad { trainer: ctx.trainer, on_pick: move |c| answer_letter(ctx, c) }
                    p { class: "kbdhint", "tap a letter — or type a–z on a keyboard" }
                },
                Answer::SendCode => rsx! {
                    div { class: "buffer",
                        if buffer_empty {
                            span { class: "buffer-empty", "key the code" }
                        } else {
                            "{buffer_text}"
                        }
                    }
                    match (ctx.method)() {
                        InputMethod::TwoKey => rsx! {
                            div { class: "keys",
                                button {
                                    class: "key dot",
                                    onclick: move |_| { ctx.flash.set(None); ctx.buffer.with_mut(|b| b.push('.')); },
                                    "•"
                                }
                                button {
                                    class: "key dash",
                                    onclick: move |_| { ctx.flash.set(None); ctx.buffer.with_mut(|b| b.push('-')); },
                                    "—"
                                }
                            }
                        },
                        InputMethod::StraightKey => rsx! {
                            div { class: "keys",
                                button {
                                    class: "key straight",
                                    onpointerdown: move |_| {
                                        ctx.flash.set(None);
                                        ctx.key_gen.with_mut(|g| *g += 1); // cancel any pending auto-commit
                                        ctx.press_start.set(Some(platform::now_ms()));
                                        platform::tone_on(); // sidetone for the whole press
                                    },
                                    onpointerup: move |_| {
                                        platform::tone_off();
                                        if let Some(started) = (ctx.press_start)() {
                                            let held = (platform::now_ms() - started).max(0.0) as u32;
                                            let symbol = ctx.timing().classify_press(held);
                                            ctx.buffer.with_mut(|b| b.push(symbol.as_char()));
                                        }
                                        ctx.press_start.set(None);
                                        // Auto-commit the letter after an inter-character gap.
                                        ctx.key_gen.with_mut(|g| *g += 1);
                                        schedule_autocommit(ctx);
                                    },
                                    // Stop the tone if the pointer leaves while held.
                                    onpointerleave: move |_| {
                                        if (ctx.press_start)().is_some() {
                                            platform::tone_off();
                                        }
                                    },
                                    "hold · short / long —"
                                }
                            }
                        },
                    }
                    div { class: "keys secondary",
                        button {
                            class: "key clear",
                            onclick: move |_| { ctx.key_gen.with_mut(|g| *g += 1); ctx.buffer.set(String::new()); },
                            "clear"
                        }
                        button {
                            class: "key enter",
                            onclick: move |_| commit_code(ctx),
                            "enter"
                        }
                    }
                    p { class: "kbdhint", "keyboard:  hold Space = straight key  ·  . / j = dit  ·  - / k = dah  ·  Enter = commit  ·  ⌫ = clear" }
                },
            }
        }
    }
}

/// Speed controls + drill picker + key-method toggle + auto-speed switch.
#[component]
fn SettingsBar(ctx: Ctx) -> Element {
    let mut ctx = ctx;
    let drills = Drill::all();
    let current = (ctx.drill)();
    let show_method = current.answer() == Answer::SendCode;
    let char_wpm = (ctx.wpm)();
    let eff = (ctx.eff_wpm)();
    let farnsworth = eff < char_wpm;
    let accuracy_pct = ctx.coach.read().accuracy().map(|a| (a * 100.0).round() as u32);

    rsx! {
        div { class: "settings",
            // character speed
            div { class: "wpm",
                span { class: "wpm-label", "char" }
                button {
                    class: "chip",
                    onclick: move |_| {
                        ctx.wpm.with_mut(|w| *w = w.saturating_sub(1).max(MIN_WPM));
                        ctx.eff_wpm.with_mut(|e| *e = (*e).min((ctx.wpm)()));
                    },
                    "−"
                }
                span { class: "wpm-value", "{char_wpm} WPM" }
                button {
                    class: "chip",
                    onclick: move |_| ctx.wpm.with_mut(|w| *w = (*w + 1).min(MAX_WPM)),
                    "+"
                }
            }

            // effective (Farnsworth) speed
            div { class: "wpm",
                span { class: "wpm-label", "effective" }
                button {
                    class: "chip",
                    onclick: move |_| ctx.eff_wpm.with_mut(|e| *e = e.saturating_sub(1).max(MIN_WPM)),
                    "−"
                }
                span {
                    class: if farnsworth { "wpm-value farnsworth" } else { "wpm-value" },
                    "{eff} WPM"
                }
                button {
                    class: "chip",
                    onclick: move |_| ctx.eff_wpm.with_mut(|e| *e = (*e + 1).min((ctx.wpm)())),
                    "+"
                }
            }

            // feature toggles: hints + auto speed-ramp, with live accuracy
            div { class: "method",
                button {
                    class: if (ctx.show_hints)() { "tab on" } else { "tab" },
                    onclick: move |_| {
                        let now = !(ctx.show_hints)();
                        ctx.show_hints.set(now);
                        platform::save_flag("show_hints", now);
                    },
                    "Hints"
                }
                button {
                    class: if (ctx.auto_speed)() { "tab on" } else { "tab" },
                    onclick: move |_| {
                        let now = !(ctx.auto_speed)();
                        ctx.auto_speed.set(now);
                        if now { ctx.coach.write().reset(); }
                    },
                    "Auto speed"
                }
                if let Some(acc) = accuracy_pct {
                    span { class: "acc", "{acc}% acc" }
                }
            }

            // drill picker
            div { class: "drills",
                for d in drills {
                    button {
                        key: "{d.label()}",
                        class: if d == current { "tab on" } else { "tab" },
                        onclick: move |_| {
                            ctx.drill.set(d);
                            ctx.buffer.set(String::new());
                        },
                        "{d.label()}"
                    }
                }
            }

            // key-method toggle (only relevant for send drills)
            if show_method {
                div { class: "method",
                    button {
                        class: if (ctx.method)() == InputMethod::TwoKey { "tab on" } else { "tab" },
                        onclick: move |_| { ctx.method.set(InputMethod::TwoKey); ctx.buffer.set(String::new()); },
                        "Two keys"
                    }
                    button {
                        class: if (ctx.method)() == InputMethod::StraightKey { "tab on" } else { "tab" },
                        onclick: move |_| { ctx.method.set(InputMethod::StraightKey); ctx.buffer.set(String::new()); },
                        "Straight key"
                    }
                }
            }
        }
    }
}

/// A grid of the letters currently in play, for recognition answers.
#[component]
fn LetterPad(trainer: Signal<Trainer>, on_pick: EventHandler<char>) -> Element {
    let mut letters = trainer.read().letters_in_play().to_vec();
    letters.sort_unstable();
    rsx! {
        div { class: "letterpad",
            for letter in letters {
                button {
                    key: "{letter}",
                    class: "letter-key",
                    onclick: move |_| on_pick.call(letter),
                    "{letter}"
                }
            }
        }
    }
}

/// The row of 26 dots under the header, one per letter in learning order.
#[component]
fn ProgressLights(trainer: Signal<Trainer>) -> Element {
    let lights: Vec<(char, &'static str)> = trainer
        .read()
        .progress()
        .into_iter()
        .map(|p| (p.letter, if p.learned { "light on" } else { "light" }))
        .collect();
    rsx! {
        div { class: "lights",
            for (letter, class) in lights {
                span { key: "{letter}", class, title: "{letter}" }
            }
        }
    }
}

/// The rhythm-mnemonic hint for the current letter: a word whose spoken cadence
/// is the letter's Morse rhythm (stressed syllable = dah, short syllable = dit).
/// At the higher hint level the dit/dah marks are shown under each syllable.
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
            if let Some(note) = m.note {
                div { class: "mnem-note", "♪ {note}" }
            }
        }
    }
}
