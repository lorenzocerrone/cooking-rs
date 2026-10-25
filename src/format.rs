//! Pure display formatting: turn domain data into user-facing strings.
//!
//! This is the boundary between logic and presentation. It has **no Leptos
//! types** — the component/page layer calls into here so that what something
//! *says* is decoupled from how the logic computes it and how the view renders it.

use crate::models::{IngredientCatalog, Recipe, RecipeIngredient};
use crate::quantity::{resolve_unit, CountSpec, Dimension, Unit};
use crate::shopping::{ShoppingCategory, ShoppingLine, Subtotal};

/// A quantity as authored in a recipe: the number trimmed to at most two
/// decimals, followed by the unit — or the ingredient's piece label for counts
/// ("3 cloves"), or nothing when it has none ("4").
pub fn format_quantity(amount: f64, unit: Unit, count: Option<&CountSpec>) -> String {
    let num = trim_decimals(amount, 2);
    match unit {
        Unit::Piece => match count.and_then(|c| c.label_for(amount)) {
            Some(label) => format!("{num} {label}"),
            None => num,
        },
        other => format!("{num} {}", other.as_str()),
    }
}

/// The amount column for one recipe ingredient, resolved against the catalog.
/// `None` amount renders "to taste".
pub fn format_ingredient_amount(item: &RecipeIngredient, catalog: &IngredientCatalog) -> String {
    format_ingredient_amount_scaled(item, catalog, 1.0)
}

/// [`format_ingredient_amount`] with the amount multiplied by `factor` (the
/// recipe page's servings stepper). "To taste" never scales.
pub fn format_ingredient_amount_scaled(
    item: &RecipeIngredient,
    catalog: &IngredientCatalog,
    factor: f64,
) -> String {
    let Some(amount) = item.amount.map(|a| a * factor) else {
        return "to taste".to_string();
    };
    let info = catalog.get(&item.ingredient);
    let unit = match info {
        Some(i) => resolve_unit(item.unit.as_deref(), i.unit, i.count.as_ref()).unwrap_or(i.unit),
        None => item
            .unit
            .as_deref()
            .and_then(Unit::parse)
            .unwrap_or(Unit::Piece),
    };
    format_quantity(amount, unit, info.and_then(|i| i.count.as_ref()))
}

/// One summed sub-total for the grocery list, in shop-friendly units:
/// g below 1 kg (one decimal under 10 g) then kg; ml/l likewise; pieces rounded
/// *up* to whole items and labelled ("2 cans"). Unlabelled pieces print bare
/// unless the line mixes dimensions, where "pcs" disambiguates.
pub fn format_subtotal(subtotal: &Subtotal, count: Option<&CountSpec>, mixed: bool) -> String {
    match subtotal.dimension {
        Dimension::Mass => scaled(subtotal.amount, "g", "kg"),
        Dimension::Volume => scaled(subtotal.amount, "ml", "l"),
        Dimension::Count => {
            let n = (subtotal.amount - 1e-6).ceil().max(0.0);
            let num = trim_decimals(n, 0);
            match count.and_then(|c| c.label_for(n)) {
                Some(label) => format!("{num} {label}"),
                None if mixed => format!("{num} pcs"),
                None => num,
            }
        }
    }
}

/// The quantity text for a whole shopping line: sub-totals joined with " + ",
/// then "to taste" if any contribution was amount-less, then "(optional)".
pub fn format_line_quantity(line: &ShoppingLine) -> String {
    if !line.custom_quantities.is_empty() {
        return line.custom_quantities.join(" + ");
    }
    let mixed = line.subtotals.len() > 1;
    let mut parts: Vec<String> = line
        .subtotals
        .iter()
        .map(|s| format_subtotal(s, line.count.as_ref(), mixed))
        .collect();
    if line.to_taste {
        parts.push("to taste".to_string());
    }
    let mut out = parts.join(" + ");
    if line.optional {
        out.push_str(" (optional)");
    }
    out
}

