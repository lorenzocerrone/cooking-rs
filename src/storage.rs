//! Local persistence boundary.
//!
//! The meal plan and the grocery list's check-off state are saved to
//! `localStorage` on every change, so they survive reloads on this device. The
//! URL hash (see [`crate::state`]) stays the *sharing* channel: a plan in the
//! hash wins over the stored one on startup and is then persisted too.
//!
//! Everything except the two thin `window` wrappers at the bottom is plain data
//! and host-testable.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::models::MealPlan;
use crate::quantity::Dimension;
use crate::shopping::{ShoppingCategory, ShoppingLine};

/// Bump when the stored shape changes incompatibly; older app versions refuse
/// newer payloads instead of misreading them.
pub const SCHEMA_VERSION: u32 = 2;

const STORAGE_KEY: &str = "cooking-rs.state";

/// Everything persisted locally.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PersistedState {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub plan: MealPlan,
    #[serde(default)]
    pub grocery: GroceryState,
}

impl PersistedState {
    pub fn new(plan: MealPlan, grocery: GroceryState) -> Self {
        Self {
            version: SCHEMA_VERSION,
            plan,
            grocery,
        }
    }
}

/// Per-device grocery-list state. Not shared via the URL.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GroceryState {
    /// Lines ticked off, each with the amounts it had when ticked.
    #[serde(default)]
    pub checked: Vec<CheckedItem>,
    /// Catalog keys the user always has at home; hidden from the list. Sticky
    /// across plan changes.
    #[serde(default)]
    pub omitted: BTreeSet<String>,
    /// Hide lines that are only "to taste".
    #[serde(default)]
    pub hide_to_taste: bool,
}

/// A ticked-off line and the amounts it showed at the time, so a tick is
/// forgotten once the plan changes what has to be bought.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CheckedItem {
    pub key: String,
    #[serde(default)]
    pub subtotals: Vec<(Dimension, f64)>,
}

/// The comparable amounts of a line (rounded so float noise doesn't untick it).
fn snapshot(line: &ShoppingLine) -> Vec<(Dimension, f64)> {
    line.subtotals
        .iter()
        .map(|s| (s.dimension, (s.amount * 1000.0).round() / 1000.0))
        .collect()
}

impl GroceryState {
    pub fn is_checked(&self, key: &str) -> bool {
        self.checked.iter().any(|c| c.key == key)
    }

    /// Tick or untick a line, recording its current amounts.
    pub fn toggle_checked(&mut self, line: &ShoppingLine) {
        if let Some(i) = self.checked.iter().position(|c| c.key == line.key) {
            self.checked.remove(i);
        } else {
            self.checked.push(CheckedItem {
                key: line.key.clone(),
                subtotals: snapshot(line),
            });
        }
    }

    pub fn is_omitted(&self, key: &str) -> bool {
        self.omitted.contains(key)
    }

    pub fn toggle_omitted(&mut self, key: &str) {
        if !self.omitted.remove(key) {
            self.omitted.insert(key.to_string());
        }
    }

    /// The lines actually shown and exported: without the "to taste"-only lines
    /// when hidden, and without the ingredients the user has at home. Empty
    /// categories are dropped.
    pub fn visible(&self, categories: &[ShoppingCategory]) -> Vec<ShoppingCategory> {
        categories
            .iter()
            .filter_map(|cat| {
                let lines: Vec<ShoppingLine> = cat
                    .lines
                    .iter()
                    .filter(|line| {
                        let to_taste_only = line.subtotals.is_empty()
                            && line.custom_quantities.is_empty()
                            && line.to_taste;
                        !(self.is_omitted(&line.key) || self.hide_to_taste && to_taste_only)
                    })
                    .cloned()
                    .collect();
                (!lines.is_empty()).then(|| ShoppingCategory {
                    name: cat.name.clone(),
                    lines,
                })
            })
            .collect()
    }

    /// Number of visible lines not yet ticked (the grocery tab badge).
    pub fn remaining(&self, categories: &[ShoppingCategory]) -> usize {
        self.visible(categories)
            .iter()
            .flat_map(|c| c.lines.iter())
            .filter(|l| !self.is_checked(&l.key))
            .count()
    }

