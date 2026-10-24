//! Shopping-list aggregation: turn a meal plan into summed quantities grouped by
//! ingredient category. Every contribution is converted into the ingredient's
//! shopping unit (see [`crate::quantity`]); anything that cannot be converted is
//! kept as a separate sub-total on the same line rather than dropped.
//!
//! This is pure logic — it returns data structures. Turning those into display
//! strings lives in [`crate::format`].

use std::collections::BTreeMap;

use crate::models::{Extra, IngredientCatalog, MealPlan, Recipe, RecipeIngredient};
use crate::quantity::{resolve_unit, to_shopping, Converted, CountSpec, Dimension, Quantity};

/// Preferred category ordering for the shopping list.
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

/// Aisle for hand-typed extras that are not in the catalog. Always sorts last.
pub const OTHER_CATEGORY: &str = "Other";

/// Sort key for a category name following [`CATEGORY_ORDER`] (known first,
/// unknown after, [`OTHER_CATEGORY`] at the very end).
pub fn category_sort_key(name: &str) -> usize {
    if name == OTHER_CATEGORY {
        return usize::MAX;
    }
    CATEGORY_ORDER
        .iter()
        .position(|known| *known == name)
        .unwrap_or(CATEGORY_ORDER.len())
}

/// Line key for a custom (non-catalog) extra: stable per name, case-insensitive.
pub fn custom_key(name: &str) -> String {
    format!("custom:{}", name.trim().to_lowercase())
}

/// A summed amount in one dimension, expressed in that dimension's base unit
/// (g, ml or pieces).
#[derive(Clone, Debug, PartialEq)]
pub struct Subtotal {
    pub dimension: Dimension,
    pub amount: f64,
}

/// One line in the shopping list: one catalog ingredient.
#[derive(Clone, Debug, PartialEq)]
pub struct ShoppingLine {
    /// Catalog key — stable identity for check-off state.
    pub key: String,
    pub name: String,
    /// The ingredient's piece spec, for labelling counts ("3 cloves").
    pub count: Option<CountSpec>,
    /// Summed amounts. The shopping unit's dimension comes first; any
    /// unconvertible contributions follow in Mass, Volume, Count order.
    pub subtotals: Vec<Subtotal>,
    /// True if at least one contribution was amount-less ("to taste").
    pub to_taste: bool,
    /// True only if every contribution was marked optional.
    pub optional: bool,
    /// Recipe ids that contributed, de-duplicated, in plan order.
    pub sources: Vec<String>,
    /// Free-text quantities of custom extras ("1 loaf"), shown verbatim. Only
    /// set on `custom:*` lines, which have no `subtotals`.
    pub custom_quantities: Vec<String>,
}

/// A category section of the shopping list.
#[derive(Clone, Debug, PartialEq)]
pub struct ShoppingCategory {
    pub name: String,
    pub lines: Vec<ShoppingLine>,
}