/// Render the whole shopping list as plain text for copying/exporting.
pub fn format_text(categories: &[ShoppingCategory]) -> String {
    if categories.is_empty() {
        return "No meals selected yet.".to_string();
    }

    let mut out = String::new();
    for cat in categories {
        out.push_str(&cat.name);
        out.push('\n');
        for line in &cat.lines {
            let qty = format_line_quantity(line);
            if qty.is_empty() {
                out.push_str(&format!("- {}\n", line.name));
            } else {
                out.push_str(&format!("- {}: {qty}\n", line.name));
            }
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Total time (prep + cook) in minutes, if either is set.
pub fn total_time(recipe: &Recipe) -> Option<u32> {
    match (recipe.prep_time, recipe.cook_time) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

/// Total time as a short label ("40 min"), if any time is set.
pub fn format_total_time(recipe: &Recipe) -> Option<String> {
    total_time(recipe).map(|m| format!("{m} min"))
}

/// "10 min prep + 15 min cook", omitting absent parts; empty when neither is set.
pub fn format_timing(recipe: &Recipe) -> String {
    let mut parts = Vec::new();
    if let Some(p) = recipe.prep_time {
        parts.push(format!("{p} min prep"));
    }
    if let Some(c) = recipe.cook_time {
        parts.push(format!("{c} min cook"));
    }
    parts.join(" + ")
}

/// Mass/volume with automatic step-up to the large unit at 1000.
fn scaled(amount: f64, small: &str, big: &str) -> String {
    if amount >= 1000.0 {
        format!("{} {big}", trim_decimals(amount / 1000.0, 2))
    } else if amount < 10.0 {
        format!("{} {small}", trim_decimals(amount, 1))
    } else {
        format!("{} {small}", trim_decimals(amount, 0))
    }
}

/// Format with at most `max` decimals, dropping trailing zeros: `1.50 → "1.5"`,
/// `2.00 → "2"`, `0.333 → "0.33"`.
fn trim_decimals(n: f64, max: usize) -> String {
    let s = format!("{n:.max$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CatalogIngredient;
    use std::collections::HashMap;

    fn clove() -> CountSpec {
        CountSpec {
            label: Some("clove".into()),
            plural: None,
            per_piece: Some(4.0),
        }
    }

    fn catalog() -> IngredientCatalog {
        let mut m = HashMap::new();
        m.insert(
            "garlic".to_string(),
            CatalogIngredient {
                name: "Garlic".into(),
                unit: Unit::G,
                category: "Produce".into(),
                count: Some(clove()),
            },
        );
        m.insert(
            "egg".to_string(),
            CatalogIngredient {
                name: "Egg".into(),
                unit: Unit::Piece,
                category: "Dairy & Eggs".into(),
                count: None,
            },
        );
        m.insert(
            "oil".to_string(),
            CatalogIngredient {
                name: "Olive oil".into(),
                unit: Unit::Ml,
                category: "Spices & Oils".into(),
                count: None,
            },
        );
        IngredientCatalog(m)
    }

    fn sub(dimension: Dimension, amount: f64) -> Subtotal {
        Subtotal { dimension, amount }
    }

    fn line(subtotals: Vec<Subtotal>, count: Option<CountSpec>) -> ShoppingLine {
        ShoppingLine {
            key: "k".into(),
            name: "Thing".into(),
            count,
            subtotals,
            to_taste: false,
            optional: false,
            sources: vec![],
            custom_quantities: vec![],
        }
    }

    #[test]
    fn quantity_formatting_follows_authored_unit() {
        assert_eq!(format_quantity(200.0, Unit::G, None), "200 g");
        assert_eq!(format_quantity(0.5, Unit::Cup, None), "0.5 cup");
        assert_eq!(format_quantity(2.0, Unit::Tbsp, None), "2 tbsp");
        assert_eq!(format_quantity(1.333, Unit::Kg, None), "1.33 kg");
        assert_eq!(format_quantity(4.0, Unit::Piece, None), "4");
        assert_eq!(format_quantity(1.0, Unit::Piece, Some(&clove())), "1 clove");
        assert_eq!(
            format_quantity(3.0, Unit::Piece, Some(&clove())),
            "3 cloves"
        );
    }

    #[test]
    fn ingredient_amount_resolves_against_catalog() {
        let cat = catalog();
        let item = |key: &str, amount: Option<f64>, unit: Option<&str>| RecipeIngredient {
            ingredient: key.into(),
            amount,
            unit: unit.map(Into::into),
            note: None,
            optional: false,
        };
        assert_eq!(
            format_ingredient_amount(&item("garlic", Some(3.0), Some("cloves")), &cat),
            "3 cloves"
        );
        assert_eq!(
            format_ingredient_amount(&item("garlic", Some(10.0), None), &cat),
            "10 g"
        );
        assert_eq!(
            format_ingredient_amount(&item("oil", Some(2.0), Some("tbsp")), &cat),
            "2 tbsp"
        );
        assert_eq!(
            format_ingredient_amount(&item("egg", Some(2.0), None), &cat),
            "2"
        );
        assert_eq!(
            format_ingredient_amount(&item("egg", None, None), &cat),
            "to taste"
        );
        // Unknown ingredient / unit: never panics, falls back to something readable.
        assert_eq!(
            format_ingredient_amount(&item("mystery", Some(2.0), Some("kg")), &cat),
            "2 kg"
        );
        assert_eq!(
            format_ingredient_amount(&item("garlic", Some(1.0), Some("bogus")), &cat),
            "1 g"
        );
    }

    #[test]
    fn subtotal_rounding_table() {
        let cases = [
            (sub(Dimension::Mass, 2.5), None, false, "2.5 g"),
            (sub(Dimension::Mass, 2.54), None, false, "2.5 g"),
            (sub(Dimension::Mass, 249.6), None, false, "250 g"),
            (sub(Dimension::Mass, 999.0), None, false, "999 g"),
            (sub(Dimension::Mass, 1000.0), None, false, "1 kg"),
            (sub(Dimension::Mass, 1500.0), None, false, "1.5 kg"),
            (sub(Dimension::Mass, 1234.0), None, false, "1.23 kg"),
            (sub(Dimension::Volume, 500.0), None, false, "500 ml"),
            (sub(Dimension::Volume, 1200.0), None, false, "1.2 l"),
            (sub(Dimension::Count, 1.3), None, false, "2"),
            (sub(Dimension::Count, 2.0000001), None, false, "2"),
            (sub(Dimension::Count, 2.0), None, true, "2 pcs"),
            (sub(Dimension::Count, 1.0), Some(clove()), false, "1 clove"),
            (sub(Dimension::Count, 2.2), Some(clove()), false, "3 cloves"),
        ];
        for (s, count, mixed, want) in cases {
            assert_eq!(format_subtotal(&s, count.as_ref(), mixed), want, "{s:?}");
        }
    }

    #[test]
    fn line_quantity_joins_subtotals_to_taste_and_optional() {
        let l = line(
            vec![sub(Dimension::Mass, 100.0), sub(Dimension::Count, 2.0)],
            None,
        );
        assert_eq!(format_line_quantity(&l), "100 g + 2 pcs");

        let mut l = line(vec![sub(Dimension::Mass, 5.0)], None);
        l.to_taste = true;
        assert_eq!(format_line_quantity(&l), "5 g + to taste");

        let mut l = line(vec![], None);
        l.to_taste = true;
        l.optional = true;
        assert_eq!(format_line_quantity(&l), "to taste (optional)");
    }

    #[test]
    fn custom_lines_show_their_text_as_typed() {
        let mut l = line(vec![], None);
        l.custom_quantities = vec!["1 loaf".into(), "2 rolls".into()];
        assert_eq!(format_line_quantity(&l), "1 loaf + 2 rolls");
        let bare = line(vec![], None);
        assert_eq!(format_line_quantity(&bare), "");
    }

    #[test]
    fn scaled_amounts_follow_the_servings_factor() {
        let cat = catalog();
        let garlic = RecipeIngredient {
            ingredient: "garlic".into(),
            amount: Some(3.0),
            unit: Some("cloves".into()),
            note: None,
            optional: false,
        };
        assert_eq!(
            format_ingredient_amount_scaled(&garlic, &cat, 1.5),
            "4.5 cloves"
        );
        assert_eq!(
            format_ingredient_amount_scaled(&garlic, &cat, 1.0 / 3.0),
            "1 clove"
        );
        let salt = RecipeIngredient::new("salt".into(), None);
        assert_eq!(
            format_ingredient_amount_scaled(&salt, &cat, 2.0),
            "to taste"
        );
    }

    #[test]
    fn text_export_lists_categories_and_lines() {
        assert_eq!(format_text(&[]), "No meals selected yet.");
        let mut bare = line(vec![], None);
        bare.name = "Candles".into();
        let cats = vec![
            ShoppingCategory {
                name: "Produce".into(),
                lines: vec![line(vec![sub(Dimension::Mass, 1500.0)], None)],
            },
            ShoppingCategory {
                name: "Other".into(),
                lines: vec![bare],
            },
        ];
        assert_eq!(
            format_text(&cats),
            "Produce\n- Thing: 1.5 kg\n\nOther\n- Candles"
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
            author: None,
        };
        assert_eq!(total_time(&r), Some(25));
        assert_eq!(format_total_time(&r), Some("25 min".into()));
        assert_eq!(format_timing(&r), "10 min prep + 15 min cook");
        r.cook_time = None;
        assert_eq!(format_timing(&r), "10 min prep");
        r.prep_time = None;
        assert_eq!(total_time(&r), None);
        assert_eq!(format_total_time(&r), None);
        assert_eq!(format_timing(&r), "");
    }
}
