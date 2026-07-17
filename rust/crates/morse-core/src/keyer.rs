// Keying: turning key presses into Morse symbols.
//
// The app supports two input styles, and both funnel through here so the rest
// of the code only ever sees a Morse string like "-.-":
//
//   * Two-key / paddle: one key means dit, the other means dah. No timing
//     needed — the symbol is decided by *which* key. Use [`Keyer::push`].
//
//   * Straight key: a single key, where the *duration* of the press decides
//     dit vs dah and the *duration* of the gap decides whether the letter is
//     finished — exactly like a real Morse key. Feed press/gap durations to
//     [`Keyer::press`] / [`Keyer::gap`] and let [`Timing`] classify them.
//
// All classification thresholds come from the current [`Timing`], so the feel
// tracks the selected WPM automatically.

use crate::timing::Timing;

/// A single Morse element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    Dit,
    Dah,
}

impl Symbol {
    /// The character used in Morse strings (`.` or `-`).
    pub fn as_char(self) -> char {
        match self {
            Symbol::Dit => '.',
            Symbol::Dah => '-',
        }
    }
}

/// What a silence between presses means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gap {
    /// Short silence: still inside the same character.
    Symbol,
    /// Medium silence: the character is complete.
    Character,
    /// Long silence: a word break.
    Word,
}

impl Timing {
    /// Classify a straight-key press by its duration. The dit/dah boundary sits
    /// at two dits (halfway between a 1-unit dit and a 3-unit dah).
    pub fn classify_press(&self, duration_ms: u32) -> Symbol {
        if duration_ms < 2 * self.dit_ms() {
            Symbol::Dit
        } else {
            Symbol::Dah
        }
    }

    /// Classify a silence by its duration into symbol / character / word gaps.
    /// Boundaries: below 2 units keeps the character going, below 5 units ends
    /// the character, longer is a word break.
    pub fn classify_gap(&self, duration_ms: u32) -> Gap {
        let unit = self.dit_ms();
        if duration_ms < 2 * unit {
            Gap::Symbol
        } else if duration_ms < 5 * unit {
            Gap::Character
        } else {
            Gap::Word
        }
    }
}

/// Accumulates symbols into the Morse code for the character being entered.
#[derive(Debug, Default, Clone)]
pub struct Keyer {
    current: String,
}

impl Keyer {
    /// A fresh keyer with an empty buffer.
    pub fn new() -> Self {
        Keyer::default()
    }

    /// The Morse entered so far for the in-progress character (e.g. `"-."`).
    pub fn current(&self) -> &str {
        &self.current
    }

    /// True when nothing has been entered for the current character yet.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// Append a symbol directly. Use this for two-key/paddle input.
    pub fn push(&mut self, symbol: Symbol) {
        self.current.push(symbol.as_char());
    }

    /// Append a symbol decided by a straight-key press duration.
    pub fn press(&mut self, duration_ms: u32, timing: &Timing) {
        self.push(timing.classify_press(duration_ms));
    }

    /// Report a silence of `duration_ms`. Returns `Some(code)` (and clears the
    /// buffer) when the gap is long enough to end the character; returns `None`
    /// while the character is still in progress.
    pub fn gap(&mut self, duration_ms: u32, timing: &Timing) -> Option<String> {
        match timing.classify_gap(duration_ms) {
            Gap::Symbol => None,
            Gap::Character | Gap::Word => self.take(),
        }
    }

    /// Take the current character's code and clear the buffer. Returns `None`
    /// if nothing has been entered. Use this to commit on an explicit button.
    pub fn take(&mut self) -> Option<String> {
        if self.current.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.current))
        }
    }

    /// Discard whatever is in the buffer.
    pub fn clear(&mut self) {
        self.current.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_duration_picks_dit_or_dah() {
        let t = Timing::new(20); // dit 60ms, boundary at 120ms
        assert_eq!(t.classify_press(50), Symbol::Dit);
        assert_eq!(t.classify_press(119), Symbol::Dit);
        assert_eq!(t.classify_press(120), Symbol::Dah);
        assert_eq!(t.classify_press(300), Symbol::Dah);
    }

    #[test]
    fn gap_duration_picks_boundary() {
        let t = Timing::new(20); // dit 60ms
        assert_eq!(t.classify_gap(60), Gap::Symbol); // < 120
        assert_eq!(t.classify_gap(180), Gap::Character); // 120..300
        assert_eq!(t.classify_gap(400), Gap::Word); // >= 300
    }

    #[test]
    fn straight_key_spells_a_letter() {
        let t = Timing::new(20);
        let mut k = Keyer::new();
        k.press(50, &t); // dit
        assert_eq!(k.gap(60, &t), None); // intra-character silence
        k.press(200, &t); // dah
        // Long silence ends the character: "-." reversed? no -> ".-" = 'a'
        assert_eq!(k.gap(200, &t), Some(".-".to_string()));
        assert!(k.is_empty());
    }

    #[test]
    fn two_key_input_needs_no_timing() {
        let mut k = Keyer::new();
        k.push(Symbol::Dah);
        k.push(Symbol::Dit);
        k.push(Symbol::Dah);
        k.push(Symbol::Dit);
        assert_eq!(k.current(), "-.-.");
        assert_eq!(k.take(), Some("-.-.".to_string())); // 'c'
        assert_eq!(k.take(), None);
    }
}
