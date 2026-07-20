// Drill modes and input methods.
//
// Real mastery needs both *directions* of the skill, plus listening. A drill is
// a pairing of how the target is *presented* (prompt) and how the learner
// *responds* (answer):
//
//   prompt:  see the letter | see the code | hear the code
//   answer:  name the letter | send the code
//
// The four drills below cover recognition, production, head-copy, and the
// intermediate "echo what you hear" step. The input method is independent: send
// answers can be produced with a two-key paddle or a single straight key.

/// How the current target is presented to the learner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// Show the letter glyph.
    SeeLetter,
    /// Show the dots/dashes.
    SeeCode,
    /// Play the code as audio (a drill may additionally reveal the letter — see
    /// [`Drill::shows_letter`]).
    HearCode,
}

/// How the learner answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Identify the letter (recognition).
    TypeLetter,
    /// Key the Morse code (production).
    SendCode,
}

/// A complete practice drill. The set is sending-focused, with one receiving
/// (head-copy) drill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drill {
    /// Hear the code *and* see the letter → send it. Full reinforcement for
    /// learning a new letter (ear + eye + hand).
    HearCodeSendCode,
    /// See a letter → send its code. Recall production.
    SeeLetterSendCode,
    /// See the code → send it. Transcribe/echo the written code.
    SeeCodeSendCode,
    /// Hear the code → name the letter. Head-copy / "receiving".
    HearCodeTypeLetter,
}

impl Drill {
    /// All drills, in a sensible learning order.
    pub fn all() -> [Drill; 4] {
        [
            Drill::HearCodeSendCode,
            Drill::SeeLetterSendCode,
            Drill::SeeCodeSendCode,
            Drill::HearCodeTypeLetter,
        ]
    }

    /// How this drill presents the target.
    pub fn prompt(self) -> Prompt {
        match self {
            Drill::SeeLetterSendCode => Prompt::SeeLetter,
            Drill::SeeCodeSendCode => Prompt::SeeCode,
            Drill::HearCodeTypeLetter | Drill::HearCodeSendCode => Prompt::HearCode,
        }
    }

    /// How this drill expects an answer.
    pub fn answer(self) -> Answer {
        match self {
            Drill::SeeLetterSendCode | Drill::SeeCodeSendCode | Drill::HearCodeSendCode => {
                Answer::SendCode
            }
            Drill::HearCodeTypeLetter => Answer::TypeLetter,
        }
    }

    /// Whether this drill plays audio for its prompt.
    pub fn is_audio(self) -> bool {
        self.prompt() == Prompt::HearCode
    }

    /// Whether the letter glyph should be displayed. The see-letter drill shows
    /// it as its prompt; hear→send also reveals it to reinforce the sound→letter
    /// link while learning. The see-code and hear→letter drills hide it (the
    /// learner must supply/recall it).
    pub fn shows_letter(self) -> bool {
        matches!(self, Drill::SeeLetterSendCode | Drill::HearCodeSendCode)
    }

    /// A short human label for a mode picker.
    pub fn label(self) -> &'static str {
        match self {
            Drill::HearCodeSendCode => "Hear → send",
            Drill::SeeLetterSendCode => "See letter → send",
            Drill::SeeCodeSendCode => "See code → send",
            Drill::HearCodeTypeLetter => "Hear → letter",
        }
    }
}

/// How send-style answers are produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethod {
    /// Two keys: one for dit, one for dah (paddle style).
    TwoKey,
    /// One key: press duration decides dit vs dah (straight key).
    StraightKey,
}

impl InputMethod {
    pub fn label(self) -> &'static str {
        match self {
            InputMethod::TwoKey => "Two keys",
            InputMethod::StraightKey => "Straight key",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drills_map_to_expected_prompt_and_answer() {
        assert_eq!(Drill::SeeLetterSendCode.prompt(), Prompt::SeeLetter);
        assert_eq!(Drill::SeeLetterSendCode.answer(), Answer::SendCode);
        assert_eq!(Drill::SeeCodeSendCode.prompt(), Prompt::SeeCode);
        assert_eq!(Drill::SeeCodeSendCode.answer(), Answer::SendCode);
        assert_eq!(Drill::HearCodeTypeLetter.prompt(), Prompt::HearCode);
        assert_eq!(Drill::HearCodeTypeLetter.answer(), Answer::TypeLetter);
        assert!(Drill::HearCodeSendCode.is_audio());
        assert!(!Drill::SeeCodeSendCode.is_audio());
    }

    #[test]
    fn only_learn_and_see_letter_drills_reveal_the_letter() {
        assert!(Drill::HearCodeSendCode.shows_letter()); // learn-new reinforcement
        assert!(Drill::SeeLetterSendCode.shows_letter());
        assert!(!Drill::SeeCodeSendCode.shows_letter());
        assert!(!Drill::HearCodeTypeLetter.shows_letter()); // it's the test
    }

    #[test]
    fn all_four_drills_are_distinct() {
        let all = Drill::all();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
