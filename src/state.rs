//! Shareable state: the meal plan is serialized to JSON, base64-encoded
//! (URL-safe, no padding) and stored in the URL hash fragment (`#plan=...`).

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use leptos::prelude::window;

use crate::models::MealPlan;

const HASH_PREFIX: &str = "plan=";

/// Encode a meal plan into a compact, URL-safe string.
pub fn encode(plan: &MealPlan) -> Option<String> {
    let json = serde_json::to_vec(plan).ok()?;
    Some(URL_SAFE_NO_PAD.encode(json))
}

/// Decode a meal plan from the string produced by [`encode`].
pub fn decode(encoded: &str) -> Option<MealPlan> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Read the meal plan currently encoded in the URL hash, if any.
pub fn read_hash() -> Option<MealPlan> {
    let hash = window().location().hash().ok()?;
    // `hash` includes the leading '#'.
    let raw = hash.strip_prefix('#').unwrap_or(&hash);
    let encoded = raw.strip_prefix(HASH_PREFIX)?;
    decode(encoded)
}

/// Write the meal plan into the URL hash (replaces the current fragment).
pub fn write_hash(plan: &MealPlan) {
    if let Some(encoded) = encode(plan) {
        let _ = window()
            .location()
            .set_hash(&format!("{HASH_PREFIX}{encoded}"));
    }
}
