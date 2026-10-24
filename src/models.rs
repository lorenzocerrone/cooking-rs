//! Data model for the app. Recipes and ingredients are authored as JSON and
//! embedded at build time; the meal plan is the only mutable, shareable state.

use std::collections::HashMap;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A single recipe, as authored in `recipes/*.json`. Ingredients reference the
/// central catalog ([`IngredientCatalog`]) by key rather than embedding their own
/// names/units, so amounts can be summed across recipes.
///
/// Everything beyond the original core fields is optional (`#[serde(default)]`) so
/// older recipe files keep parsing unchanged.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
/// (rendered but not numerically summed).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RecipeIngredient {
    /// Key into the [`IngredientCatalog`].
    pub ingredient: String,
    #[serde(default)]
    pub amount: Option<f64>,
}

/// One entry in the ingredient catalog (`ingredients.json`). The map key is the
/// lookup id used by [`RecipeIngredient::ingredient`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CatalogIngredient {
    pub name: String,
    /// The single canonical unit for this ingredient (e.g. "g", "ml", "unit").
    pub unit: String,
    /// Shopping-aisle grouping (e.g. "Produce", "Dairy & Eggs").
    pub category: String,
}

/// The full ingredient catalog: `key -> entry`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct IngredientCatalog(pub HashMap<String, CatalogIngredient>);

impl IngredientCatalog {
    pub fn get(&self, key: &str) -> Option<&CatalogIngredient> {
        self.0.get(key)
    }
}

/// The three meal slots in a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Breakfast,
    Lunch,
    Dinner,
}

impl Slot {
    pub const ALL: [Slot; 3] = [Slot::Breakfast, Slot::Lunch, Slot::Dinner];

    pub fn label(self) -> &'static str {
        match self {
            Slot::Breakfast => "Breakfast",
            Slot::Lunch => "Lunch",
            Slot::Dinner => "Dinner",
        }
    }
}

/// Day-of-week labels, index 0..7.
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

/// One day's chosen meals. `None` means the slot is empty.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DayPlan {
    pub breakfast: Option<MealEntry>,
    pub lunch: Option<MealEntry>,
    pub dinner: Option<MealEntry>,
}

impl DayPlan {
    fn slot_mut(&mut self, slot: Slot) -> &mut Option<MealEntry> {
        match slot {
            Slot::Breakfast => &mut self.breakfast,
            Slot::Lunch => &mut self.lunch,
            Slot::Dinner => &mut self.dinner,
        }
    }

    pub fn slot(&self, slot: Slot) -> &Option<MealEntry> {
        match slot {
            Slot::Breakfast => &self.breakfast,
            Slot::Lunch => &self.lunch,
            Slot::Dinner => &self.dinner,
        }
    }
}

/// Number of days in a plan (Monday..Sunday).
pub const DAYS_PER_WEEK: usize = 7;

/// The full weekly meal plan. This is the value that gets encoded into the URL
/// and persisted locally.
///
/// Wire format is plain, untagged JSON so every link ever shared keeps decoding:
/// new fields are optional and omitted when empty, so a plan without extras
/// serializes byte-for-byte as it did before `extras` existed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MealPlan {
    /// Exactly [`DAYS_PER_WEEK`] entries after [`MealPlan::normalize`].
    pub days: Vec<DayPlan>,
    /// Extra grocery items not tied to a meal (staples, snacks). They reference
    /// the ingredient catalog exactly like recipe ingredients and are folded into
    /// the shopping list unscaled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extras: Vec<RecipeIngredient>,
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

    /// True when no slot holds a recipe and there are no extras.
    pub fn is_empty(&self) -> bool {
        self.entries().next().is_none() && self.extras.is_empty()
    }

    /// Every chosen meal, in Monday..Sunday × breakfast..dinner order.
    pub fn entries(&self) -> impl Iterator<Item = &MealEntry> {
        self.days
            .iter()
            .flat_map(|d| Slot::ALL.iter().filter_map(move |&s| d.slot(s).as_ref()))
    }

    /// Selected recipe id for a given day/slot, if any.
    pub fn recipe_id(&self, day: usize, slot: Slot) -> Option<String> {
        self.days
            .get(day)
            .and_then(|d| d.slot(slot).as_ref().map(|e| e.recipe.clone()))
    }

    /// Custom target servings for a given day/slot, if set.
    pub fn servings(&self, day: usize, slot: Slot) -> Option<u32> {
        self.days
            .get(day)
            .and_then(|d| d.slot(slot).as_ref().and_then(|e| e.servings))
    }

    /// Set (or clear, with `None`) the recipe for a given day/slot. Changing the
    /// recipe resets any custom servings back to the recipe default.
    pub fn set_recipe(&mut self, day: usize, slot: Slot, recipe: Option<String>) {
        if let Some(d) = self.days.get_mut(day) {
            *d.slot_mut(slot) = recipe.map(MealEntry::new);
        }
    }

    /// Set (or clear, with `None`) the custom servings for a given day/slot. Does
    /// nothing if the slot is empty.
    pub fn set_servings(&mut self, day: usize, slot: Slot, servings: Option<u32>) {
        if let Some(d) = self.days.get_mut(day) {
            if let Some(entry) = d.slot_mut(slot) {
                entry.servings = servings;
            }
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
    fn legacy_id_only_plan_still_decodes() {
        // Old links serialized slots as bare recipe-id strings.
        let json = r#"{"days":[{"breakfast":"omelette","lunch":null,"dinner":null}]}"#;
        let plan: MealPlan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.recipe_id(0, Slot::Breakfast), Some("omelette".into()));
        assert_eq!(plan.servings(0, Slot::Breakfast), None);
    }

    #[test]
    fn normalize_pads_and_truncates_to_seven_days() {
        let mut short: MealPlan =
            serde_json::from_str(r#"{"days":[{"breakfast":"omelette"}]}"#).unwrap();
        short.normalize();
        assert_eq!(short.days.len(), DAYS_PER_WEEK);
        assert_eq!(short.recipe_id(0, Slot::Breakfast), Some("omelette".into()));
        assert_eq!(short.recipe_id(6, Slot::Dinner), None);

        let mut long = MealPlan::default();
        long.days.push(DayPlan::default());
        long.normalize();
        assert_eq!(long.days.len(), DAYS_PER_WEEK);
    }

    #[test]
    fn extras_are_omitted_when_empty_and_round_trip() {
        // Byte-identical to the pre-extras format, so old links don't change.
        let empty = MealPlan::default();
        let json = serde_json::to_string(&empty).unwrap();
        assert!(!json.contains("extras"), "{json}");

        let mut plan = MealPlan::default();
        plan.extras.push(RecipeIngredient {
            ingredient: "milk".into(),
            amount: Some(1000.0),
        });
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("\"extras\""));
        let back: MealPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }

    #[test]
    fn is_empty_and_entries_cover_slots_and_extras() {
        let mut plan = MealPlan::default();
        assert!(plan.is_empty());
        assert_eq!(plan.entries().count(), 0);

        plan.set_recipe(2, Slot::Lunch, Some("dal".into()));
        plan.set_recipe(0, Slot::Dinner, Some("soup".into()));
        assert!(!plan.is_empty());
        let ids: Vec<&str> = plan.entries().map(|e| e.recipe.as_str()).collect();
        assert_eq!(ids, vec!["soup", "dal"]); // plan order: Mon dinner, then Wed lunch

        let mut only_extras = MealPlan::default();
        only_extras.extras.push(RecipeIngredient {
            ingredient: "bread".into(),
            amount: None,
        });
        assert!(!only_extras.is_empty());
    }
}
