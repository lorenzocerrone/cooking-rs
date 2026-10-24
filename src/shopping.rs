//! Shopping-list aggregation: turn a meal plan into summed quantities grouped by
//! ingredient category. Quantities sum exactly because every catalog ingredient
//! has a single canonical unit (no conversion needed).
//!
//! This is pure logic — it returns data structures. Turning those into display
//! strings lives in [`crate::format`].

use std::collections::BTreeMap;

use crate::models::{IngredientCatalog, MealPlan, Recipe, RecipeIngredient};

/// Preferred category ordering for the shopping list (and the ingredients page).
/// Unknown categories are appended afterwards, sorted alphabetically.
pub const CATEGORY_ORDER: &[&str] = &[
    "Produce",
    "Meat",
    "Dairy & Eggs",
    "Pasta & Grains",
    "Bakery",
    "Legumes",
    "Pantry",
    "Spices & Oils",
];

/// Sort key for a category name following [`CATEGORY_ORDER`] (known first).
pub fn category_sort_key(name: &str) -> usize {
    CATEGORY_ORDER
        .iter()
        .position(|known| *known == name)
        .unwrap_or(CATEGORY_ORDER.len())
}

/// One summed line in the shopping list.
#[derive(Clone, Debug, PartialEq)]
pub struct ShoppingItem {
    pub name: String,
    pub unit: String,
    /// Summed numeric amount, if any recipe specified one.
    pub total: Option<f64>,
    /// True if at least one occurrence was amount-less ("to taste").
    pub to_taste: bool,
}

/// A category section of the shopping list.
#[derive(Clone, Debug, PartialEq)]
pub struct ShoppingCategory {
    pub name: String,
    pub items: Vec<ShoppingItem>,
}

/// Aggregate the selected meals into a category-grouped, summed shopping list.
pub fn aggregate(
    plan: &MealPlan,
    recipes: &[Recipe],
    catalog: &IngredientCatalog,
) -> Vec<ShoppingCategory> {
    // Accumulate per ingredient key, preserving catalog metadata.
    struct Acc {
        name: String,
        unit: String,
        category: String,
        total: Option<f64>,
        to_taste: bool,
    }
    let mut totals: BTreeMap<String, Acc> = BTreeMap::new();

    let mut add = |item: &RecipeIngredient, factor: f64| {
        let Some(info) = catalog.get(&item.ingredient) else {
            return;
        };
        let acc = totals
            .entry(item.ingredient.clone())
            .or_insert_with(|| Acc {
                name: info.name.clone(),
                unit: info.unit.clone(),
                category: info.category.clone(),
                total: None,
                to_taste: false,
            });
        match item.amount {
            Some(a) => *acc.total.get_or_insert(0.0) += a * factor,
            None => acc.to_taste = true,
        }
    };

    for entry in plan.entries() {
        let Some(recipe) = recipes.iter().find(|r| r.id == entry.recipe) else {
            continue;
        };
        // Scale ingredient amounts to the chosen servings (default: no scaling).
        let target = entry.servings.unwrap_or(recipe.servings);
        let factor = target as f64 / recipe.servings.max(1) as f64;
        for item in &recipe.ingredients {
            add(item, factor);
        }
    }
    // Extras are plain grocery items: never scaled.
    for item in &plan.extras {
        add(item, 1.0);
    }

    // Group by category.
    let mut by_category: BTreeMap<String, Vec<ShoppingItem>> = BTreeMap::new();
    for acc in totals.into_values() {
        by_category
            .entry(acc.category)
            .or_default()
            .push(ShoppingItem {
                name: acc.name,
                unit: acc.unit,
                total: acc.total,
                to_taste: acc.to_taste,
            });
    }

    // Order categories: known order first, then unknown alphabetically.
    let mut categories: Vec<ShoppingCategory> = by_category
        .into_iter()
        .map(|(name, mut items)| {
            items.sort_by(|a, b| a.name.cmp(&b.name));
            ShoppingCategory { name, items }
        })
        .collect();

    categories.sort_by_key(|c| category_sort_key(&c.name));

    categories
}
