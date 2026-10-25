//! Local persistence boundary.
//!
//! The URL hash (see [`crate::state`]) is the plan: the address bar always
//! carries it. `localStorage` only remembers the **last plan opened** on this
//! device, so the home-screen icon (which opens the bare URL) comes back to it,
//! plus this device's grocery check-off state, which is never shared.
//!
//! Everything except the two thin `window` wrappers at the bottom is plain data
//! and host-testable.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::models::{MealPlan, PlanWire};
use crate::quantity::Dimension;
use crate::shopping::{ShoppingCategory, ShoppingLine};

/// Bump when the stored shape changes incompatibly; older app versions refuse
/// newer payloads instead of misreading them.
pub const SCHEMA_VERSION: u32 = 3;

const STORAGE_KEY: &str = "cooking-rs.state";

/// Everything persisted locally. Written as is; read back through
/// [`StoredWire`] so older payloads (weekday plans, v2 and before) migrate.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct PersistedState {
    pub version: u32,
    pub plan: MealPlan,
    pub grocery: GroceryState,
}

/// [`PersistedState`] as stored, before its plan is tied to a calendar.
#[derive(Deserialize)]
struct StoredWire {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    plan: PlanWire,
    #[serde(default)]
    grocery: GroceryState,
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
    /// Which planned days the list covers.
    #[serde(default)]
    pub range: GroceryRange,
}

/// The days the grocery list covers, always starting today.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroceryRange {
    /// Today through the last planned day.
    #[default]
    All,
    /// Today and the following days, `n` days in all.
    Days(u8),
}

impl GroceryRange {
    /// The presets offered on the grocery page, with their chip labels.
    pub const PRESETS: [(GroceryRange, &'static str); 4] = [
        (GroceryRange::Days(1), "Today"),
        (GroceryRange::Days(3), "3 days"),
        (GroceryRange::Days(7), "7 days"),
        (GroceryRange::All, "All planned"),
    ];

    /// The last day covered (never before `today`).
    pub fn until(self, plan: &MealPlan, today: Date) -> Date {
        match self {
            GroceryRange::All => plan.last_date().map_or(today, |d| d.max(today)),
            GroceryRange::Days(n) => today.add_days(i32::from(n.max(1)) - 1),
        }
    }
}

/// A ticked-off line and the amounts it showed at the time, so a tick is
/// forgotten once the plan needs more than was bought.
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

    /// Drop ticks for lines that are gone or now need more than when they were
    /// ticked (a larger amount, or a new dimension). A line that needs less, as
    /// when a past day drops off the plan, stays ticked. Returns whether
    /// anything was removed. `omitted` is never pruned.
    pub fn prune_checked(&mut self, lines: &[ShoppingLine]) -> bool {
        let before = self.checked.len();
        self.checked.retain(|c| {
            lines.iter().any(|l| {
                l.key == c.key
                    && snapshot(l).iter().all(|&(dim, now)| {
                        c.subtotals
                            .iter()
                            .any(|&(ticked_dim, ticked)| ticked_dim == dim && now <= ticked)
                    })
            })
        });
        self.checked.len() != before
    }
}

/// Serialize for storage.
pub fn encode(state: &PersistedState) -> Option<String> {
    serde_json::to_string(state).ok()
}

/// Parse a stored payload as of `today`. `None` for garbage or a payload newer
/// than this build understands. Weekday plans (v2 and before) move to their
/// next dates and past days are dropped (see [`PlanWire::into_plan`]).
pub fn decode(raw: &str, today: Date) -> Option<PersistedState> {
    let stored: StoredWire = serde_json::from_str(raw).ok()?;
    if stored.version > SCHEMA_VERSION {
        return None;
    }
    Some(PersistedState::new(
        stored.plan.into_plan(today),
        stored.grocery,
    ))
}

/// Startup precedence: a plan in the URL hash (a link someone sent, or the
/// address bar on reload) wins; otherwise the last plan opened on this device;
/// otherwise an empty plan. The device's grocery state is always kept.
pub fn resolve_initial(hash: Option<MealPlan>, stored: Option<PersistedState>) -> PersistedState {
    let (last, grocery) = stored.map(|s| (s.plan, s.grocery)).unwrap_or_default();
    PersistedState::new(hash.unwrap_or(last), grocery)
}

