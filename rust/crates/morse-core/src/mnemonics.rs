// Sound-alike rhythm mnemonics.
//
// Morse is a *sound* skill: you learn each letter as a rhythm, not as a picture
// of dots. The strongest memory hook for that is a word whose spoken cadence is
// the letter's rhythm — a short (unstressed) syllable for a dit, a long
// (stressed) syllable for a dah. Said out loud, "CO-ca-CO-la" *is* the rhythm of
// C (-.-.), and "KANG-a-ROO" is K (-.-).
//
// Each mnemonic's stress pattern is guaranteed by a unit test to match the
// letter's Morse code exactly, so the hook can never teach the wrong rhythm.
// These are a training aid for the early recognition phase; the audio drills are
// where the sound reflex is actually built.

/// One syllable of a mnemonic. `dah` marks a stressed/long syllable (a dash);
/// otherwise it's a short syllable (a dot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syllable {
    pub text: &'static str,
    pub dah: bool,
}

/// A rhythm mnemonic for one letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mnemonic {
    pub letter: char,
    /// Optional aside shown with the mnemonic (e.g. a familiar tune).
    pub note: Option<&'static str>,
    pub syllables: &'static [Syllable],
}

const fn dit(text: &'static str) -> Syllable {
    Syllable { text, dah: false }
}
const fn dah(text: &'static str) -> Syllable {
    Syllable { text, dah: true }
}

// Named consts so the slices have a `'static` lifetime (array literals in a
// `match` arm aren't promoted to `'static`).
const A: &[Syllable] = &[dit("a"), dah("BOUT")];
const B: &[Syllable] = &[dah("BOIS"), dit("ter"), dit("ous"), dit("ly")];
const C: &[Syllable] = &[dah("CO"), dit("ca"), dah("CO"), dit("la")];
const D: &[Syllable] = &[dah("DAN"), dit("ger"), dit("ous")];
const E: &[Syllable] = &[dit("Eh")];
const F: &[Syllable] = &[dit("fi"), dit("de"), dah("LI"), dit("ty")];
const G: &[Syllable] = &[dah("GOOD"), dah("GOL"), dit("ly")];
const H: &[Syllable] = &[dit("hip"), dit("pi"), dit("ty"), dit("hop")];
const I: &[Syllable] = &[dit("i"), dit("bis")];
const J: &[Syllable] = &[dit("je"), dah("LLY"), dah("BEANS"), dah("GONE")];
const K: &[Syllable] = &[dah("KANG"), dit("a"), dah("ROO")];
const L: &[Syllable] = &[dit("le"), dah("MON"), dit("a"), dit("de")];
const M: &[Syllable] = &[dah("MA"), dah("MA")];
const N: &[Syllable] = &[dah("NO"), dit("va")];
const O: &[Syllable] = &[dah("OH"), dah("SO"), dah("SLOW")];
const P: &[Syllable] = &[dit("pop"), dah("CORN"), dah("POPS"), dit("up")];
const Q: &[Syllable] = &[dah("GOD"), dah("SAVE"), dit("the"), dah("QUEEN")];
const R: &[Syllable] = &[dit("ro"), dah("TA"), dit("tion")];
const S: &[Syllable] = &[dit("sau"), dit("sa"), dit("ges")];
const T: &[Syllable] = &[dah("Tall")];
const U: &[Syllable] = &[dit("u"), dit("ni"), dah("FORM")];
const V: &[Syllable] = &[dit("ta"), dit("ta"), dit("ta"), dah("DAAH")];
const W: &[Syllable] = &[dit("we"), dah("TWO"), dah("WON")];
const X: &[Syllable] = &[dah("EX"), dit("er"), dit("cise"), dah("MORE")];
const Y: &[Syllable] = &[dah("YO"), dit("ho"), dah("HO"), dah("HO")];
const Z: &[Syllable] = &[dah("ZE"), dah("BRA"), dit("cross"), dit("ing")];

/// The rhythm mnemonic for a letter, or `None` for non-letters.
pub fn mnemonic(letter: char) -> Option<Mnemonic> {
    let (note, syllables): (Option<&'static str>, &'static [Syllable]) =
        match letter.to_ascii_lowercase() {
            'a' => (None, A),
            'b' => (None, B),
            'c' => (None, C),
            'd' => (None, D),
            'e' => (None, E),
            'f' => (None, F),
            'g' => (None, G),
            'h' => (None, H),
            'i' => (None, I),
            'j' => (None, J),
            'k' => (None, K),
            'l' => (None, L),
            'm' => (None, M),
            'n' => (None, N),
            'o' => (None, O),
            'p' => (None, P),
            'q' => (None, Q),
            'r' => (None, R),
            's' => (None, S),
            't' => (None, T),
            'u' => (None, U),
            'v' => (Some("Beethoven's 5th"), V),
            'w' => (None, W),
            'x' => (None, X),
            'y' => (None, Y),
            'z' => (None, Z),
            _ => return None,
        };
    Some(Mnemonic {
        letter: letter.to_ascii_lowercase(),
        note,
        syllables,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alphabet::to_morse;

    #[test]
    fn every_mnemonic_rhythm_matches_the_letters_code() {
        for letter in 'a'..='z' {
            let m = mnemonic(letter).expect("a mnemonic for every letter");
            let rhythm: String = m
                .syllables
                .iter()
                .map(|s| if s.dah { '-' } else { '.' })
                .collect();
            assert_eq!(
                rhythm,
                to_morse(letter).unwrap(),
                "mnemonic for '{letter}' has the wrong rhythm"
            );
        }
    }

    #[test]
    fn non_letters_have_no_mnemonic() {
        assert!(mnemonic('5').is_none());
        assert!(mnemonic(' ').is_none());
    }
}
