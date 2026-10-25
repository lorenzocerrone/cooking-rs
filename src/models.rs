//! Data model for the app. Recipes and ingredients are authored as JSON and
//! embedded at build time; the meal plan is the only mutable, shareable state.

use std::collections::{BTreeMap, HashMap};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::date::Date;
use crate::quantity::{CountSpec, Unit};

/// A single recipe, as authored in `recipes/*.json`. Ingredients reference the
/// central catalog ([`IngredientCatalog`]) by key rather than embedding their own
/// names/units, so amounts can be summed across recipes.
///
/// Everything beyond the original core fields is optional (`#[serde(default)]`) so
/// older recipe files keep parsing unchanged.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub title: String,
    pub servings: u32,
    pub ingredients: Vec<RecipeIngredient>,
    pub steps: Vec<String>,

    /// One-line/paragraph summary shown on cards and the detail page.
    #[serde(default)]
    pub description: Option<String>,
    /// Cuisine label (e.g. "Italian"), used for filtering.
    #[serde(default)]
    pub cuisine: Option<String>,
    /// Free-form tags (e.g. "vegetarian", "quick"), used for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Preparation time in minutes.
    #[serde(default)]
    pub prep_time: Option<u32>,
    /// Cooking time in minutes.
    #[serde(default)]
    pub cook_time: Option<u32>,
    /// Optional image URL/path (image hosting itself is a non-goal).
    #[serde(default)]
    pub image: Option<String>,
    /// Who wrote the recipe ("Claude" for generated ones), used for filtering.
    /// Required by the data validation; optional here so a missing field is
    /// reported by that rule rather than as a parse error.
    #[serde(default)]
    pub author: Option<String>,
}

/// A recipe's reference to a catalog ingredient. `amount: None` means "to taste"
/// (rendered but not numerically summed). Also used for a plan's extras, so the
/// optional fields are skipped when empty to keep share links short.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeIngredient {
    /// Key into the [`IngredientCatalog`].
    pub ingredient: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    /// Unit as written in the recipe: any [`Unit`] spelling, or the ingredient's
    /// piece label ("clove", "can"). `None` means the catalog's shopping unit.
    /// Kept as text because resolving labels needs the catalog
    /// (see [`crate::quantity::resolve_unit`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Preparation note shown next to the name ("finely chopped").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Optional ingredients are listed but flagged, in the recipe and the list.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

impl RecipeIngredient {
    /// An ingredient in its catalog unit with no note.
    pub fn new(ingredient: String, amount: Option<f64>) -> Self {
        Self {
            ingredient,
            amount,
            unit: None,
            note: None,
            optional: false,
        }
    }
}

/// One entry in the ingredient catalog (`ingredients.json`). The map key is the
/// lookup id used by [`RecipeIngredient::ingredient`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CatalogIngredient {
    pub name: String,
    /// The **shopping unit**: how this ingredient is bought and summed. Must be a
    /// base unit (`g`, `ml` or `pcs`); recipes may use any convertible unit.
    pub unit: Unit,
    /// Shopping-aisle grouping (e.g. "Produce", "Dairy & Eggs").
    pub category: String,
    /// How the ingredient is counted in pieces, if recipes may do so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<CountSpec>,
}

/// The full ingredient catalog: `key -> entry`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct IngredientCatalog(pub HashMap<String, CatalogIngredient>);

impl IngredientCatalog {
    pub fn get(&self, key: &str) -> Option<&CatalogIngredient> {
        self.0.get(key)
    }
}

/// A chosen meal: a recipe id plus an optional target servings count. `servings:
/// None` means "use the recipe's own servings" (no scaling).
///
/// To keep shared `#plan=` links stable, this (de)serializes as a **bare string**
/// when `servings` is `None` (the original format) and as an object only when a
/// custom servings count is set. Legacy id-only links therefore keep decoding.
#[derive(Clone, Debug, PartialEq)]
pub struct MealEntry {
    pub recipe: String,
    pub servings: Option<u32>,
}

impl MealEntry {
    pub fn new(recipe: String) -> Self {
        Self {
            recipe,
            servings: None,
        }
    }
}

impl Serialize for MealEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.servings {
            None => serializer.serialize_str(&self.recipe),
            Some(servings) => {
                let mut st = serializer.serialize_struct("MealEntry", 2)?;
                st.serialize_field("recipe", &self.recipe)?;
                st.serialize_field("servings", &servings)?;
                st.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for MealEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Id(String),
            Full {
                recipe: String,
                #[serde(default)]
                servings: Option<u32>,
            },
        }

        Ok(match Repr::deserialize(deserializer)? {
            Repr::Id(recipe) => MealEntry {
                recipe,
                servings: None,
            },
            Repr::Full { recipe, servings } => MealEntry { recipe, servings },
        })
    }
}