/// Read the stored state, if any and if readable (private mode may refuse).
pub fn load(today: Date) -> Option<PersistedState> {
    let raw = leptos::prelude::window()
        .local_storage()
        .ok()??
        .get_item(STORAGE_KEY)
        .ok()??;
    decode(&raw, today)
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
    use crate::models::{Extra, RecipeIngredient};
    use crate::shopping::Subtotal;

    /// A Sunday.
    fn today() -> Date {
        Date::from_ymd(2026, 10, 25).unwrap()
    }

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
        plan.add_meal(today().add_days(1), "dal".into());
        plan.extras.push(Extra::Catalog(RecipeIngredient::new(
            "milk".into(),
            Some(1000.0),
        )));
        let mut grocery = GroceryState::default();
        grocery.toggle_checked(&line("onion", &[(Dimension::Mass, 250.0)]));
        grocery.toggle_omitted("salt");
        grocery.hide_to_taste = true;
        grocery.range = GroceryRange::Days(3);
        PersistedState::new(plan, grocery)
    }

    #[test]
    fn round_trips_through_json() {
        let state = sample();
        let raw = encode(&state).unwrap();
        assert!(raw.starts_with("{\"version\":3"), "{raw}");
        assert!(raw.contains("\"range\":{\"days\":3}"), "{raw}");
        assert_eq!(decode(&raw, today()), Some(state));
    }

    #[test]
    fn rejects_future_versions_and_garbage_defaults_missing_fields() {
        assert_eq!(decode("{\"version\":99}", today()), None);
        assert_eq!(decode("not json", today()), None);
        // Older/partial payloads decode with defaults; weekday plans get dates.
        let partial = decode("{\"plan\":{\"days\":[{\"dinner\":\"soup\"}]}}", today()).unwrap();
        assert_eq!(partial.version, SCHEMA_VERSION);
        assert_eq!(
            partial.plan.meal(today().add_days(1), 0).unwrap().recipe,
            "soup"
        );
        assert_eq!(partial.grocery, GroceryState::default());
        assert_eq!(decode("{}", today()).unwrap().plan, MealPlan::default());
    }

    #[test]
    fn a_v2_payload_migrates_to_dates() {
        let v2 = r#"{"version":2,"plan":{"days":[["oats"],[],[],[],[],[],["roast"]],
            "extras":[{"name":"Bread"}]},
            "grocery":{"checked":[{"key":"onion","subtotals":[["Mass",250.0]]}],
            "omitted":["salt"],"hide_to_taste":true}}"#;
        let state = decode(v2, today()).unwrap();
        assert_eq!(state.version, SCHEMA_VERSION);
        assert_eq!(state.plan.meal(today(), 0).unwrap().recipe, "roast");
        assert_eq!(
            state.plan.meal(today().add_days(1), 0).unwrap().recipe,
            "oats"
        );
        assert_eq!(state.plan.extras.len(), 1);
        assert!(state.grocery.is_checked("onion"));
        assert!(state.grocery.is_omitted("salt"));
        assert_eq!(state.grocery.range, GroceryRange::All);
    }

    #[test]
    fn grocery_range_ends() {
        let t = today();
        let mut plan = MealPlan::default();
        assert_eq!(GroceryRange::All.until(&plan, t), t, "nothing planned");
        plan.add_meal(t.add_days(9), "dal".into());
        assert_eq!(GroceryRange::All.until(&plan, t), t.add_days(9));
        assert_eq!(GroceryRange::Days(1).until(&plan, t), t);
        assert_eq!(GroceryRange::Days(3).until(&plan, t), t.add_days(2));
        assert_eq!(GroceryRange::Days(0).until(&plan, t), t);
        // A plan last decoded yesterday still never ends the range in the past.
        assert_eq!(
            GroceryRange::All.until(&plan, t.add_days(12)),
            t.add_days(12)
        );
    }

    #[test]
    fn hash_plan_wins_and_device_grocery_state_is_kept() {
        let stored = sample();
        let mut shared = MealPlan::default();
        shared.add_meal(today().add_days(6), "pancakes".into());

        let r = resolve_initial(Some(shared.clone()), Some(stored.clone()));
        assert_eq!(r.plan, shared);
        assert_eq!(r.grocery, stored.grocery);
        assert_eq!(r.version, SCHEMA_VERSION);

        let r = resolve_initial(Some(shared.clone()), None);
        assert_eq!(r.plan, shared);
        assert_eq!(r.grocery, GroceryState::default());

        let r = resolve_initial(None, Some(stored.clone()));
        assert_eq!(r, stored);

        let r = resolve_initial(None, None);
        assert_eq!(r.plan, MealPlan::default());
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
    fn prune_drops_gone_and_grown_lines_keeps_equal_shrunk_and_omitted() {
        let mut g = GroceryState::default();
        g.toggle_checked(&line("onion", &[(Dimension::Mass, 250.0)]));
        g.toggle_checked(&line("garlic", &[(Dimension::Mass, 12.0)]));
        g.toggle_checked(&line("egg", &[(Dimension::Count, 4.0)]));
        g.toggle_checked(&line("rice", &[(Dimension::Mass, 500.0)]));
        g.toggle_checked(&line("milk", &[(Dimension::Volume, 1000.0)]));
        g.toggle_checked(&line("salt", &[]));
        g.toggle_omitted("salt");

        let current = vec![
            line("onion", &[(Dimension::Mass, 250.0004)]), // float noise: still equal
            line("garlic", &[(Dimension::Mass, 16.0)]),    // needs more
            // egg: gone
            line("rice", &[(Dimension::Mass, 200.0)]), // needs less: still bought
            line(
                "milk",
                &[(Dimension::Volume, 500.0), (Dimension::Count, 1.0)],
            ), // new dimension
            line("salt", &[]),                         // to taste: unchanged
        ];
        assert!(g.prune_checked(&current));
        assert!(g.is_checked("onion"));
        assert!(!g.is_checked("garlic"));
        assert!(!g.is_checked("egg"));
        assert!(g.is_checked("rice"));
        assert!(!g.is_checked("milk"));
        assert!(g.is_checked("salt"));
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
