//! Pure nutrition logic: compute a recipe's macros from the ingredient catalog.
//!
//! Like [`crate::shopping`], this module takes plain data and returns plain data
//! (no Leptos types), so it is easy to unit-test and reuse.

use crate::models::{
    CatalogIngredient, DayPlan, IngredientCatalog, Macros, MealEntry, MealPlan, Recipe,
    RecipeIngredient,
};

/// Scale every macro field by `factor`.
pub fn scale(m: &Macros, factor: f64) -> Macros {
    Macros {
        calories: m.calories * factor,
        protein: m.protein * factor,
        carbs: m.carbs * factor,
        fat: m.fat * factor,
    }
}

/// Divide whole-recipe macros by the serving count (clamped to at least 1).
pub fn per_serving(m: &Macros, servings: u32) -> Macros {
    scale(m, 1.0 / servings.max(1) as f64)
}

/// Macros for one planned meal, scaled to its chosen servings (the same factor
/// `shopping::aggregate` uses). Returns zero when the recipe is missing or has no
/// macro data, so day/week totals stay additive.
pub fn meal_macros(entry: &MealEntry, recipes: &[Recipe], catalog: &IngredientCatalog) -> Macros {
    let Some(recipe) = recipes.iter().find(|r| r.id == entry.recipe) else {
        return Macros::default();
    };
    let Some(total) = recipe_macros(recipe, catalog) else {
        return Macros::default();
    };
    let target = entry.servings.unwrap_or(recipe.servings);
    let factor = target as f64 / recipe.servings.max(1) as f64;
    scale(&total, factor)
}

/// Summed macros for one day's selected meals.
pub fn day_macros(day: &DayPlan, recipes: &[Recipe], catalog: &IngredientCatalog) -> Macros {
    [&day.breakfast, &day.lunch, &day.dinner]
        .into_iter()
        .flatten()
        .map(|entry| meal_macros(entry, recipes, catalog))
        .fold(Macros::default(), |acc, m| add(&acc, &m))
}

/// Summed macros for the whole week.
pub fn week_macros(plan: &MealPlan, recipes: &[Recipe], catalog: &IngredientCatalog) -> Macros {
    plan.days
        .iter()
        .map(|day| day_macros(day, recipes, catalog))
        .fold(Macros::default(), |acc, m| add(&acc, &m))
}

/// Add two macro sets field-by-field.
fn add(a: &Macros, b: &Macros) -> Macros {
    Macros {
        calories: a.calories + b.calories,
        protein: a.protein + b.protein,
        carbs: a.carbs + b.carbs,
        fat: a.fat + b.fat,
    }
}

/// How much of an ingredient's per-reference macros one `amount` contributes.
/// Catalog macros are per 100 g/ml for weight/volume units, or per 1 for `"unit"`.
fn ingredient_factor(amount: f64, ingredient: &CatalogIngredient) -> f64 {
    if ingredient.unit == "unit" {
        amount
    } else {
        amount / 100.0
    }
}

/// The macros one recipe ingredient contributes, scaled to the amount used.
///
/// Returns `None` when the ingredient has no amount (`"to taste"`), is missing from
/// the catalog, or the catalog entry carries no macro data — so the UI can render a
/// `—` placeholder in those cases.
pub fn ingredient_macros(item: &RecipeIngredient, catalog: &IngredientCatalog) -> Option<Macros> {
    let amount = item.amount?;
    let info = catalog.get(&item.ingredient)?;
    let m = info.macros.as_ref()?;
    Some(scale(m, ingredient_factor(amount, info)))
}

