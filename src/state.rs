//! Shareable state: the meal plan is serialized to JSON, base64-encoded
//! (URL-safe, no padding) and carried in the URL hash fragment (`#plan=...`) of a
//! share link. The app reads such a link on startup (see `storage::resolve_initial`)
//! and produces one on demand (the planner's Share button).
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

/// The shareable link for a plan: `page_url` (origin + path, no fragment) plus
/// the encoded plan in the fragment.
pub fn share_url(page_url: &str, plan: &MealPlan) -> Option<String> {
    let encoded = encode(plan)?;
    let base = page_url.split('#').next().unwrap_or(page_url);
    Some(format!("{base}#{HASH_PREFIX}{encoded}"))
}

/// The current page's share link (see [`share_url`]).
pub fn current_share_url(plan: &MealPlan) -> Option<String> {
    let href = window().location().href().ok()?;
    share_url(&href, plan)
}

/// Remove the fragment from the address bar without reloading or adding a
/// history entry, once a shared plan has been imported.
pub fn clear_hash() {
    let location = window().location();
    let (Ok(path), Ok(search)) = (location.pathname(), location.search()) else {
        return;
    };
    if let Ok(history) = window().history() {
        let _ = history.replace_state_with_url(
            &leptos::wasm_bindgen::JsValue::NULL,
            "",
            Some(&format!("{path}{search}")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DAYS_PER_WEEK;

    #[test]
    fn encode_decode_round_trip() {
        let mut plan = MealPlan::default();
        plan.add_meal(3, "carbonara".into());
        plan.set_servings(3, 0, Some(4));
        let encoded = encode(&plan).unwrap();
        assert!(!encoded.contains('=') && !encoded.contains('+') && !encoded.contains('/'));
        assert_eq!(decode(&encoded), Some(plan));
    }

    #[test]
    fn decode_normalizes_short_plans_and_rejects_garbage() {
        let short = URL_SAFE_NO_PAD.encode(br#"{"days":[{"lunch":"dal"}]}"#);
        let plan = decode(&short).unwrap();
        assert_eq!(plan.days.len(), DAYS_PER_WEEK);
        assert_eq!(plan.meal(0, 0).unwrap().recipe, "dal");

        assert_eq!(decode("not base64!"), None);
        assert_eq!(decode(&URL_SAFE_NO_PAD.encode(b"{nope")), None);
    }

    #[test]
    fn share_url_replaces_any_existing_fragment() {
        let mut plan = MealPlan::default();
        plan.add_meal(0, "dal".into());
        let encoded = encode(&plan).unwrap();
        assert_eq!(
            share_url("https://ex.com/cooking-rs/planner", &plan).unwrap(),
            format!("https://ex.com/cooking-rs/planner#plan={encoded}")
        );
        assert_eq!(
            share_url("https://ex.com/planner#plan=old", &plan).unwrap(),
            format!("https://ex.com/planner#plan={encoded}")
        );
    }
}
