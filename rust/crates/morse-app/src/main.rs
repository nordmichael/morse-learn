// Morse trainer — Dioxus UI.
//
// A clean rewrite of the original Phaser game as a Dioxus app. The same binary
// targets the web (WASM), Android, iOS, and desktop; all learning logic and
// timing live in the platform-agnostic `morse-core` crate, so this file is
// purely presentation + input plumbing.
//
// It supports both directions of the skill plus listening:
//   * See code  → name the letter   (visual recognition)
//   * See letter → send the code     (production / sending)
//   * Hear code → send the code      (intermediate "echo" step)
//   * Hear code → name the letter    (head-copy / receiving)
// Sending can use two keys (dit/dah) or a single straight key (by press
// length), and playback/keying speed runs from 5 up to 40 WPM.

use dioxus::prelude::*;
use morse_core::{Answer, Drill, InputMethod, Judgement, Prompt, Timing, Trainer, MAX_WPM, MIN_WPM};

mod platform;

const STYLE: Asset = asset!("/assets/style.css");

fn main() {
    dioxus::launch(App);
}

/// The active timing from the character-speed and effective-speed signals.
/// When they're equal this is standard PARIS timing; when effective is lower it
/// is Farnsworth (characters at full speed, gaps stretched).
fn current_timing(char_wpm: Signal<u32>, eff_wpm: Signal<u32>) -> Timing {
    Timing::farnsworth(char_wpm(), eff_wpm())
}

/// Play the current target's code if the active drill is an audio one. These are
/// free functions (not closures) so event handlers and the gap timer can all
/// call them — `Signal` is `Copy`, so we just pass the handles in.
fn play_if_audio(trainer: Signal<Trainer>, drill: Signal<Drill>, timing: Timing) {
    if drill().is_audio() {
        platform::play_code(trainer.read().target_morse(), timing);
    }
}

/// Commit the keyed Morse in `buffer` as an answer, flash the result, persist,
/// and (for audio drills) play the next target.
fn commit_code(
    mut trainer: Signal<Trainer>,
    mut buffer: Signal<String>,
    mut flash: Signal<Option<Judgement>>,
    drill: Signal<Drill>,
    timing: Timing,
) {
    if buffer().is_empty() {
        return;
    }
    let judgement = trainer.write().submit_morse(&buffer());
    flash.set(Some(judgement));
    buffer.set(String::new());
    platform::save_scores(&trainer.read().scores());
    play_if_audio(trainer, drill, timing);
}