/// One day's meals, in the order they were added. A day has no fixed slots.
///
/// Serializes as a plain JSON array of [`MealEntry`]. The decoder also accepts
/// the original `{breakfast, lunch, dinner}` object (slots become list entries
/// in that order, empty slots are dropped), so links shared before days became
/// lists still open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DayPlan {
    pub meals: Vec<MealEntry>,
}

impl Serialize for DayPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.meals.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DayPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            List(Vec<MealEntry>),
            Slots {
                #[serde(default)]
                breakfast: Option<MealEntry>,
                #[serde(default)]
                lunch: Option<MealEntry>,
                #[serde(default)]
                dinner: Option<MealEntry>,
            },
        }

        Ok(match Repr::deserialize(deserializer)? {
            Repr::List(meals) => DayPlan { meals },
            Repr::Slots {
                breakfast,
                lunch,
                dinner,
            } => DayPlan {
                meals: [breakfast, lunch, dinner].into_iter().flatten().collect(),
            },
        })
    }
}

/// A grocery item added by hand, outside any recipe.
///
/// Untagged on the wire: a catalog item is exactly a [`RecipeIngredient`] (the
/// pre-existing format, keyed by `ingredient`), a custom item is `{name, quantity}`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Extra {
    /// A catalog ingredient with an optional amount; summed like recipe lines.
    Catalog(RecipeIngredient),
    /// Anything not in the catalog. The quantity is free text, shown as typed.
    Custom {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quantity: Option<String>,
    },
}

/// The meal plan: each calendar date's meals, plus extra grocery items. This is
/// the value that gets encoded into a share link and persisted locally.
///
/// Days without meals are never stored, so iterating `dates` visits only the
/// planned days, in date order. On the wire it is `{"dates": {"2026-10-27":
/// [...]}, "extras": [...]}`; decoding goes through [`PlanWire`], which also
/// accepts the older seven-weekday shape.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct MealPlan {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub dates: BTreeMap<Date, DayPlan>,
    /// Extra grocery items not tied to a meal (staples, snacks), folded into the
    /// shopping list unscaled.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<Extra>,
}

impl MealPlan {
    /// True when no day holds a meal and there are no extras.
    pub fn is_empty(&self) -> bool {
        self.dates.is_empty() && self.extras.is_empty()
    }

    /// The meals planned for `date` (empty when none).
    pub fn day(&self, date: Date) -> &[MealEntry] {
        self.dates.get(&date).map_or(&[], |d| d.meals.as_slice())
    }

    /// Every chosen meal, in date order and each day's order.
    pub fn entries(&self) -> impl Iterator<Item = &MealEntry> {
        self.dates.values().flat_map(|d| d.meals.iter())
    }

    /// The meals dated `from..=to`, in order.
    pub fn entries_in(&self, from: Date, to: Date) -> impl Iterator<Item = &MealEntry> {
        (from <= to)
            .then(|| self.dates.range(from..=to))
            .into_iter()
            .flatten()
            .flat_map(|(_, d)| d.meals.iter())
    }

    /// Number of planned meals.
    pub fn meal_count(&self) -> usize {
        self.entries().count()
    }

    /// The last date holding a meal.
    pub fn last_date(&self) -> Option<Date> {
        self.dates.keys().next_back().copied()
    }

    /// The `idx`-th meal of a day, if any.
    pub fn meal(&self, date: Date, idx: usize) -> Option<&MealEntry> {
        self.day(date).get(idx)
    }

    /// Append a recipe to a day (default servings).
    pub fn add_meal(&mut self, date: Date, recipe: String) {
        self.dates
            .entry(date)
            .or_default()
            .meals
            .push(MealEntry::new(recipe));
    }

    /// Remove the `idx`-th meal of a day. Out-of-range indices are ignored; a day
    /// left without meals is dropped.
    pub fn remove_meal(&mut self, date: Date, idx: usize) {
        if let Some(d) = self.dates.get_mut(&date) {
            if idx < d.meals.len() {
                d.meals.remove(idx);
            }
            if d.meals.is_empty() {
                self.dates.remove(&date);
            }
        }
    }

    /// Set (or clear, with `None`) the custom servings of a meal. Does nothing
    /// for an index that holds no meal.
    pub fn set_servings(&mut self, date: Date, idx: usize, servings: Option<u32>) {
        if let Some(entry) = self.dates.get_mut(&date).and_then(|d| d.meals.get_mut(idx)) {
            entry.servings = servings;
        }
    }

    /// Forget the days before `today`. Past meals are never shown or shopped for.
    pub fn drop_before(&mut self, today: Date) {
        self.dates = self.dates.split_off(&today);
    }
}

