//! Pure display formatting: turn domain data into user-facing strings.
//!
//! This is the boundary between logic and presentation. It has **no Leptos
//! types** — the component/page layer calls into here so that what something
//! *says* is decoupled from how the logic computes it and how the view renders it.

use crate::models::{IngredientCatalog, Macros, Recipe, RecipeIngredient};
use crate::shopping::{ShoppingCategory, ShoppingItem};

/// Format a numeric amount + unit for display. `None` amount renders "to taste".
/// The pseudo-unit `"unit"` prints just the count (e.g. "4" rather than "4 unit").
pub fn format_amount(amount: Option<f64>, unit: &str) -> String {
    match amount {
        None => "to taste".to_string(),
        Some(n) => {
            let num = trim_number(n);
            if unit == "unit" {
                num
            } else {
                format!("{num} {unit}")
            }
        }
    }
}

/// Condensed one-line ingredient preview for cards: the first `max` ingredient
/// names joined by ", ", with a "+N more" suffix when there are extras.
pub fn ingredient_summary(
    items: &[RecipeIngredient],
    catalog: &IngredientCatalog,
    max: usize,
) -> String {
    let names: Vec<&str> = items
        .iter()
        .map(|i| {
            catalog
                .get(&i.ingredient)
                .map(|info| info.name.as_str())
                .unwrap_or("?")
        })
        .collect();
    let shown = names
        .iter()
        .take(max)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    let extra = names.len().saturating_sub(max);
    if extra > 0 {
        format!("{shown} +{extra} more")
    } else {
        shown
    }
}

/// Render a shopping item's quantity, accounting for mixed numeric + "to taste".
pub fn format_item_quantity(item: &ShoppingItem) -> String {
    match (item.total, item.to_taste) {
        (None, _) => "to taste".to_string(),
        (Some(_), false) => format_amount(item.total, &item.unit),
        (Some(_), true) => format!("{} + to taste", format_amount(item.total, &item.unit)),
    }
}

