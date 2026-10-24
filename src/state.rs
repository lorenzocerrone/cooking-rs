//! Shareable state: the meal plan is serialized to JSON, base64-encoded
//! (URL-safe, no padding) and stored in the URL hash fragment (`#plan=...`).
//!
//! The payload is untagged JSON and every change so far has been additive, so
//! all links ever shared keep decoding. If a breaking change is ever needed,
//! introduce a `#plan=v2:...` prefix and keep this decoder for the old form.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use leptos::prelude::window;

use crate::models::MealPlan;

const HASH_PREFIX: &str = "plan=";

/// Encode a meal plan into a compact, URL-safe string.
pub fn encode(plan: &MealPlan) -> Option<String> {
    let json = serde_json::to_vec(plan).ok()?;
    Some(URL_SAFE_NO_PAD.encode(json))
}

/// Decode a meal plan from the string produced by [`encode`]. The result is
/// normalized to exactly seven days, whatever the link contained.
pub fn decode(encoded: &str) -> Option<MealPlan> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    let mut plan: MealPlan = serde_json::from_slice(&bytes).ok()?;
    plan.normalize();
    Some(plan)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Slot, DAYS_PER_WEEK};

    #[test]
    fn encode_decode_round_trip() {
        let mut plan = MealPlan::default();
        plan.set_recipe(3, Slot::Dinner, Some("carbonara".into()));
        plan.set_servings(3, Slot::Dinner, Some(4));
        let encoded = encode(&plan).unwrap();
        assert!(!encoded.contains('=') && !encoded.contains('+') && !encoded.contains('/'));
        assert_eq!(decode(&encoded), Some(plan));
    }

    #[test]
    fn decode_normalizes_short_plans_and_rejects_garbage() {
        let short = URL_SAFE_NO_PAD.encode(br#"{"days":[{"lunch":"dal"}]}"#);
        let plan = decode(&short).unwrap();
        assert_eq!(plan.days.len(), DAYS_PER_WEEK);
        assert_eq!(plan.recipe_id(0, Slot::Lunch), Some("dal".into()));

        assert_eq!(decode("not base64!"), None);
        assert_eq!(decode(&URL_SAFE_NO_PAD.encode(b"{nope")), None);
    }
}