/// A meal plan as found in a share link or in storage, before it is tied to a
/// calendar. Untagged so every link ever shared keeps decoding:
/// - `Weekdays`: the older seven Monday-first `days` (each a list, or the
///   original `{breakfast, lunch, dinner}` object, see [`DayPlan`]);
/// - `Dated`: the current shape, keyed by ISO date.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PlanWire {
    // `Weekdays` first: it requires `days`, while every `Dated` field defaults.
    Weekdays {
        days: Vec<DayPlan>,
        #[serde(default)]
        extras: Vec<Extra>,
    },
    Dated {
        #[serde(default)]
        dates: BTreeMap<Date, DayPlan>,
        #[serde(default)]
        extras: Vec<Extra>,
    },
}

impl Default for PlanWire {
    fn default() -> Self {
        PlanWire::Dated {
            dates: BTreeMap::new(),
            extras: Vec::new(),
        }
    }
}

impl PlanWire {
    /// The plan as of `today`: weekday plans move each day to its next date on
    /// or after today (Monday → the coming Monday), past dates are dropped, and
    /// empty days are not kept.
    pub fn into_plan(self, today: Date) -> MealPlan {
        let (dates, extras) = match self {
            PlanWire::Dated { dates, extras } => (dates, extras),
            PlanWire::Weekdays { days, extras } => {
                let dates = days
                    .into_iter()
                    .take(7)
                    .enumerate()
                    .map(|(weekday, day)| (today.next_weekday(weekday), day))
                    .collect();
                (dates, extras)
            }
        };
        let mut plan = MealPlan {
            dates: dates
                .into_iter()
                .filter(|(_, d)| !d.meals.is_empty())
                .collect(),
            extras,
        };
        plan.drop_before(today);
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Sunday.
    fn today() -> Date {
        Date::from_ymd(2026, 10, 25).unwrap()
    }

    fn decode(json: &str) -> MealPlan {
        serde_json::from_str::<PlanWire>(json)
            .unwrap()
            .into_plan(today())
    }

    #[test]
    fn meal_entry_serializes_bare_string_without_servings() {
        let entry = MealEntry::new("carbonara".into());
        assert_eq!(serde_json::to_string(&entry).unwrap(), "\"carbonara\"");
    }

    #[test]
    fn meal_entry_serializes_object_with_servings() {
        let entry = MealEntry {
            recipe: "carbonara".into(),
            servings: Some(4),
        };
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            "{\"recipe\":\"carbonara\",\"servings\":4}"
        );
    }

    #[test]
    fn meal_entry_decodes_from_string_or_object() {
        let from_str: MealEntry = serde_json::from_str("\"carbonara\"").unwrap();
        assert_eq!(from_str, MealEntry::new("carbonara".into()));

        let from_obj: MealEntry =
            serde_json::from_str("{\"recipe\":\"carbonara\",\"servings\":4}").unwrap();
        assert_eq!(from_obj.recipe, "carbonara");
        assert_eq!(from_obj.servings, Some(4));
    }

    #[test]
    fn dated_plan_serializes_by_iso_date_and_round_trips() {
        let mut plan = MealPlan::default();
        let tomorrow = today().add_days(1);
        plan.add_meal(tomorrow, "oats".into());
        plan.add_meal(tomorrow, "dal".into());
        plan.set_servings(tomorrow, 1, Some(6));
        plan.add_meal(today(), "soup".into());
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(
            json,
            r#"{"dates":{"2026-10-25":["soup"],"2026-10-26":["oats",{"recipe":"dal","servings":6}]}}"#
        );
        assert_eq!(decode(&json), plan);
        assert_eq!(serde_json::to_string(&MealPlan::default()).unwrap(), "{}");
        assert_eq!(decode("{}"), MealPlan::default());
    }

    #[test]
    fn weekday_plans_move_to_their_next_date() {
        // Today is a Sunday: Monday..Saturday land in the coming week, Sunday today.
        let json = r#"{"days":[["oats"],[],[],[],[],["pizza"],["roast"]]}"#;
        let plan = decode(json);
        let dates: Vec<String> = plan.dates.keys().map(|d| d.to_string()).collect();
        assert_eq!(dates, vec!["2026-10-25", "2026-10-26", "2026-10-31"]);
        assert_eq!(plan.day(today())[0].recipe, "roast");
        assert_eq!(plan.day(today().add_days(1))[0].recipe, "oats");
        assert_eq!(plan.day(today().add_days(6))[0].recipe, "pizza");
    }

