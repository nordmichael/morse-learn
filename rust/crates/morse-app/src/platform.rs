// Platform glue: a random seed and progress persistence.
//
// These are the only two things the trainer needs from the outside world, and
// they differ per platform. On the web we use the browser clock + localStorage;
// on native targets (desktop/mobile dev) we fall back to the system clock and
// keep progress in memory. Everything else in the app is platform-agnostic.

use morse_core::LEARNING_ORDER;

const STORAGE_KEY: &str = "morse_scores";

/// A seed for the word-shuffling RNG. Variety across sessions, nothing more.
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

/// Load saved per-letter scores, indexed like `LEARNING_ORDER`. Returns all
/// zeroes for a first-time player or if nothing is stored.
#[cfg(target_arch = "wasm32")]
pub fn load_scores() -> [i32; 26] {
    let mut scores = [0; 26];
    if let Some(raw) = local_storage().and_then(|s| s.get_item(STORAGE_KEY).ok().flatten()) {
        // Stored as 26 comma-separated integers in learning order.
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

/// Persist per-letter scores. No-op off the web (dev builds keep state in RAM).
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

/// Keep the import referenced on every platform so the module always compiles
/// cleanly regardless of which `cfg` branches are active.
#[allow(dead_code)]
const _LETTER_COUNT_CHECK: usize = LEARNING_ORDER.len();
