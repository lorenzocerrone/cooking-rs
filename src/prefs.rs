//! Local, non-shared preferences. Unlike the meal plan (which is encoded in the
//! URL hash so links can be shared — see [`crate::state`]), these are personal
//! settings that live only in `localStorage` on this device.

use leptos::prelude::window;

/// `localStorage` key for the daily calorie goal (kcal).
const DAILY_GOAL_KEY: &str = "daily_goal_kcal";

/// The browser's `localStorage`, if available (may be `None` in private mode or
/// when storage is disabled).
fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// Read the saved daily calorie goal, if one has been set.
pub fn get_daily_goal() -> Option<u32> {
    storage()?
        .get_item(DAILY_GOAL_KEY)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
}

/// Save (or clear, with `None`) the daily calorie goal.
pub fn set_daily_goal(goal: Option<u32>) {
    if let Some(store) = storage() {
        let _ = match goal {
            Some(kcal) => store.set_item(DAILY_GOAL_KEY, &kcal.to_string()),
            None => store.remove_item(DAILY_GOAL_KEY),
        };
    }
}