/// Whole-recipe macros.
///
/// If the recipe carries explicit `macros`, those win. Otherwise macros are summed
/// from the ingredients that carry catalog macro data; ingredients without data (or
/// with `amount: None`) contribute nothing. Returns `None` only when *no* macro
/// information is available at all (so the UI can hide the section entirely).
pub fn recipe_macros(recipe: &Recipe, catalog: &IngredientCatalog) -> Option<Macros> {
    if let Some(m) = &recipe.macros {
        return Some(m.clone());
    }

    let mut total = Macros::default();
    let mut any = false;
    for item in &recipe.ingredients {
        if let Some(m) = ingredient_macros(item, catalog) {
            total = add(&total, &m);
            any = true;
        }
    }

    any.then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CatalogIngredient, RecipeIngredient};
    use std::collections::HashMap;

    fn catalog() -> IngredientCatalog {
        let mut map = HashMap::new();
        map.insert(
            "egg".to_string(),
            CatalogIngredient {
                name: "Egg".into(),
                unit: "unit".into(),
                category: "Dairy & Eggs".into(),
                macros: Some(Macros {
                    calories: 70.0,
                    protein: 6.0,
                    carbs: 0.5,
                    fat: 5.0,
                }),
            },
        );
        map.insert(
            "butter".to_string(),
            CatalogIngredient {
                name: "Butter".into(),
                unit: "g".into(),
                category: "Dairy & Eggs".into(),
                macros: Some(Macros {
                    calories: 717.0,
                    protein: 0.85,
                    carbs: 0.06,
                    fat: 81.0,
                }),
            },
        );
        IngredientCatalog(map)
    }

    fn recipe() -> Recipe {
        Recipe {
            id: "omelette".into(),
            title: "Omelette".into(),
            servings: 1,
            ingredients: vec![
                RecipeIngredient {
                    ingredient: "egg".into(),
                    amount: Some(2.0),
                },
                RecipeIngredient {
                    ingredient: "butter".into(),
                    amount: Some(10.0),
                },
            ],
            steps: vec![],
            description: None,
            cuisine: None,
            tags: vec![],
            prep_time: None,
            cook_time: None,
            image: None,
            macros: None,
        }
    }

    #[test]
    fn computes_macros_from_ingredients() {
        let m = recipe_macros(&recipe(), &catalog()).expect("macros computed");
        // 2 eggs (per-unit) + 10g butter (per 100g → 0.1×).
        assert!((m.calories - (2.0 * 70.0 + 0.1 * 717.0)).abs() < 1e-9);
        assert!((m.fat - (2.0 * 5.0 + 0.1 * 81.0)).abs() < 1e-9);
    }

    #[test]
    fn ingredient_macros_scale_to_amount() {
        let cat = catalog();
        // 2 eggs → per-unit macros doubled.
        let eggs = ingredient_macros(
            &RecipeIngredient {
                ingredient: "egg".into(),
                amount: Some(2.0),
            },
            &cat,
        )
        .unwrap();
        assert!((eggs.calories - 2.0 * 70.0).abs() < 1e-9);
        assert!((eggs.fat - 2.0 * 5.0).abs() < 1e-9);

        // 10 g butter → 0.1× the per-100g macros.
        let butter = ingredient_macros(
            &RecipeIngredient {
                ingredient: "butter".into(),
                amount: Some(10.0),
            },
            &cat,
        )
        .unwrap();
        assert!((butter.fat - 0.1 * 81.0).abs() < 1e-9);

        // "to taste" (no amount) contributes nothing.
        assert!(ingredient_macros(
            &RecipeIngredient {
                ingredient: "egg".into(),
                amount: None,
            },
            &cat,
        )
        .is_none());

        // Unknown ingredient → None.
        assert!(ingredient_macros(
            &RecipeIngredient {
                ingredient: "nope".into(),
                amount: Some(5.0),
            },
            &cat,
        )
        .is_none());
    }

    #[test]
    fn explicit_recipe_macros_take_precedence() {
        let mut r = recipe();
        r.macros = Some(Macros {
            calories: 1.0,
            ..Default::default()
        });
        let m = recipe_macros(&r, &catalog()).unwrap();
        assert_eq!(m.calories, 1.0);
    }

    #[test]
    fn none_when_no_macro_data() {
        let mut empty = catalog();
        for v in empty.0.values_mut() {
            v.macros = None;
        }
        assert!(recipe_macros(&recipe(), &empty).is_none());
    }

    #[test]
    fn meal_macros_scale_with_servings() {
        let recipes = vec![recipe()]; // 1 serving
        let cat = catalog();
        let base = recipe_macros(&recipe(), &cat).unwrap();

        // Default servings → unscaled.
        let one = meal_macros(&MealEntry::new("omelette".into()), &recipes, &cat);
        assert!((one.calories - base.calories).abs() < 1e-9);

        // Doubled servings → doubled macros.
        let two = meal_macros(
            &MealEntry {
                recipe: "omelette".into(),
                servings: Some(2),
            },
            &recipes,
            &cat,
        );
        assert!((two.calories - 2.0 * base.calories).abs() < 1e-9);
    }

    #[test]
    fn meal_macros_zero_for_missing_recipe() {
        let m = meal_macros(&MealEntry::new("nope".into()), &[], &catalog());
        assert_eq!(m, Macros::default());
    }

    #[test]
    fn week_macros_sums_days() {
        use crate::models::{MealPlan, Slot};
        let recipes = vec![recipe()];
        let cat = catalog();
        let base = recipe_macros(&recipe(), &cat).unwrap();

        let mut plan = MealPlan::default();
        plan.set_recipe(0, Slot::Breakfast, Some("omelette".into()));
        plan.set_recipe(0, Slot::Dinner, Some("omelette".into()));
        plan.set_recipe(3, Slot::Lunch, Some("omelette".into()));

        let day0 = day_macros(&plan.days[0], &recipes, &cat);
        assert!((day0.calories - 2.0 * base.calories).abs() < 1e-9);

        let week = week_macros(&plan, &recipes, &cat);
        assert!((week.calories - 3.0 * base.calories).abs() < 1e-9);

        // Empty plan → zero.
        let empty = MealPlan::default();
        assert_eq!(week_macros(&empty, &recipes, &cat), Macros::default());
    }
}
