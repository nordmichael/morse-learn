// Morse alphabet, ported from the original trainer's morse-dictionary.js.
//
// This is the portable heart of the trainer: a static lookup table plus a
// couple of small conversion helpers. Everything here is `const`/pure, has no
// dependencies, and works the same on web (WASM), Android, iOS, and desktop.

/// Letters in the order the trainer teaches them. This is *not* alphabetical:
/// it roughly follows letter frequency / Morse simplicity so early lessons use
/// short, common codes (`e = .`, `t = -`, `a = .-`, ...).
pub const LEARNING_ORDER: [char; 26] = [
    'e', 't', 'a', 'i', 'm', 's', 'o', 'h', 'n', 'c', 'r', 'd', 'u', 'k', 'l',
    'f', 'b', 'p', 'g', 'j', 'v', 'q', 'w', 'x', 'y', 'z',
];

/// `(letter, morse)` pairs. `.` is a dot (dit), `-` is a dash (dah).
const TABLE: [(char, &str); 26] = [
    ('a', ".-"),
    ('b', "-..."),
    ('c', "-.-."),
    ('d', "-.."),
    ('e', "."),
    ('f', "..-."),
    ('g', "--."),
    ('h', "...."),
    ('i', ".."),
    ('j', ".---"),
    ('k', "-.-"),
    ('l', ".-.."),
    ('m', "--"),
    ('n', "-."),
    ('o', "---"),
    ('p', ".--."),
    ('q', "--.-"),
    ('r', ".-."),
    ('s', "..."),
    ('t', "-"),
    ('u', "..-"),
    ('v', "...-"),
    ('w', ".--"),
    ('x', "-..-"),
    ('y', "-.--"),
    ('z', "--.."),
];

/// The Morse pattern for an ASCII letter, e.g. `to_morse('s') == Some("...")`.
/// Returns `None` for anything that isn't `a`..=`z` (case-insensitive).
pub fn to_morse(letter: char) -> Option<&'static str> {
    let letter = letter.to_ascii_lowercase();
    TABLE
        .iter()
        .find(|(c, _)| *c == letter)
        .map(|(_, code)| *code)
}

/// The letter for a Morse pattern, e.g. `from_morse("...") == Some('s')`.
/// Returns `None` for any sequence that isn't a valid single-letter code.
pub fn from_morse(code: &str) -> Option<char> {
    TABLE
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(letter, _)| *letter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_letter() {
        for letter in 'a'..='z' {
            let code = to_morse(letter).expect("every letter has a code");
            assert_eq!(from_morse(code), Some(letter));
        }
    }

    #[test]
    fn known_codes() {
        assert_eq!(to_morse('e'), Some("."));
        assert_eq!(to_morse('t'), Some("-"));
        assert_eq!(to_morse('S'), Some("...")); // case-insensitive
        assert_eq!(from_morse("-.-.").unwrap(), 'c');
    }

    #[test]
    fn rejects_unknown() {
        assert_eq!(to_morse('5'), None);
        assert_eq!(from_morse(".-.-.-"), None);
    }

    #[test]
    fn learning_order_is_a_permutation_of_the_alphabet() {
        let mut sorted = LEARNING_ORDER;
        sorted.sort_unstable();
        let alphabet: Vec<char> = ('a'..='z').collect();
        assert_eq!(sorted.to_vec(), alphabet);
    }
}