    #[test]
    fn legacy_slot_days_decode_into_dated_lists() {
        // Links shared before days became lists: slots as an object, meals as
        // bare ids or {recipe, servings} objects, empty slots as null/missing.
        let json = r#"{"days":[
            {"breakfast":"omelette","lunch":null,"dinner":{"recipe":"dal","servings":4}},
            {"dinner":"soup"},
            {}
        ]}"#;
        let plan = decode(json);
        let monday = today().next_weekday(0);
        let day0: Vec<_> = plan.day(monday).iter().map(|m| m.recipe.as_str()).collect();
        assert_eq!(day0, vec!["omelette", "dal"]);
        assert_eq!(plan.meal(monday, 1).unwrap().servings, Some(4));
        assert_eq!(plan.meal(monday.add_days(1), 0).unwrap().recipe, "soup");
        assert_eq!(plan.dates.len(), 2, "empty days are not kept");
    }

    #[test]
    fn past_dates_and_empty_days_are_dropped_on_decode() {
        let json = r#"{"dates":{"2026-10-20":["old"],"2026-10-24":["yesterday"],
            "2026-10-25":["today"],"2026-10-28":[]}}"#;
        let plan = decode(json);
        let ids: Vec<_> = plan.entries().map(|m| m.recipe.as_str()).collect();
        assert_eq!(ids, vec!["today"]);
        assert_eq!(plan.dates.len(), 1);
        assert!(serde_json::from_str::<PlanWire>(r#"{"dates":{"tomorrow":[]}}"#).is_err());
    }

    #[test]
    fn extras_are_omitted_when_empty_and_both_kinds_round_trip() {
        let mut plan = MealPlan::default();
        plan.extras.push(Extra::Catalog(RecipeIngredient::new(
            "milk".into(),
            Some(1000.0),
        )));
        plan.extras.push(Extra::Custom {
            name: "Bread".into(),
            quantity: Some("1 loaf".into()),
        });
        plan.extras.push(Extra::Custom {
            name: "Candles".into(),
            quantity: None,
        });
        let json = serde_json::to_string(&plan).unwrap();
        assert!(
            json.contains(r#"{"ingredient":"milk","amount":1000.0}"#),
            "{json}"
        );
        assert!(
            json.contains(r#"{"name":"Bread","quantity":"1 loaf"}"#),
            "{json}"
        );
        assert!(json.contains(r#"{"name":"Candles"}"#), "{json}");
        assert_eq!(decode(&json), plan);

        // The pre-`Extra` format (a bare RecipeIngredient list) still decodes.
        let legacy = decode(r#"{"days":[],"extras":[{"ingredient":"milk"}]}"#);
        assert_eq!(
            legacy.extras,
            vec![Extra::Catalog(RecipeIngredient::new("milk".into(), None))]
        );
    }

    #[test]
    fn add_remove_servings_and_ranges() {
        let t = today();
        let mut plan = MealPlan::default();
        assert!(plan.is_empty());
        assert_eq!(plan.last_date(), None);

        plan.add_meal(t.add_days(2), "dal".into());
        plan.add_meal(t, "soup".into());
        plan.add_meal(t.add_days(9), "tacos".into());
        assert!(!plan.is_empty());
        let ids: Vec<&str> = plan.entries().map(|e| e.recipe.as_str()).collect();
        assert_eq!(ids, vec!["soup", "dal", "tacos"], "date order");
        assert_eq!(plan.meal_count(), 3);
        assert_eq!(plan.last_date(), Some(t.add_days(9)));

        let in_range: Vec<&str> = plan
            .entries_in(t, t.add_days(2))
            .map(|e| e.recipe.as_str())
            .collect();
        assert_eq!(in_range, vec!["soup", "dal"]);
        assert_eq!(plan.entries_in(t.add_days(3), t.add_days(8)).count(), 0);
        assert_eq!(plan.entries_in(t.add_days(2), t).count(), 0, "reversed");

        plan.set_servings(t.add_days(2), 0, Some(6));
        plan.set_servings(t.add_days(2), 5, Some(6)); // no such meal: ignored
        plan.set_servings(t.add_days(4), 0, Some(6)); // no such day: ignored
        assert_eq!(plan.meal(t.add_days(2), 0).unwrap().servings, Some(6));
        assert!(!plan.dates.contains_key(&t.add_days(4)));

        plan.remove_meal(t.add_days(2), 3); // ignored
        assert_eq!(plan.meal_count(), 3);
        plan.remove_meal(t.add_days(2), 0);
        assert!(
            !plan.dates.contains_key(&t.add_days(2)),
            "empty day dropped"
        );

        plan.drop_before(t.add_days(1));
        assert_eq!(plan.meal_count(), 1);
        assert_eq!(plan.last_date(), Some(t.add_days(9)));

        let mut only_extras = MealPlan::default();
        only_extras.extras.push(Extra::Custom {
            name: "bread".into(),
            quantity: None,
        });
        assert!(!only_extras.is_empty());
    }
}
