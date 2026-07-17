// Platform glue: random seed, persistence, a clock, and audio playback.
//
// These are the only things the app needs from the outside world, and they
// differ per platform. On the web we use the browser clock, localStorage, and
// the Web Audio API; on native targets (desktop/mobile dev) they degrade to
// no-ops or the system clock. Everything else in the app is platform-agnostic.

use morse_core::LEARNING_ORDER;

const STORAGE_KEY: &str = "morse_scores";
/// Sidetone frequency in Hz — a comfortable CW pitch.
const TONE_HZ: f32 = 600.0;

// ---- random seed --------------------------------------------------------

#[cfg(target_arch = "wasm32")]
pub fn seed() -> u64 {
    (js_sys::Date::now() as u64) ^ 0x9E37_79B9_7F4A_7C15
}

#[cfg(not(target_arch = "wasm32"))]
pub fn seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

// ---- a millisecond clock (for straight-key press timing) ----------------

#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> f64 {
    0.0
}

// ---- progress persistence -----------------------------------------------

#[cfg(target_arch = "wasm32")]
pub fn load_scores() -> [i32; 26] {
    let mut scores = [0; 26];
    if let Some(raw) = local_storage().and_then(|s| s.get_item(STORAGE_KEY).ok().flatten()) {
        for (slot, part) in scores.iter_mut().zip(raw.split(',')) {
            if let Ok(v) = part.trim().parse::<i32>() {
                *slot = v;
            }
        }
    }
    scores
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_scores() -> [i32; 26] {
    [0; 26]
}

#[cfg(target_arch = "wasm32")]
pub fn save_scores(scores: &[i32; 26]) {
    let encoded = scores
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(STORAGE_KEY, &encoded);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_scores(_scores: &[i32; 26]) {}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

// ---- audio: play a Morse code as a sidetone -----------------------------

// Keep the AudioContext alive until playback finishes. Starting a new tone
// replaces (and drops) the previous context, which stops any earlier tone.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static AUDIO_CTX: std::cell::RefCell<Option<web_sys::AudioContext>> =
        const { std::cell::RefCell::new(None) };
}

/// Play a single character's Morse code with the given timing.
#[cfg(target_arch = "wasm32")]
pub fn play_code(code: &str, timing: morse_core::Timing) {
    use morse_core::schedule_code;

    let tones = schedule_code(code, &timing);
    if tones.is_empty() {
        return;
    }

    let Ok(ctx) = web_sys::AudioContext::new() else {
        return;
    };
    let _ = ctx.resume(); // browsers may start the context suspended

    let start = ctx.current_time();
    let osc = ctx.create_oscillator().expect("oscillator");
    osc.set_type(web_sys::OscillatorType::Sine);
    osc.frequency().set_value(TONE_HZ);

    let gain = ctx.create_gain().expect("gain");
    gain.gain().set_value(0.0);
    let _ = osc.connect_with_audio_node(&gain);
    let _ = gain.connect_with_audio_node(&ctx.destination());

    // 5ms attack/release keeps the keying click-free.
    let edge = 0.005;
    let param = gain.gain();
    for tone in &tones {
        let on = start + tone.start_ms as f64 / 1000.0;
        let off = on + tone.len_ms as f64 / 1000.0;
        let _ = param.set_value_at_time(0.0, (on - edge).max(start));
        let _ = param.linear_ramp_to_value_at_time(0.6, on + edge);
        let _ = param.set_value_at_time(0.6, (off - edge).max(on + edge));
        let _ = param.linear_ramp_to_value_at_time(0.0, off);
    }

    let total = tones.last().map(|t| t.start_ms + t.len_ms).unwrap_or(0);
    let _ = osc.start();
    let _ = osc.stop_with_when(start + total as f64 / 1000.0 + 0.05);

    AUDIO_CTX.with(|slot| *slot.borrow_mut() = Some(ctx));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn play_code(_code: &str, _timing: morse_core::Timing) {}

/// Keep the import referenced on every platform so the module always compiles.
#[allow(dead_code)]
const _LETTER_COUNT_CHECK: usize = LEARNING_ORDER.len();
