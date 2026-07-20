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

/// Load a persisted on/off setting, falling back to `default` when unset.
#[cfg(target_arch = "wasm32")]
pub fn load_flag(key: &str, default: bool) -> bool {
    match local_storage().and_then(|s| s.get_item(key).ok().flatten()) {
        Some(v) => v == "1",
        None => default,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_flag(_key: &str, default: bool) -> bool {
    default
}

/// Persist an on/off setting.
#[cfg(target_arch = "wasm32")]
pub fn save_flag(key: &str, value: bool) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, if value { "1" } else { "0" });
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_flag(_key: &str, _value: bool) {}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

// ---- audio: shared context, Morse playback, and a held sidetone ---------

#[cfg(target_arch = "wasm32")]
thread_local! {
    // One AudioContext is reused for the whole app (browsers cap how many can
    // exist); created lazily on first sound and resumed on each use.
    static AUDIO_CTX: std::cell::RefCell<Option<web_sys::AudioContext>> =
        const { std::cell::RefCell::new(None) };
    // The currently-held straight-key tone (gain + oscillator), if any.
    static LIVE_TONE: std::cell::RefCell<Option<(web_sys::GainNode, web_sys::OscillatorNode)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn audio_ctx() -> Option<web_sys::AudioContext> {
    AUDIO_CTX.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = web_sys::AudioContext::new().ok();
        }
        if let Some(ctx) = slot.as_ref() {
            let _ = ctx.resume(); // browsers may start it suspended
        }
        slot.clone()
    })
}

/// Play a single character's Morse code with the given timing.
#[cfg(target_arch = "wasm32")]
pub fn play_code(code: &str, timing: morse_core::Timing) {
    use morse_core::schedule_code;

    let tones = schedule_code(code, &timing);
    if tones.is_empty() {
        return;
    }
    let Some(ctx) = audio_ctx() else {
        return;
    };

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
}

/// Start a continuous sidetone for a held straight-key press. Idempotent — a
/// second call while a tone is already sounding does nothing.
#[cfg(target_arch = "wasm32")]
pub fn tone_on() {
    if LIVE_TONE.with(|t| t.borrow().is_some()) {
        return;
    }
    let Some(ctx) = audio_ctx() else {
        return;
    };
    let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) else {
        return;
    };
    osc.set_type(web_sys::OscillatorType::Sine);
    osc.frequency().set_value(TONE_HZ);
    gain.gain().set_value(0.0);
    let _ = osc.connect_with_audio_node(&gain);
    let _ = gain.connect_with_audio_node(&ctx.destination());

    let now = ctx.current_time();
    let _ = gain.gain().set_value_at_time(0.0, now);
    let _ = gain.gain().linear_ramp_to_value_at_time(0.5, now + 0.006); // click-free attack
    let _ = osc.start();
    LIVE_TONE.with(|t| *t.borrow_mut() = Some((gain, osc)));
}

/// Stop the continuous sidetone (on key release).
#[cfg(target_arch = "wasm32")]
pub fn tone_off() {
    if let Some((gain, osc)) = LIVE_TONE.with(|t| t.borrow_mut().take()) {
        if let Some(ctx) = audio_ctx() {
            let now = ctx.current_time();
            let _ = gain.gain().set_value_at_time(0.5, now);
            let _ = gain.gain().linear_ramp_to_value_at_time(0.0, now + 0.012); // release
            let _ = osc.stop_with_when(now + 0.04);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn play_code(_code: &str, _timing: morse_core::Timing) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn tone_on() {}
#[cfg(not(target_arch = "wasm32"))]
pub fn tone_off() {}

// ---- keyboard: document-level keydown / keyup listeners -----------------

/// Register a global keyboard handler for `event` ("keydown" / "keyup"),
/// receiving each key's name (e.g. ".", "Enter", " ", "a"). If the handler
/// returns `true` the key is consumed and its default action suppressed. On
/// keydown, auto-repeat is ignored (so a held key fires once). The listener
/// lives for the app's lifetime.
#[cfg(target_arch = "wasm32")]
fn register_key(event: &str, ignore_repeat: bool, mut handler: impl FnMut(String) -> bool + 'static) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let closure = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        if ignore_repeat && e.repeat() {
            return;
        }
        if handler(e.key()) {
            e.prevent_default();
        }
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let _ = document.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
pub fn on_keydown(handler: impl FnMut(String) -> bool + 'static) {
    register_key("keydown", true, handler);
}

#[cfg(target_arch = "wasm32")]
pub fn on_keyup(handler: impl FnMut(String) -> bool + 'static) {
    register_key("keyup", false, handler);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn on_keydown(_handler: impl FnMut(String) -> bool + 'static) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn on_keyup(_handler: impl FnMut(String) -> bool + 'static) {}

/// Keep the import referenced on every platform so the module always compiles.
#[allow(dead_code)]
const _LETTER_COUNT_CHECK: usize = LEARNING_ORDER.len();
