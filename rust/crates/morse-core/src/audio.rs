// Audio schedule generation.
//
// This turns a Morse string (or a whole word) into a list of tone bursts: when
// each beep starts and how long it lasts, in milliseconds. It contains no audio
// code itself — the platform layer feeds this schedule to Web Audio on the web,
// or to a native tone generator on mobile. Keeping the schedule pure means the
// exact same rhythm plays everywhere and can be unit-tested with no sound card.

use crate::timing::Timing;

/// A single beep: a tone that starts at `start_ms` after playback begins and
/// lasts `len_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tone {
    pub start_ms: u32,
    pub len_ms: u32,
}

/// Build the tone schedule for a single character's Morse code (e.g. `"-.-"`).
/// Unknown characters in `code` are ignored.
pub fn schedule_code(code: &str, timing: &Timing) -> Vec<Tone> {
    let mut tones = Vec::new();
    let mut cursor = 0u32;
    let mut first = true;
    for symbol in code.chars() {
        let len = match symbol {
            '.' => timing.dit_ms(),
            '-' => timing.dah_ms(),
            _ => continue,
        };
        if !first {
            cursor += timing.symbol_gap_ms(); // gap before this symbol
        }
        tones.push(Tone {
            start_ms: cursor,
            len_ms: len,
        });
        cursor += len;
        first = false;
    }
    tones
}

/// Build the tone schedule for a word or phrase, inserting inter-character and
/// inter-word gaps. Spaces separate words; each other letter is looked up in the
/// Morse table (via [`crate::alphabet::to_morse`]).
pub fn schedule_text(text: &str, timing: &Timing) -> Vec<Tone> {
    let mut tones = Vec::new();
    let mut cursor = 0u32;

    for (i, ch) in text.chars().enumerate() {
        if i > 0 {
            cursor += if ch == ' ' {
                timing.word_gap_ms()
            } else if text.chars().nth(i - 1) == Some(' ') {
                0 // the word gap was already added by the space itself
            } else {
                timing.char_gap_ms()
            };
        }
        if ch == ' ' {
            continue;
        }
        let Some(code) = crate::alphabet::to_morse(ch) else {
            continue;
        };
        for tone in schedule_code(code, timing) {
            tones.push(Tone {
                start_ms: cursor + tone.start_ms,
                len_ms: tone.len_ms,
            });
        }
        // Advance the cursor to the end of this character's own tones.
        if let Some(last) = schedule_code(code, timing).last() {
            cursor += last.start_ms + last.len_ms;
        }
    }
    tones
}

/// Total time (ms) a schedule occupies, from start to the end of its last tone.
pub fn total_duration_ms(tones: &[Tone]) -> u32 {
    tones
        .last()
        .map(|t| t.start_ms + t.len_ms)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_dit_is_one_burst() {
        let t = Timing::new(20); // dit = 60ms
        let tones = schedule_code(".", &t);
        assert_eq!(tones, vec![Tone { start_ms: 0, len_ms: 60 }]);
    }

    #[test]
    fn letter_a_is_dit_gap_dah() {
        let t = Timing::new(20); // dit 60, dah 180, gap 60
        let tones = schedule_code(".-", &t);
        assert_eq!(
            tones,
            vec![
                Tone { start_ms: 0, len_ms: 60 },   // dit
                Tone { start_ms: 120, len_ms: 180 } // gap(60) after dit(60) -> dah
            ]
        );
    }

    #[test]
    fn word_inserts_character_gaps() {
        let t = Timing::new(20);
        let tones = schedule_text("ee", &t); // two dits separated by a 3-dit gap
        assert_eq!(tones.len(), 2);
        assert_eq!(tones[0], Tone { start_ms: 0, len_ms: 60 });
        // second 'e' starts after dit(60) + char_gap(180) = 240
        assert_eq!(tones[1].start_ms, 240);
    }

    #[test]
    fn total_duration_reaches_end_of_last_tone() {
        let t = Timing::new(20);
        let tones = schedule_code("-", &t); // one dah of 180ms
        assert_eq!(total_duration_ms(&tones), 180);
    }
}