    /// Drop ticks for lines that are gone or whose amounts changed. Returns
    /// whether anything was removed. `omitted` is never pruned.
    pub fn prune_checked(&mut self, lines: &[ShoppingLine]) -> bool {
        let before = self.checked.len();
        self.checked.retain(|c| {
            lines
                .iter()
                .any(|l| l.key == c.key && snapshot(l) == c.subtotals)
        });
        self.checked.len() != before
    }
}

/// Serialize for storage.
pub fn encode(state: &PersistedState) -> Option<String> {
    serde_json::to_string(state).ok()
}

/// Parse a stored payload. `None` for garbage or a payload newer than this
/// build understands. The plan is normalized to seven days.
pub fn decode(raw: &str) -> Option<PersistedState> {
    let mut state: PersistedState = serde_json::from_str(raw).ok()?;
    if state.version > SCHEMA_VERSION {
        return None;
    }
    state.version = SCHEMA_VERSION;
    state.plan.normalize();
    Some(state)
}

/// What the app starts with.
#[derive(Clone, Debug, PartialEq)]
pub struct Startup {
    /// The active state: the plan shown and edited, plus this device's grocery state.
    pub state: PersistedState,
    /// `Some` while **previewing a shared plan**: the device's own plan (or an
    /// empty week if it had none), kept aside until the user keeps the shared
    /// plan or goes back to their own. Nothing is persisted while this is `Some`.
    pub own_plan: Option<MealPlan>,
}

/// Startup precedence: a plan in the URL hash (a share link) is shown first, with
/// the device's own plan set aside; the device's grocery state is always kept.
/// Without a hash plan, the stored state is used; otherwise an empty week.
pub fn resolve_initial(hash: Option<MealPlan>, stored: Option<PersistedState>) -> Startup {
    match (hash, stored) {
        (Some(shared), stored) => {
            let (own, grocery) = stored.map(|s| (s.plan, s.grocery)).unwrap_or_default();
            Startup {
                state: PersistedState::new(shared, grocery),
                own_plan: Some(own),
            }
        }
        (None, Some(stored)) => Startup {
            state: PersistedState::new(stored.plan, stored.grocery),
            own_plan: None,
        },
        (None, None) => Startup {
            state: PersistedState::new(MealPlan::default(), GroceryState::default()),
            own_plan: None,
        },
    }
}

/// Read the stored state, if any and if readable (private mode may refuse).
pub fn load() -> Option<PersistedState> {
    let raw = leptos::prelude::window()
        .local_storage()
        .ok()??
        .get_item(STORAGE_KEY)
        .ok()??;
    decode(&raw)
}

