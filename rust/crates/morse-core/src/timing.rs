// Morse timing.
//
// Everything about *speed* lives here: how long a dit/dah lasts and how long
// the gaps are, all derived from a words-per-minute (WPM) setting. This is the
// basis for both audio playback and for decoding a straight-key press by its
// duration, so it's pure, integer-millisecond logic shared by every platform.
//
// Standard Morse uses the word PARIS (50 dit-units) to define WPM:
//   dit = 1 unit, dah = 3 units, intra-character gap = 1 unit,
//   inter-character gap = 3 units, inter-word gap = 7 units.
// so one dit at W wpm is 1200 / W milliseconds.
//
// For *learning*, Farnsworth timing sends each character at a fast "character
// speed" but stretches the gaps between characters/words so the overall
// "effective speed" is slower. Learners hear the real rhythm of each letter
// from the start while still having thinking time — then close the gap up to
// full speed (here, a 40 WPM ceiling).

/// Slowest speed the UI should allow.
pub const MIN_WPM: u32 = 5;
/// Fastest speed the UI should allow (the learner's target ceiling).
pub const MAX_WPM: u32 = 40;

/// A resolved timing: the character speed plus the (equal or slower) effective
/// speed used for gap padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    char_wpm: u32,
    effective_wpm: u32,
}

impl Timing {
    /// Standard timing at `wpm` (character speed == effective speed). `wpm` is
    /// clamped to `[MIN_WPM, MAX_WPM]`.
    pub fn new(wpm: u32) -> Self {
        let w = wpm.clamp(MIN_WPM, MAX_WPM);
        Timing {
            char_wpm: w,
            effective_wpm: w,
        }
    }

    /// Farnsworth timing: characters sent at `char_wpm`, gaps stretched so the
    /// overall pace feels like `effective_wpm`. If `effective_wpm >= char_wpm`
    /// this is just standard timing.
    pub fn farnsworth(char_wpm: u32, effective_wpm: u32) -> Self {
        let c = char_wpm.clamp(MIN_WPM, MAX_WPM);
        let e = effective_wpm.clamp(MIN_WPM, c);
        Timing {
            char_wpm: c,
            effective_wpm: e,
        }
    }

    /// The character speed in WPM.
    pub fn char_wpm(&self) -> u32 {
        self.char_wpm
    }

    /// The effective (overall) speed in WPM.
    pub fn effective_wpm(&self) -> u32 {
        self.effective_wpm
    }

    /// Duration of one dit (dot), in milliseconds. The base unit.
    pub fn dit_ms(&self) -> u32 {
        1200 / self.char_wpm
    }

    /// Duration of one dah (dash) — three dits.
    pub fn dah_ms(&self) -> u32 {
        3 * self.dit_ms()
    }

    /// Gap between symbols within a character — one dit. Always at character
    /// speed (Farnsworth never stretches intra-character spacing).
    pub fn symbol_gap_ms(&self) -> u32 {
        self.dit_ms()
    }

    /// Gap between characters — 3 dits at full speed, stretched under Farnsworth.
    pub fn char_gap_ms(&self) -> u32 {
        (3.0 * self.farnsworth_unit_ms()).round() as u32
    }

    /// Gap between words — 7 dits at full speed, stretched under Farnsworth.
    pub fn word_gap_ms(&self) -> u32 {
        (7.0 * self.farnsworth_unit_ms()).round() as u32
    }

    /// The length of one *spacing* unit in ms. Equal to a dit at standard speed;
    /// larger when Farnsworth padding is active.
    fn farnsworth_unit_ms(&self) -> f64 {
        let c = self.char_wpm as f64;
        let e = self.effective_wpm as f64;
        // Time (seconds) the 31 character-units of PARIS occupy at char speed.
        let char_time = 31.0 * 1.2 / c;
        // Total time (seconds) one PARIS word may take at effective speed.
        let total_time = 60.0 / e;
        // Remaining time is shared over PARIS's 19 spacing units.
        let spacing = total_time - char_time;
        let unit_s = if spacing > 0.0 {
            spacing / 19.0
        } else {
            1.2 / c // effective >= char: fall back to standard spacing
        };
        unit_s * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dit_length_matches_the_paris_standard() {
        assert_eq!(Timing::new(20).dit_ms(), 60); // 1200/20
        assert_eq!(Timing::new(40).dit_ms(), 30); // ceiling
        assert_eq!(Timing::new(5).dit_ms(), 240);
    }

    #[test]
    fn wpm_is_clamped_to_the_supported_range() {
        assert_eq!(Timing::new(1000).char_wpm(), MAX_WPM);
        assert_eq!(Timing::new(0).char_wpm(), MIN_WPM);
    }

    #[test]
    fn standard_timing_has_the_classic_ratios() {
        let t = Timing::new(20);
        assert_eq!(t.dah_ms(), 3 * t.dit_ms());
        assert_eq!(t.symbol_gap_ms(), t.dit_ms());
        assert_eq!(t.char_gap_ms(), 3 * t.dit_ms());
        assert_eq!(t.word_gap_ms(), 7 * t.dit_ms());
    }

    #[test]
    fn farnsworth_stretches_gaps_but_not_symbols() {
        // Characters at 20 wpm, overall pace only 8 wpm.
        let t = Timing::farnsworth(20, 8);
        let std = Timing::new(20);
        // A dit is unchanged (still character speed)...
        assert_eq!(t.dit_ms(), std.dit_ms());
        assert_eq!(t.symbol_gap_ms(), std.symbol_gap_ms());
        // ...but the gaps between characters and words are noticeably longer.
        assert!(t.char_gap_ms() > std.char_gap_ms());
        assert!(t.word_gap_ms() > std.word_gap_ms());
    }

    #[test]
    fn farnsworth_collapses_to_standard_when_effective_equals_char() {
        let f = Timing::farnsworth(18, 18);
        let s = Timing::new(18);
        assert_eq!(f.char_gap_ms(), s.char_gap_ms());
        assert_eq!(f.word_gap_ms(), s.word_gap_ms());
    }
}
