// Morse trainer — Dioxus UI.
//
// A clean rewrite of the original Phaser game as a Dioxus app. The same binary
// targets the web (WASM), Android, iOS, and desktop; all learning logic lives
// in the platform-agnostic `morse-core` crate, so this file is purely
// presentation + input plumbing.

use dioxus::prelude::*;
use morse_core::{HintLevel, Judgement, Trainer};

mod platform;

const STYLE: Asset = asset!("/assets/style.css");

fn main() {
    // `dioxus::launch` picks the renderer from the build target / enabled
    // feature (web, desktop, or mobile). Run with the Dioxus CLI, e.g.
    // `dx serve --platform web`.
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // The whole game state is a single signal holding the core trainer.
    let mut trainer = use_signal(|| Trainer::restore(platform::load_scores(), platform::seed()));
    // Morse the user has tapped for the current letter but not yet committed.
    let mut buffer = use_signal(String::new);
    // The last judgement, used to flash the word green/red briefly.
    let mut flash = use_signal(|| Option::<Judgement>::None);

    // Commit the buffered Morse as an answer for the current letter.
    let mut commit = move || {
        let code = buffer();
        if code.is_empty() {
            return;
        }
        let judgement = trainer.write().submit_morse(&code);
        buffer.set(String::new());
        flash.set(Some(judgement));
        platform::save_scores(&trainer.read().scores());
    };

    let word = trainer.read().current_word().to_string();
    let active = trainer.read().letter_index();
    let hint = trainer.read().hint_level();
    let target = trainer.read().target_letter();
    let completion = (trainer.read().completion() * 100.0).round() as u32;
    let flash_class = match flash() {
        Some(Judgement::Correct) => "flash-correct",
        Some(Judgement::Incorrect) => "flash-incorrect",
        None => "",
    };

    // Precompute each letter's CSS class so the `rsx!` block stays free of
    // inline `if/else` chains (which the macro can't type-infer).
    let letters: Vec<(usize, char, &'static str)> = word
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let class = if i == active {
                "letter active"
            } else if i < active {
                "letter done"
            } else {
                "letter"
            };
            (i, ch, class)
        })
        .collect();

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }
        document::Title { "Morse Learn" }

        main { class: "app",
            // ---- header: overall progress + per-letter lights ----
            header { class: "top",
                div { class: "bar", div { class: "bar-fill", style: "width: {completion}%;" } }
                span { class: "pct", "{completion}% learned" }
            }
            ProgressLights { trainer }

            // ---- the word being practised ----
            section { class: "word {flash_class}",
                for (i, ch, class) in letters.iter().copied() {
                    span { key: "{i}", class, "{ch}" }
                }
            }

            // ---- hint for the current (unlearned) letter ----
            Hint { letter: target, level: hint }

            // ---- what the user has tapped so far ----
            div { class: "buffer",
                if buffer().is_empty() {
                    span { class: "buffer-empty", "tap dot / dash" }
                } else {
                    "{buffer()}"
                }
            }

            // ---- input keys ----
            div { class: "keys",
                button {
                    class: "key dot",
                    onclick: move |_| {
                        flash.set(None);
                        buffer.with_mut(|b| b.push('.'));
                    },
                    "•"
                }
                button {
                    class: "key dash",
                    onclick: move |_| {
                        flash.set(None);
                        buffer.with_mut(|b| b.push('-'));
                    },
                    "—"
                }
            }
            div { class: "keys secondary",
                button {
                    class: "key clear",
                    onclick: move |_| buffer.set(String::new()),
                    "clear"
                }
                button {
                    class: "key enter",
                    onclick: move |_| commit(),
                    "enter letter"
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
/// the letter, and — once the user has struggled — the Morse pattern too.
#[component]
fn Hint(letter: char, level: HintLevel) -> Element {
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