/// Write the state; failures (quota, private mode) are ignored on purpose.
pub fn save(state: &PersistedState) {
    if let (Some(raw), Ok(Some(store))) = (encode(state), leptos::prelude::window().local_storage())
    {
        let _ = store.set_item(STORAGE_KEY, &raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Extra, RecipeIngredient, DAYS_PER_WEEK};
    use crate::shopping::Subtotal;

    fn line(key: &str, amounts: &[(Dimension, f64)]) -> ShoppingLine {
        ShoppingLine {
            key: key.into(),
            name: key.to_uppercase(),
            count: None,
            subtotals: amounts
                .iter()
                .map(|&(dimension, amount)| Subtotal { dimension, amount })
                .collect(),
            to_taste: false,
            optional: false,
            sources: vec![],
            custom_quantities: vec![],
        }
    }

    fn sample() -> PersistedState {
        let mut plan = MealPlan::default();
        plan.add_meal(1, "dal".into());
        plan.extras.push(Extra::Catalog(RecipeIngredient::new(
            "milk".into(),
            Some(1000.0),
        )));
        let mut grocery = GroceryState::default();
        grocery.toggle_checked(&line("onion", &[(Dimension::Mass, 250.0)]));
        grocery.toggle_omitted("salt");
        grocery.hide_to_taste = true;
        PersistedState::new(plan, grocery)
    }

    #[test]
    fn round_trips_through_json() {
        let state = sample();
        let raw = encode(&state).unwrap();
        assert!(raw.starts_with("{\"version\":2"));
        assert_eq!(decode(&raw), Some(state));
    }

    #[test]
    fn rejects_future_versions_and_garbage_defaults_missing_fields() {
        assert_eq!(decode("{\"version\":99}"), None);
        assert_eq!(decode("not json"), None);
        // Older/partial payloads decode with defaults and a 7-day plan.
        let partial = decode("{\"plan\":{\"days\":[{\"dinner\":\"soup\"}]}}").unwrap();
        assert_eq!(partial.version, SCHEMA_VERSION);
        assert_eq!(partial.plan.days.len(), DAYS_PER_WEEK);
        assert_eq!(partial.plan.meal(0, 0).unwrap().recipe, "soup");
        assert_eq!(partial.grocery, GroceryState::default());
    }

    #[test]
    fn hash_plan_is_previewed_and_keeps_device_grocery_state() {
        let stored = sample();
        let mut shared = MealPlan::default();
        shared.add_meal(6, "pancakes".into());

        let r = resolve_initial(Some(shared.clone()), Some(stored.clone()));
        assert_eq!(r.state.plan, shared);
        assert_eq!(r.state.grocery, stored.grocery);
        assert_eq!(r.state.version, SCHEMA_VERSION);
        assert_eq!(r.own_plan, Some(stored.plan.clone()));

        let r = resolve_initial(Some(shared.clone()), None);
        assert_eq!(r.state.plan, shared);
        assert_eq!(r.state.grocery, GroceryState::default());
        assert_eq!(r.own_plan, Some(MealPlan::default()));

        let r = resolve_initial(None, Some(stored.clone()));
        assert_eq!(r.state.plan, stored.plan);
        assert_eq!(r.state.grocery, stored.grocery);
        assert_eq!(r.own_plan, None);

        let r = resolve_initial(None, None);
        assert_eq!(r.state.plan, MealPlan::default());
        assert_eq!(r.own_plan, None);
    }

    #[test]
    fn visible_and_remaining_apply_omissions_and_the_to_taste_toggle() {
        let mut salt = line("salt", &[]);
        salt.to_taste = true;
        let mut bread = line("custom:bread", &[]);
        bread.custom_quantities = vec!["1 loaf".into()];
        let cats = vec![
            ShoppingCategory {
                name: "Produce".into(),
                lines: vec![
                    line("onion", &[(Dimension::Mass, 250.0)]),
                    line("garlic", &[(Dimension::Mass, 12.0)]),
                ],
            },
            ShoppingCategory {
                name: "Spices & Oils".into(),
                lines: vec![salt],
            },
            ShoppingCategory {
                name: "Other".into(),
                lines: vec![bread],
            },
        ];
        let mut g = GroceryState::default();
        assert_eq!(g.visible(&cats).len(), 3);
        assert_eq!(g.remaining(&cats), 4);

        g.hide_to_taste = true;
        let v = g.visible(&cats);
        assert_eq!(v.len(), 2, "to-taste-only aisle dropped, custom kept");
        assert_eq!(g.remaining(&cats), 3);

        g.toggle_omitted("onion");
        g.toggle_checked(&line("garlic", &[(Dimension::Mass, 12.0)]));
        assert_eq!(g.visible(&cats)[0].lines.len(), 1);
        assert_eq!(g.remaining(&cats), 1);
    }

    #[test]
    fn prune_drops_removed_and_changed_lines_keeps_equal_and_omitted() {
        let mut g = GroceryState::default();
        g.toggle_checked(&line("onion", &[(Dimension::Mass, 250.0)]));
        g.toggle_checked(&line("garlic", &[(Dimension::Mass, 12.0)]));
        g.toggle_checked(&line("egg", &[(Dimension::Count, 4.0)]));
        g.toggle_omitted("salt");

        let current = vec![
            line("onion", &[(Dimension::Mass, 250.0004)]), // float noise: still equal
            line("garlic", &[(Dimension::Mass, 16.0)]),    // amount changed
                                                           // egg: gone
        ];
        assert!(g.prune_checked(&current));
        assert!(g.is_checked("onion"));
        assert!(!g.is_checked("garlic"));
        assert!(!g.is_checked("egg"));
        assert!(g.is_omitted("salt"));
        assert!(!g.prune_checked(&current), "second pass is a no-op");
    }

    #[test]
    fn toggles_flip_state() {
        let mut g = GroceryState::default();
        let l = line("onion", &[(Dimension::Mass, 1.0)]);
        g.toggle_checked(&l);
        assert!(g.is_checked("onion"));
        g.toggle_checked(&l);
        assert!(!g.is_checked("onion"));
        g.toggle_omitted("salt");
        g.toggle_omitted("salt");
        assert!(!g.is_omitted("salt"));
    }
}