/// Aggregate the selected meals (and the plan's extras) into a category-grouped,
/// summed shopping list. Deterministic: lines sort by name, then key.
pub fn aggregate(
    plan: &MealPlan,
    recipes: &[Recipe],
    catalog: &IngredientCatalog,
) -> Vec<ShoppingCategory> {
    struct Acc {
        name: String,
        category: String,
        primary: Dimension,
        count: Option<CountSpec>,
        totals: BTreeMap<Dimension, f64>,
        to_taste: bool,
        optional: bool,
        sources: Vec<String>,
    }
    let mut lines: BTreeMap<String, Acc> = BTreeMap::new();

    let mut add = |item: &RecipeIngredient, factor: f64, source: Option<&str>| {
        let Some(info) = catalog.get(&item.ingredient) else {
            return;
        };
        let acc = lines.entry(item.ingredient.clone()).or_insert_with(|| Acc {
            name: info.name.clone(),
            category: info.category.clone(),
            primary: info.unit.dimension(),
            count: info.count.clone(),
            totals: BTreeMap::new(),
            to_taste: false,
            optional: true,
            sources: Vec::new(),
        });
        acc.optional &= item.optional;
        if let Some(src) = source {
            if acc.sources.last().is_none_or(|last| last != src)
                && !acc.sources.iter().any(|s| s == src)
            {
                acc.sources.push(src.to_string());
            }
        }
        match item.amount {
            None => acc.to_taste = true,
            Some(amount) => {
                // Validation guarantees embedded data resolves; the fallback means
                // a bad unit can never make food disappear from the list.
                let unit = resolve_unit(item.unit.as_deref(), info.unit, info.count.as_ref())
                    .unwrap_or(info.unit);
                let (dimension, value) = match to_shopping(
                    Quantity::new(amount * factor, unit),
                    info.unit,
                    info.count.as_ref(),
                ) {
                    Converted::Primary(v) => (acc.primary, v),
                    Converted::Other(d, v) => (d, v),
                };
                *acc.totals.entry(dimension).or_insert(0.0) += value;
            }
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
            add(item, factor, Some(&recipe.id));
        }
    }
    // Extras are plain grocery items: never scaled, no recipe source.
    for extra in &plan.extras {
        if let Extra::Catalog(item) = extra {
            add(item, 1.0, None);
        }
    }

    // Group by category.
    let mut by_category: BTreeMap<String, Vec<ShoppingLine>> = BTreeMap::new();
    for (key, acc) in lines {
        let mut subtotals: Vec<Subtotal> = acc
            .totals
            .into_iter()
            .map(|(dimension, amount)| Subtotal { dimension, amount })
            .collect();
        // Primary dimension first; the BTreeMap already ordered the rest.
        subtotals.sort_by_key(|s| s.dimension != acc.primary);
        by_category
            .entry(acc.category)
            .or_default()
            .push(ShoppingLine {
                key,
                name: acc.name,
                count: acc.count,
                subtotals,
                to_taste: acc.to_taste,
                optional: acc.optional,
                sources: acc.sources,
                custom_quantities: Vec::new(),
            });
    }

    // Custom extras: one line per distinct name in the "Other" aisle, quantities
    // kept as typed (and merged when the same name was added twice).
    let mut customs: BTreeMap<String, ShoppingLine> = BTreeMap::new();
    for extra in &plan.extras {
        let Extra::Custom { name, quantity } = extra else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let line = customs
            .entry(custom_key(name))
            .or_insert_with_key(|key| ShoppingLine {
                key: key.clone(),
                name: name.to_string(),
                count: None,
                subtotals: Vec::new(),
                to_taste: false,
                optional: false,
                sources: Vec::new(),
                custom_quantities: Vec::new(),
            });
        if let Some(q) = quantity.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            line.custom_quantities.push(q.to_string());
        }
    }
    if !customs.is_empty() {
        by_category
            .entry(OTHER_CATEGORY.to_string())
            .or_default()
            .extend(customs.into_values());
    }

    // Order categories: known order first, then unknown alphabetically.
    let mut categories: Vec<ShoppingCategory> = by_category
        .into_iter()
        .map(|(name, mut lines)| {
            lines.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.key.cmp(&b.key)));
            ShoppingCategory { name, lines }
        })
        .collect();
    categories.sort_by_key(|c| category_sort_key(&c.name));
    categories
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CatalogIngredient;
    use crate::quantity::Unit;
    use std::collections::HashMap;

    fn entry(
        name: &str,
        unit: Unit,
        category: &str,
        count: Option<CountSpec>,
    ) -> CatalogIngredient {
        CatalogIngredient {
            name: name.into(),
            unit,
            category: category.into(),
            count,
        }
    }

    fn pieces(label: Option<&str>, per_piece: f64) -> Option<CountSpec> {
        Some(CountSpec {
            label: label.map(Into::into),
            plural: None,
            per_piece: Some(per_piece),
        })
    }

    fn catalog() -> IngredientCatalog {
        let mut m = HashMap::new();
        m.insert(
            "onion".into(),
            entry("Onion", Unit::G, "Produce", pieces(None, 150.0)),
        );
        m.insert(
            "garlic".into(),
            entry("Garlic", Unit::G, "Produce", pieces(Some("clove"), 4.0)),
        );
        m.insert(
            "olive_oil".into(),
            entry("Olive oil", Unit::Ml, "Spices & Oils", None),
        );
        m.insert(
            "tomatoes_can".into(),
            entry(
                "Chopped tomatoes",
                Unit::G,
                "Pantry",
                pieces(Some("can"), 400.0),
            ),
        );
        m.insert(
            "egg".into(),
            entry("Egg", Unit::Piece, "Dairy & Eggs", None),
        );
        m.insert("lemon".into(), entry("Lemon", Unit::G, "Produce", None)); // no piece conversion
        m.insert("salt".into(), entry("Salt", Unit::G, "Spices & Oils", None));
        IngredientCatalog(m)
    }

    fn ing(key: &str, amount: Option<f64>, unit: Option<&str>) -> RecipeIngredient {
        RecipeIngredient {
            ingredient: key.into(),
            amount,
            unit: unit.map(Into::into),
            note: None,
            optional: false,
        }
    }

    fn recipe(id: &str, servings: u32, ingredients: Vec<RecipeIngredient>) -> Recipe {
        Recipe {
            id: id.into(),
            title: id.to_uppercase(),
            servings,
            ingredients,
            steps: vec!["cook".into()],
            description: None,
            cuisine: None,
            tags: vec![],
            prep_time: None,
            cook_time: None,
            image: None,
        }
    }

    fn line<'a>(cats: &'a [ShoppingCategory], key: &str) -> &'a ShoppingLine {
        cats.iter()
            .flat_map(|c| c.lines.iter())
            .find(|l| l.key == key)
            .unwrap_or_else(|| panic!("no line for {key}"))
    }

    fn sub(line: &ShoppingLine) -> Vec<(Dimension, f64)> {
        line.subtotals
            .iter()
            .map(|s| (s.dimension, s.amount))
            .collect()
    }

    #[test]
    fn converts_the_four_canonical_inputs_into_shopping_units() {
        let recipes = vec![
            recipe(
                "dal",
                2,
                vec![
                    ing("onion", Some(1.0), Some("pcs")),
                    ing("garlic", Some(3.0), Some("cloves")),
                    ing("olive_oil", Some(2.0), Some("tbsp")),
                    ing("tomatoes_can", Some(1.0), Some("can")),
                    ing("egg", Some(4.0), None),
                ],
            ),
            recipe("tacos", 2, vec![ing("onion", Some(100.0), None)]),
        ];
        let mut plan = MealPlan::default();
        plan.add_meal(0, "dal".into());
        plan.add_meal(1, "tacos".into());
        let cats = aggregate(&plan, &recipes, &catalog());

        assert_eq!(sub(line(&cats, "onion")), vec![(Dimension::Mass, 250.0)]);
        assert_eq!(sub(line(&cats, "garlic")), vec![(Dimension::Mass, 12.0)]);
        assert_eq!(
            sub(line(&cats, "olive_oil")),
            vec![(Dimension::Volume, 30.0)]
        );
        assert_eq!(
            sub(line(&cats, "tomatoes_can")),
            vec![(Dimension::Mass, 400.0)]
        );
        assert_eq!(sub(line(&cats, "egg")), vec![(Dimension::Count, 4.0)]);
        assert_eq!(line(&cats, "onion").sources, vec!["dal", "tacos"]);
        assert!(!line(&cats, "onion").to_taste);
    }

    #[test]
    fn missing_conversion_keeps_two_subtotals_primary_first() {
        let recipes = vec![recipe(
            "r",
            1,
            vec![
                ing("lemon", Some(2.0), Some("pcs")),
                ing("lemon", Some(100.0), None),
            ],
        )];
        let mut plan = MealPlan::default();
        plan.add_meal(0, "r".into());
        let cats = aggregate(&plan, &recipes, &catalog());
        assert_eq!(
            sub(line(&cats, "lemon")),
            vec![(Dimension::Mass, 100.0), (Dimension::Count, 2.0)]
        );
    }

    #[test]
    fn servings_scale_to_taste_and_optional_flags() {
        let mut seasoning = ing("salt", None, None);
        seasoning.optional = true;
        let recipes = vec![
            recipe(
                "a",
                2,
                vec![ing("olive_oil", Some(10.0), None), seasoning.clone()],
            ),
            recipe("b", 1, vec![ing("salt", Some(5.0), None)]),
        ];
        let mut plan = MealPlan::default();
        plan.add_meal(0, "a".into());
        plan.set_servings(0, 0, Some(4)); // ×2
        plan.add_meal(2, "a".into()); // default servings
        let cats = aggregate(&plan, &recipes, &catalog());
        assert_eq!(
            sub(line(&cats, "olive_oil")),
            vec![(Dimension::Volume, 30.0)]
        );
        let salt = line(&cats, "salt");
        assert!(salt.to_taste && salt.subtotals.is_empty() && salt.optional);

        plan.add_meal(3, "b".into());
        let cats = aggregate(&plan, &recipes, &catalog());
        let salt = line(&cats, "salt");
        assert!(salt.to_taste && !salt.optional);
        assert_eq!(sub(salt), vec![(Dimension::Mass, 5.0)]);
        assert_eq!(salt.sources, vec!["a", "b"]);
    }

    #[test]
    fn extras_are_added_unscaled_without_source() {
        let mut plan = MealPlan::default();
        plan.extras
            .push(Extra::Catalog(ing("egg", Some(6.0), None)));
        plan.extras
            .push(Extra::Catalog(ing("olive_oil", Some(0.5), Some("l"))));
        let cats = aggregate(&plan, &[], &catalog());
        assert_eq!(sub(line(&cats, "egg")), vec![(Dimension::Count, 6.0)]);
        assert_eq!(
            sub(line(&cats, "olive_oil")),
            vec![(Dimension::Volume, 500.0)]
        );
        assert!(line(&cats, "egg").sources.is_empty());
    }

    #[test]
    fn custom_extras_land_last_in_other_and_merge_by_name() {
        let mut plan = MealPlan::default();
        plan.add_meal(0, "r".into());
        plan.extras.push(Extra::Custom {
            name: "Bread".into(),
            quantity: Some("1 loaf".into()),
        });
        plan.extras.push(Extra::Custom {
            name: "bread ".into(),
            quantity: Some("2 rolls".into()),
        });
        plan.extras.push(Extra::Custom {
            name: "Candles".into(),
            quantity: None,
        });
        plan.extras.push(Extra::Custom {
            name: "   ".into(),
            quantity: None,
        });
        let recipes = vec![recipe("r", 1, vec![ing("salt", Some(1.0), None)])];
        let cats = aggregate(&plan, &recipes, &catalog());
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Spices & Oils", OTHER_CATEGORY]);
        let other = &cats[1];
        assert_eq!(other.lines.len(), 2);
        let bread = line(&cats, "custom:bread");
        assert_eq!(bread.name, "Bread");
        assert_eq!(bread.custom_quantities, vec!["1 loaf", "2 rolls"]);
        assert!(bread.subtotals.is_empty());
        assert!(line(&cats, "custom:candles").custom_quantities.is_empty());
        assert_eq!(custom_key(" Bread "), "custom:bread");
    }

    #[test]
    fn unknown_ingredients_and_recipes_are_skipped_and_order_is_deterministic() {
        let recipes = vec![recipe(
            "r",
            1,
            vec![
                ing("nope", Some(1.0), None),
                ing("salt", Some(1.0), None),
                ing("onion", Some(1.0), None),
            ],
        )];
        let mut plan = MealPlan::default();
        plan.add_meal(0, "r".into());
        plan.add_meal(1, "ghost".into());
        let cats = aggregate(&plan, &recipes, &catalog());
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Produce", "Spices & Oils"]);
        assert_eq!(cats[0].lines.len(), 1);
    }
}
