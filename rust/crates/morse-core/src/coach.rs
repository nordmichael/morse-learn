// Adaptive speed coaching.
//
// Good Morse practice keeps you at the edge of your ability: speed up when
// you're comfortably accurate, ease off when you start missing. This tracks a
// rolling window of recent answers and, once it has enough samples, advises the
// UI to go faster or slower. It's pure logic (no timers, no I/O), so the policy
// is unit-tested and identical on every platform. How the advice maps onto the
// character/effective WPM knobs is the UI's decision.

use std::collections::VecDeque;

/// What the coach suggests after the latest answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedAdvice {
    /// Stay at the current speed.
    Hold,
    /// Accuracy is high — nudge the speed up.
    Faster,
    /// Accuracy is low — nudge the speed down.
    Slower,
}

/// Rolling-accuracy speed coach.
#[derive(Debug, Clone)]
pub struct SpeedCoach {
    window: VecDeque<bool>,
    capacity: usize,
    raise_at: f32,
    lower_at: f32,
}

impl Default for SpeedCoach {
    fn default() -> Self {
        // Judge over the last 10 answers: speed up at >=90% accurate, slow down
        // at <=50%. These thresholds keep the ramp responsive but not twitchy.
        SpeedCoach::new(10, 0.9, 0.5)
    }
}

impl SpeedCoach {
    /// Create a coach with an explicit window size and accuracy thresholds.
    /// `raise_at`/`lower_at` are fractions in `0.0..=1.0`.
    pub fn new(capacity: usize, raise_at: f32, lower_at: f32) -> Self {
        SpeedCoach {
            window: VecDeque::with_capacity(capacity.max(1)),
            capacity: capacity.max(1),
            raise_at,
            lower_at,
        }
    }

    /// Record an answer and get the resulting advice. Advice other than `Hold`
    /// is only ever returned once the window is full; after a `Faster`/`Slower`
    /// the window is cleared so the next change needs a fresh run of evidence.
    pub fn record(&mut self, correct: bool) -> SpeedAdvice {
        self.window.push_back(correct);
        if self.window.len() > self.capacity {
            self.window.pop_front();
        }
        if self.window.len() < self.capacity {
            return SpeedAdvice::Hold;
        }

        let hits = self.window.iter().filter(|&&c| c).count();
        let accuracy = hits as f32 / self.capacity as f32;
        if accuracy >= self.raise_at {
            self.window.clear();
            SpeedAdvice::Faster
        } else if accuracy <= self.lower_at {
            self.window.clear();
            SpeedAdvice::Slower
        } else {
            SpeedAdvice::Hold
        }
    }

    /// Current accuracy over the window, or `None` until at least one answer.
    pub fn accuracy(&self) -> Option<f32> {
        if self.window.is_empty() {
            None
        } else {
            let hits = self.window.iter().filter(|&&c| c).count();
            Some(hits as f32 / self.window.len() as f32)
        }
    }

    /// Forget the recent history (e.g. when re-enabling auto speed).
    pub fn reset(&mut self) {
        self.window.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_until_the_window_is_full() {
        let mut coach = SpeedCoach::new(5, 0.9, 0.5);
        for _ in 0..4 {
            assert_eq!(coach.record(true), SpeedAdvice::Hold);
        }
        assert_eq!(coach.record(true), SpeedAdvice::Faster); // 5th fills it
    }

    #[test]
    fn sustained_accuracy_speeds_up_then_needs_fresh_evidence() {
        let mut coach = SpeedCoach::new(4, 0.9, 0.5);
        for _ in 0..3 {
            coach.record(true);
        }
        assert_eq!(coach.record(true), SpeedAdvice::Faster);
        // Window cleared: no immediate second bump.
        assert_eq!(coach.record(true), SpeedAdvice::Hold);
    }

    #[test]
    fn poor_accuracy_slows_down() {
        let mut coach = SpeedCoach::new(4, 0.9, 0.5);
        coach.record(false);
        coach.record(false);
        coach.record(true);
        assert_eq!(coach.record(false), SpeedAdvice::Slower); // 1/4 = 25%
    }

    #[test]
    fn middling_accuracy_holds() {
        let mut coach = SpeedCoach::new(4, 0.9, 0.5);
        coach.record(true);
        coach.record(true);
        coach.record(true);
        assert_eq!(coach.record(false), SpeedAdvice::Hold); // 3/4 = 75%
    }

    #[test]
    fn accuracy_reports_the_running_fraction() {
        let mut coach = SpeedCoach::new(10, 0.9, 0.5);
        assert_eq!(coach.accuracy(), None);
        coach.record(true);
        coach.record(false);
        assert_eq!(coach.accuracy(), Some(0.5));
    }
}
