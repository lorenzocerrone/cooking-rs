//! The URL is the plan: the meal plan is serialized to JSON, base64-encoded
//! (URL-safe, no padding) and kept in the address bar's hash fragment
//! (`#plan=...`). The app reads it on startup (see `storage::resolve_initial`)
//! and rewrites it after every change and navigation (see [`write_hash`]), so
//! bookmarking or sending the current address shares the plan; the planner's
//! Share button copies the same link.
//!
//! The payload is untagged JSON decoded through [`PlanWire`], so all links ever
//! shared keep decoding: weekday links (from before plans were dated) land on
//! the coming dates. If a change ever can't be told apart by shape, introduce a
//! `#plan=v2:...` prefix and keep this decoder for the old form.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use leptos::prelude::window;

use crate::date::Date;
use crate::models::{MealPlan, PlanWire};

const HASH_PREFIX: &str = "plan=";

/// Encode a meal plan into a compact, URL-safe string.
pub fn encode(plan: &MealPlan) -> Option<String> {
    let json = serde_json::to_vec(plan).ok()?;
    Some(URL_SAFE_NO_PAD.encode(json))
}

/// Decode a meal plan from the string produced by [`encode`] (or any older
/// link), as of `today`: past days are dropped (see [`PlanWire::into_plan`]).
pub fn decode(encoded: &str, today: Date) -> Option<MealPlan> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    let wire: PlanWire = serde_json::from_slice(&bytes).ok()?;
    Some(wire.into_plan(today))
}

/// Read the meal plan currently encoded in the URL hash, if any.
pub fn read_hash(today: Date) -> Option<MealPlan> {
    let hash = window().location().hash().ok()?;
    // `hash` includes the leading '#'.
    let raw = hash.strip_prefix('#').unwrap_or(&hash);
    let encoded = raw.strip_prefix(HASH_PREFIX)?;
    decode(encoded, today)
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

/// The address for `path` + `search` carrying `plan` in the fragment, or no
/// fragment for an empty plan.
pub fn hash_url(path: &str, search: &str, plan: &MealPlan) -> Option<String> {
    if plan.is_empty() {
        return Some(format!("{path}{search}"));
    }
    Some(format!("{path}{search}#{HASH_PREFIX}{}", encode(plan)?))
}

/// Put `plan` in the address bar without reloading or adding a history entry.
/// Does nothing when the address already matches.
pub fn write_hash(plan: &MealPlan) {
    let location = window().location();
    let (Ok(path), Ok(search), Ok(hash)) =
        (location.pathname(), location.search(), location.hash())
    else {
        return;
    };
    let Some(url) = hash_url(&path, &search, plan) else {
        return;
    };
    if url == format!("{path}{search}{hash}") {
        return;
    }
    if let Ok(history) = window().history() {
        // Keep the router's history state so back/forward keep working.
        let state = history
            .state()
            .unwrap_or(leptos::wasm_bindgen::JsValue::NULL);
        let _ = history.replace_state_with_url(&state, "", Some(&url));
    }
}

/// The address bar's query string ("?tag=quick", or "").
pub fn read_search() -> String {
    window().location().search().unwrap_or_default()
}

/// Replace the address bar's query string (keeping the path and the plan
/// fragment) without reloading or adding a history entry.
pub fn write_search(search: &str) {
    let location = window().location();
    let (Ok(path), Ok(current), Ok(hash)) =
        (location.pathname(), location.search(), location.hash())
    else {
        return;
    };
    if current == search {
        return;
    }
    if let Ok(history) = window().history() {
        let state = history
            .state()
            .unwrap_or(leptos::wasm_bindgen::JsValue::NULL);
        let _ = history.replace_state_with_url(&state, "", Some(&format!("{path}{search}{hash}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Sunday.
    fn today() -> Date {
        Date::from_ymd(2026, 10, 25).unwrap()
    }

    #[test]
    fn encode_decode_round_trip() {
        let mut plan = MealPlan::default();
        let day = today().add_days(3);
        plan.add_meal(day, "carbonara".into());
        plan.set_servings(day, 0, Some(4));
        let encoded = encode(&plan).unwrap();
        assert!(!encoded.contains('=') && !encoded.contains('+') && !encoded.contains('/'));
        assert_eq!(decode(&encoded, today()), Some(plan));
    }

    #[test]
    fn old_weekday_links_decode_onto_dates_and_garbage_is_rejected() {
        let old = URL_SAFE_NO_PAD.encode(br#"{"days":[{"lunch":"dal"}]}"#);
        let plan = decode(&old, today()).unwrap();
        assert_eq!(plan.meal(today().add_days(1), 0).unwrap().recipe, "dal");
        assert_eq!(plan.meal_count(), 1);

        assert_eq!(decode("not base64!", today()), None);
        assert_eq!(decode(&URL_SAFE_NO_PAD.encode(b"{nope"), today()), None);
    }

    #[test]
    fn a_link_opened_later_drops_the_days_that_passed() {
        let mut plan = MealPlan::default();
        plan.add_meal(today(), "soup".into());
        plan.add_meal(today().add_days(2), "dal".into());
        let encoded = encode(&plan).unwrap();
        let later = decode(&encoded, today().add_days(1)).unwrap();
        assert_eq!(later.meal_count(), 1);
        assert_eq!(later.meal(today().add_days(2), 0).unwrap().recipe, "dal");
    }

    #[test]
    fn hash_url_keeps_path_and_query_and_drops_the_fragment_for_an_empty_plan() {
        let empty = MealPlan::default();
        assert_eq!(
            hash_url("/cooking-rs/", "?tag=quick", &empty).unwrap(),
            "/cooking-rs/?tag=quick"
        );
        let mut plan = MealPlan::default();
        plan.add_meal(today(), "dal".into());
        let encoded = encode(&plan).unwrap();
        assert_eq!(
            hash_url("/cooking-rs/grocery", "", &plan).unwrap(),
            format!("/cooking-rs/grocery#plan={encoded}")
        );
    }

    #[test]
    fn share_url_replaces_any_existing_fragment() {
        let mut plan = MealPlan::default();
        plan.add_meal(today(), "dal".into());
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