/// After a straight-key press, schedule an automatic letter commit once the key
/// has been idle for an inter-character gap — just like releasing a real key.
/// `key_gen` is bumped on every key event; if it changed while we waited, the
/// user kept keying, so we don't commit.
#[cfg(target_arch = "wasm32")]
fn schedule_autocommit(
    trainer: Signal<Trainer>,
    buffer: Signal<String>,
    flash: Signal<Option<Judgement>>,
    drill: Signal<Drill>,
    key_gen: Signal<u64>,
    timing: Timing,
) {
    let generation = key_gen();
    let delay = timing.char_gap_ms();
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(delay).await;
        if key_gen() == generation && !buffer().is_empty() {
            commit_code(trainer, buffer, flash, drill, timing);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn schedule_autocommit(
    _trainer: Signal<Trainer>,
    _buffer: Signal<String>,
    _flash: Signal<Option<Judgement>>,
    _drill: Signal<Drill>,
    _key_gen: Signal<u64>,
    _timing: Timing,
) {
}

#[component]
fn App() -> Element {
    let mut trainer = use_signal(|| Trainer::restore(platform::load_scores(), platform::seed()));
    let wpm = use_signal(|| 15u32); // character speed
    let eff_wpm = use_signal(|| 15u32); // effective (Farnsworth) speed, <= wpm
    let drill = use_signal(|| Drill::SeeCodeTypeLetter);
    let method = use_signal(|| InputMethod::TwoKey);

    // In-progress Morse for "send" answers, and the straight-key press start.
    let mut buffer = use_signal(String::new);
    let mut press_start = use_signal(|| Option::<f64>::None);
    // Bumped on every key event so a pending auto-commit can tell it's stale.
    let mut key_gen = use_signal(|| 0u64);
    // Last judgement, used to flash the prompt green/red.
    let mut flash = use_signal(|| Option::<Judgement>::None);

    let timing = current_timing(wpm, eff_wpm);

    // --- answer handlers -------------------------------------------------

    // Recognition answer: the learner picked a letter.
    let mut answer_letter = move |letter: char| {
        let judgement = trainer.write().submit(letter);
        flash.set(Some(judgement));
        buffer.set(String::new());
        platform::save_scores(&trainer.read().scores());
        play_if_audio(trainer, drill, timing);
    };

    // --- derived view state ----------------------------------------------

    let current = drill();
    let target = trainer.read().target_letter();
    let target_morse = trainer.read().target_morse();
    let active = trainer.read().letter_index();
    let word = trainer.read().current_word().to_string();
    let completion = (trainer.read().completion() * 100.0).round() as u32;
    let hint = trainer.read().hint_level();

    let flash_class = match flash() {
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
        document::Title { "Morse Learn" }

        main { class: "app",
            // ---- settings: speed, drill, key method ----
            SettingsBar { wpm, eff_wpm, drill, method, buffer, press_start }

            // ---- progress ----
            header { class: "top",
                div { class: "bar", div { class: "bar-fill", style: "width: {completion}%;" } }
                span { class: "pct", "{completion}%" }
            }
            ProgressLights { trainer }

            // ---- word progress (masked) ----
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
                        button {
                            class: "play",
                            onclick: move |_| platform::play_code(trainer.read().target_morse(), timing),
                            "🔊 play"
                        }
                    },
                }
            }

            // ---- hint (mnemonic picture, then pattern) ----
            Hint { letter: target, level: hint }

            // ---- answer area: depends on the drill ----
            match current.answer() {
                Answer::TypeLetter => rsx! {
                    LetterPad { trainer, on_pick: move |c| answer_letter(c) }
                },
                Answer::SendCode => rsx! {
                    div { class: "buffer",
                        if buffer().is_empty() {
                            span { class: "buffer-empty", "key the code" }
                        } else {
                            "{buffer()}"
                        }
                    }
                    match method() {
                        InputMethod::TwoKey => rsx! {
                            div { class: "keys",
                                button {
                                    class: "key dot",
                                    onclick: move |_| { flash.set(None); buffer.with_mut(|b| b.push('.')); },
                                    "•"
                                }
                                button {
                                    class: "key dash",
                                    onclick: move |_| { flash.set(None); buffer.with_mut(|b| b.push('-')); },
                                    "—"
                                }
                            }
                        },
                        InputMethod::StraightKey => rsx! {
                            div { class: "keys",
                                button {
                                    class: "key straight",
                                    onpointerdown: move |_| {
                                        flash.set(None);
                                        key_gen.with_mut(|g| *g += 1); // cancel any pending auto-commit
                                        press_start.set(Some(platform::now_ms()));
                                    },
                                    onpointerup: move |_| {
                                        if let Some(started) = press_start() {
                                            let held = (platform::now_ms() - started).max(0.0) as u32;
                                            let symbol = timing.classify_press(held);
                                            buffer.with_mut(|b| b.push(symbol.as_char()));
                                        }
                                        press_start.set(None);
                                        // Auto-commit the letter after an inter-character gap.
                                        key_gen.with_mut(|g| *g += 1);
                                        schedule_autocommit(trainer, buffer, flash, drill, key_gen, timing);
                                    },
                                    "press · short / long —"
                                }
                            }
                        },
                    }
                    div { class: "keys secondary",
                        button {
                            class: "key clear",
                            onclick: move |_| { key_gen.with_mut(|g| *g += 1); buffer.set(String::new()); },
                            "clear"
                        }
                        button {
                            class: "key enter",
                            onclick: move |_| commit_code(trainer, buffer, flash, drill, timing),
                            "enter"
                        }
                    }
                },
            }
        }
    }
}