/// Render the whole shopping list as plain text for the export textarea.
pub fn format_text(categories: &[ShoppingCategory]) -> String {
    if categories.is_empty() {
        return "No meals selected yet.".to_string();
    }

    let mut out = String::new();
    for cat in categories {
        out.push_str(&cat.name);
        out.push('\n');
        for item in &cat.items {
            out.push_str(&format!(
                "- {}: {}\n",
                item.name,
                format_item_quantity(item)
            ));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Calories formatted with thousands separators, e.g. `1850.0 → "1,850 kcal"`.
pub fn format_kcal(calories: f64) -> String {
    format!("{} kcal", group_thousands(round(calories)))
}

/// Compact protein/carb/fat line, e.g. `"P31 · C75 · F28 g"`.
pub fn format_macros(m: &Macros) -> String {
    format!(
        "P{} · C{} · F{} g",
        round(m.protein),
        round(m.carbs),
        round(m.fat)
    )
}

/// Total time (prep + cook) in minutes, if either is set.
pub fn total_time(recipe: &Recipe) -> Option<u32> {
    match (recipe.prep_time, recipe.cook_time) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

/// A " · 10 min prep · 15 min cook" suffix, omitting absent parts.
pub fn format_timing(recipe: &Recipe) -> String {
    let mut parts = Vec::new();
    if let Some(p) = recipe.prep_time {
        parts.push(format!("{p} min prep"));
    }
    if let Some(c) = recipe.cook_time {
        parts.push(format!("{c} min cook"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" · {}", parts.join(" · "))
    }
}

/// Round to a whole number for display.
pub fn round(n: f64) -> i64 {
    n.round() as i64
}

/// Resolve a (possibly relative) asset path against the app base path so it works
/// under a subpath deploy (e.g. `/cooking-rs/`). Absolute URLs are returned as-is.
pub fn asset_url(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with('/') {
        path.to_string()
    } else {
        format!("{base}/{path}")
    }
}

/// Drop a trailing `.0` so `200.0` prints as `200`, while `0.5` stays `0.5`.
fn trim_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Insert thousands separators into an integer: `1850 → "1,850"`.
fn group_thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_formatting() {
        assert_eq!(format_amount(Some(200.0), "g"), "200 g");
        assert_eq!(format_amount(Some(0.5), "g"), "0.5 g");
        assert_eq!(format_amount(Some(4.0), "unit"), "4");
        assert_eq!(format_amount(None, "g"), "to taste");
    }

    #[test]
    fn kcal_grouping() {
        assert_eq!(format_kcal(0.0), "0 kcal");
        assert_eq!(format_kcal(950.4), "950 kcal");
        assert_eq!(format_kcal(1850.0), "1,850 kcal");
        assert_eq!(format_kcal(9200.0), "9,200 kcal");
        assert_eq!(format_kcal(1234567.0), "1,234,567 kcal");
    }

    #[test]
    fn macros_line() {
        let m = Macros {
            calories: 620.0,
            protein: 31.2,
            carbs: 74.8,
            fat: 28.0,
        };
        assert_eq!(format_macros(&m), "P31 · C75 · F28 g");
    }

    #[test]
    fn asset_url_resolves_relative_and_absolute() {
        // Relative paths get the base prefix (works on root and subpath deploys).
        assert_eq!(asset_url("", "images/x.svg"), "/images/x.svg");
        assert_eq!(
            asset_url("/cooking-rs", "images/x.svg"),
            "/cooking-rs/images/x.svg"
        );
        // Already-absolute paths/URLs are left untouched.
        assert_eq!(asset_url("/cooking-rs", "/images/x.svg"), "/images/x.svg");
        assert_eq!(
            asset_url("/cooking-rs", "https://ex.com/x.jpg"),
            "https://ex.com/x.jpg"
        );
    }

    #[test]
    fn ingredient_summary_condenses_with_overflow() {
        use crate::models::CatalogIngredient;
        use std::collections::HashMap;

        let mut map = HashMap::new();
        for key in ["a", "b", "c", "d", "e", "f"] {
            map.insert(
                key.to_string(),
                CatalogIngredient {
                    name: key.to_uppercase(),
                    unit: "g".into(),
                    category: "Test".into(),
                    macros: None,
                },
            );
        }
        let catalog = IngredientCatalog(map);
        let items = |keys: &[&str]| {
            keys.iter()
                .map(|k| RecipeIngredient {
                    ingredient: (*k).into(),
                    amount: None,
                })
                .collect::<Vec<_>>()
        };

        // More than `max` ingredients → first `max` names + "+N more".
        assert_eq!(
            ingredient_summary(&items(&["a", "b", "c", "d", "e", "f"]), &catalog, 3),
            "A, B, C +3 more"
        );
        // At or below `max` → no suffix.
        assert_eq!(ingredient_summary(&items(&["a", "b"]), &catalog, 3), "A, B");
        // Unknown keys render as "?".
        assert_eq!(
            ingredient_summary(&items(&["a", "zzz"]), &catalog, 3),
            "A, ?"
        );
    }

    #[test]
    fn timing() {
        let mut r = Recipe {
            id: "x".into(),
            title: "X".into(),
            servings: 1,
            ingredients: vec![],
            steps: vec![],
            description: None,
            cuisine: None,
            tags: vec![],
            prep_time: Some(10),
            cook_time: Some(15),
            image: None,
            macros: None,
        };
        assert_eq!(total_time(&r), Some(25));
        assert_eq!(format_timing(&r), " · 10 min prep · 15 min cook");
        r.cook_time = None;
        assert_eq!(format_timing(&r), " · 10 min prep");
        r.prep_time = None;
        assert_eq!(total_time(&r), None);
        assert_eq!(format_timing(&r), "");
    }
}
