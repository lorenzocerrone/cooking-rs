//! Data model for the app. Recipes and ingredients are authored as JSON and
//! embedded at build time; the meal plan is the only mutable, shareable state.

use std::collections::HashMap;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

/// Day-of-week labels, index 0..7 (Monday first).
pub const DAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

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

/// Number of days in a plan (Monday..Sunday).
pub const DAYS_PER_WEEK: usize = 7;

/// The full weekly meal plan. This is the value that gets encoded into the URL
/// and persisted locally.
///
/// Wire format is plain, untagged JSON so every link ever shared keeps decoding:
/// new fields are optional and omitted when empty.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MealPlan {
    /// Exactly [`DAYS_PER_WEEK`] entries after [`MealPlan::normalize`].
    pub days: Vec<DayPlan>,
    /// Extra grocery items not tied to a meal (staples, snacks), folded into the
    /// shopping list unscaled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<Extra>,
}

impl Default for MealPlan {
    fn default() -> Self {
        Self {
            days: vec![DayPlan::default(); DAYS_PER_WEEK],
            extras: Vec::new(),
        }
    }
}

impl MealPlan {
    /// Force `days` to exactly [`DAYS_PER_WEEK`] entries (pad with empty days,
    /// drop surplus). Call after decoding external input so indexing by day is
    /// always in bounds.
    pub fn normalize(&mut self) {
        self.days.resize(DAYS_PER_WEEK, DayPlan::default());
    }

    /// True when no day holds a meal and there are no extras.
    pub fn is_empty(&self) -> bool {
        self.entries().next().is_none() && self.extras.is_empty()
    }

    /// Every chosen meal, Monday..Sunday, in each day's order.
    pub fn entries(&self) -> impl Iterator<Item = &MealEntry> {
        self.days.iter().flat_map(|d| d.meals.iter())
    }

    /// Number of meals in the week.
    pub fn meal_count(&self) -> usize {
        self.entries().count()
    }

    /// The `idx`-th meal of a day, if any.
    pub fn meal(&self, day: usize, idx: usize) -> Option<&MealEntry> {
        self.days.get(day).and_then(|d| d.meals.get(idx))
    }

    /// Append a recipe to a day (default servings).
    pub fn add_meal(&mut self, day: usize, recipe: String) {
        if let Some(d) = self.days.get_mut(day) {
            d.meals.push(MealEntry::new(recipe));
        }
    }

    /// Remove the `idx`-th meal of a day. Out-of-range indices are ignored.
    pub fn remove_meal(&mut self, day: usize, idx: usize) {
        if let Some(d) = self.days.get_mut(day) {
            if idx < d.meals.len() {
                d.meals.remove(idx);
            }
        }
    }

    /// Set (or clear, with `None`) the custom servings of a meal. Does nothing
    /// for an index that holds no meal.
    pub fn set_servings(&mut self, day: usize, idx: usize, servings: Option<u32>) {
        if let Some(entry) = self.days.get_mut(day).and_then(|d| d.meals.get_mut(idx)) {
            entry.servings = servings;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn day_serializes_as_a_list_and_round_trips() {
        let mut plan = MealPlan::default();
        plan.add_meal(0, "oats".into());
        plan.add_meal(0, "dal".into());
        plan.set_servings(0, 1, Some(6));
        let json = serde_json::to_string(&plan).unwrap();
        assert!(
            json.starts_with(r#"{"days":[["oats",{"recipe":"dal","servings":6}],[],"#),
            "{json}"
        );
        let back: MealPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }

    #[test]
    fn legacy_slot_days_decode_into_lists() {
        // Links shared before days became lists: slots as an object, meals as
        // bare ids or {recipe, servings} objects, empty slots as null/missing.
        let json = r#"{"days":[
            {"breakfast":"omelette","lunch":null,"dinner":{"recipe":"dal","servings":4}},
            {"dinner":"soup"},
            {}
        ]}"#;
        let mut plan: MealPlan = serde_json::from_str(json).unwrap();
        plan.normalize();
        let day0: Vec<_> = plan.days[0]
            .meals
            .iter()
            .map(|m| m.recipe.as_str())
            .collect();
        assert_eq!(day0, vec!["omelette", "dal"]);
        assert_eq!(plan.meal(0, 1).unwrap().servings, Some(4));
        assert_eq!(plan.meal(1, 0).unwrap().recipe, "soup");
        assert!(plan.days[2].meals.is_empty());
        assert_eq!(plan.days.len(), DAYS_PER_WEEK);
    }

    #[test]
    fn normalize_pads_and_truncates_to_seven_days() {
        let mut short: MealPlan = serde_json::from_str(r#"{"days":[["omelette"]]}"#).unwrap();
        short.normalize();
        assert_eq!(short.days.len(), DAYS_PER_WEEK);
        assert_eq!(short.meal(0, 0).unwrap().recipe, "omelette");
        assert!(short.meal(6, 0).is_none());

        let mut long = MealPlan::default();
        long.days.push(DayPlan::default());
        long.normalize();
        assert_eq!(long.days.len(), DAYS_PER_WEEK);
    }

    #[test]
    fn extras_are_omitted_when_empty_and_both_kinds_round_trip() {
        let empty = MealPlan::default();
        let json = serde_json::to_string(&empty).unwrap();
        assert!(!json.contains("extras"), "{json}");

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
        let back: MealPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);

        // The pre-`Extra` format (a bare RecipeIngredient list) still decodes.
        let legacy: MealPlan =
            serde_json::from_str(r#"{"days":[],"extras":[{"ingredient":"milk"}]}"#).unwrap();
        assert_eq!(
            legacy.extras,
            vec![Extra::Catalog(RecipeIngredient::new("milk".into(), None))]
        );
    }

    #[test]
    fn add_remove_and_servings_respect_bounds() {
        let mut plan = MealPlan::default();
        assert!(plan.is_empty());
        assert_eq!(plan.meal_count(), 0);

        plan.add_meal(2, "dal".into());
        plan.add_meal(0, "soup".into());
        plan.add_meal(9, "ghost".into()); // out of range: ignored
        assert!(!plan.is_empty());
        let ids: Vec<&str> = plan.entries().map(|e| e.recipe.as_str()).collect();
        assert_eq!(ids, vec!["soup", "dal"]); // Monday first, then Wednesday
        assert_eq!(plan.meal_count(), 2);

        plan.set_servings(2, 0, Some(6));
        plan.set_servings(2, 5, Some(6)); // no such meal: ignored
        assert_eq!(plan.meal(2, 0).unwrap().servings, Some(6));
        plan.set_servings(2, 0, None);
        assert_eq!(plan.meal(2, 0).unwrap().servings, None);

        plan.remove_meal(2, 3); // ignored
        plan.remove_meal(2, 0);
        assert!(plan.days[2].meals.is_empty());
        assert_eq!(plan.meal_count(), 1);

        let mut only_extras = MealPlan::default();
        only_extras.extras.push(Extra::Custom {
            name: "bread".into(),
            quantity: None,
        });
        assert!(!only_extras.is_empty());
    }
}