/// Speed control + drill picker + key-method toggle.
#[component]
fn SettingsBar(
    wpm: Signal<u32>,
    eff_wpm: Signal<u32>,
    drill: Signal<Drill>,
    method: Signal<InputMethod>,
    buffer: Signal<String>,
    press_start: Signal<Option<f64>>,
) -> Element {
    let mut wpm = wpm;
    let mut eff_wpm = eff_wpm;
    let mut drill = drill;
    let mut method = method;
    let mut buffer = buffer;
    let mut press_start = press_start;

    let drills = Drill::all();
    let current = drill();
    let show_method = current.answer() == Answer::SendCode;
    let farnsworth = eff_wpm() < wpm();

    rsx! {
        div { class: "settings",
            // character speed
            div { class: "wpm",
                span { class: "wpm-label", "char" }
                button {
                    class: "chip",
                    onclick: move |_| {
                        wpm.with_mut(|w| *w = w.saturating_sub(1).max(MIN_WPM));
                        // keep effective speed at or below character speed
                        eff_wpm.with_mut(|e| *e = (*e).min(wpm()));
                    },
                    "−"
                }
                span { class: "wpm-value", "{wpm()} WPM" }
                button {
                    class: "chip",
                    onclick: move |_| wpm.with_mut(|w| *w = (*w + 1).min(MAX_WPM)),
                    "+"
                }
            }

            // effective (Farnsworth) speed
            div { class: "wpm",
                span { class: "wpm-label", "effective" }
                button {
                    class: "chip",
                    onclick: move |_| eff_wpm.with_mut(|e| *e = e.saturating_sub(1).max(MIN_WPM)),
                    "−"
                }
                span {
                    class: if farnsworth { "wpm-value farnsworth" } else { "wpm-value" },
                    "{eff_wpm()} WPM"
                }
                button {
                    class: "chip",
                    onclick: move |_| eff_wpm.with_mut(|e| *e = (*e + 1).min(wpm())),
                    "+"
                }
            }

            // drill picker
            div { class: "drills",
                for d in drills {
                    button {
                        key: "{d.label()}",
                        class: if d == current { "tab on" } else { "tab" },
                        onclick: move |_| {
                            drill.set(d);
                            buffer.set(String::new());
                            press_start.set(None);
                        },
                        "{d.label()}"
                    }
                }
            }

            // key-method toggle (only relevant for send drills)
            if show_method {
                div { class: "method",
                    button {
                        class: if method() == InputMethod::TwoKey { "tab on" } else { "tab" },
                        onclick: move |_| { method.set(InputMethod::TwoKey); buffer.set(String::new()); },
                        "Two keys"
                    }
                    button {
                        class: if method() == InputMethod::StraightKey { "tab on" } else { "tab" },
                        onclick: move |_| { method.set(InputMethod::StraightKey); buffer.set(String::new()); },
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

/// The mnemonic hint for the current letter: a picture whose name starts with
/// the letter, and — once the learner has struggled — the Morse pattern too.
#[component]
fn Hint(letter: char, level: morse_core::HintLevel) -> Element {
    use morse_core::HintLevel;
    if level == HintLevel::None {
        return rsx! { div { class: "hint hidden" } };
    }
    let pattern = morse_core::to_morse(letter).unwrap_or("");
    rsx! {
        div { class: "hint",
            img { class: "hint-img", src: hint_image(letter), alt: "{letter}" }
            if level == HintLevel::Pattern {
                div { class: "hint-pattern", "{letter} = {pattern}" }
            }
        }
    }
}

/// Map a letter to its mnemonic image asset (same pictures as the original
/// game: a → Archery, b → Banjo, …).
fn hint_image(letter: char) -> Asset {
    match letter.to_ascii_lowercase() {
        'a' => asset!("/assets/png/Archery.png"),
        'b' => asset!("/assets/png/Banjo.png"),
        'c' => asset!("/assets/png/Candy.png"),
        'd' => asset!("/assets/png/Dog.png"),
        'e' => asset!("/assets/png/Eye.png"),
        'f' => asset!("/assets/png/Firetruck.png"),
        'g' => asset!("/assets/png/Giraffe.png"),
        'h' => asset!("/assets/png/Hippo.png"),
        'i' => asset!("/assets/png/Insect.png"),
        'j' => asset!("/assets/png/Jet.png"),
        'k' => asset!("/assets/png/Kite.png"),
        'l' => asset!("/assets/png/Laboratory.png"),
        'm' => asset!("/assets/png/Mustache.png"),
        'n' => asset!("/assets/png/Net.png"),
        'o' => asset!("/assets/png/Orchestra.png"),
        'p' => asset!("/assets/png/Paddle.png"),
        'q' => asset!("/assets/png/Quarterback.png"),
        'r' => asset!("/assets/png/Robot.png"),
        's' => asset!("/assets/png/Submarine.png"),
        't' => asset!("/assets/png/Tape.png"),
        'u' => asset!("/assets/png/Unicorn.png"),
        'v' => asset!("/assets/png/Vacuum.png"),
        'w' => asset!("/assets/png/Wand.png"),
        'x' => asset!("/assets/png/X-ray.png"),
        'y' => asset!("/assets/png/Yard.png"),
        _ => asset!("/assets/png/Zebra.png"),
    }
}
