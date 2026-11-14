//! Shopping-list aggregation: turn a meal plan into summed quantities grouped by
//! ingredient category. Every contribution is converted into the ingredient's
//! shopping unit (see [`crate::quantity`]); anything that cannot be converted is
//! kept as a separate sub-total on the same line rather than dropped.
//!
//! This is pure logic — it returns data structures. Turning those into display
//! strings lives in [`crate::format`].

use std::collections::BTreeMap;

use crate::date::Date;
use crate::models::{
    measure, normalize_name, Extra, IngredientCatalog, MealPlan, Measure, Recipe, RecipeIngredient,
};
use crate::quantity::{to_shopping, Converted, CountSpec, Dimension, Quantity};

/// Preferred category ordering for the shopping list.
/// Unknown categories are appended afterwards, sorted alphabetically.
pub const CATEGORY_ORDER: &[&str] = &[
    "Produce",
    "Meat",
    "Fish",
    "Dairy & Eggs",
    "Pasta & Grains",
    "Bakery",
    "Legumes",
    "Pantry",
    "Frozen",
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

/// Line key for an ingredient the dictionary doesn't know (a recipe line or a
/// custom extra): stable per name, case-insensitive.
pub fn custom_key(name: &str) -> String {
    format!("custom:{}", normalize_name(name))
}

/// The grocery aisle of a recipe line: the dictionary's for a known
/// ingredient, else the line's own, else [`OTHER_CATEGORY`].
pub fn aisle(item: &RecipeIngredient, catalog: &IngredientCatalog) -> String {
    match catalog.lookup(&item.name) {
        Some((_, info)) => info.category.clone(),
        None => item
            .aisle
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .unwrap_or(OTHER_CATEGORY)
            .to_string(),
    }
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

fn capitalize(s: &str) -> String {
    crate::scaffold::capitalize(s)
}

/// Aggregate the meals dated `from..=to` (and all of the plan's extras) into a
/// category-grouped, summed shopping list. Deterministic: lines sort by name,
/// then key.
pub fn aggregate(
    plan: &MealPlan,
    from: Date,
    to: Date,
    recipes: &[Recipe],
    catalog: &IngredientCatalog,
) -> Vec<ShoppingCategory> {
    struct Acc {
        name: String,
        category: String,
        /// The dimension listed first: the shopping unit's for a known
        /// ingredient, the first amount's otherwise.
        primary: Option<Dimension>,
        count: Option<CountSpec>,
        totals: BTreeMap<Dimension, f64>,
        /// Amounts in piece words the app can't convert ("bunch"), by word.
        labelled: BTreeMap<String, f64>,
        to_taste: bool,
        optional: bool,
        sources: Vec<String>,
    }
    let mut lines: BTreeMap<String, Acc> = BTreeMap::new();

    let mut add = |item: &RecipeIngredient, factor: f64, source: Option<&str>| {
        let known = catalog.lookup(&item.name);
        let written = item.name.trim();
        if known.is_none() && written.is_empty() {
            return;
        }
        let key = known.map_or_else(|| custom_key(written), |(k, _)| k.to_string());
        let acc = lines.entry(key).or_insert_with(|| Acc {
            name: known.map_or_else(|| capitalize(written), |(_, i)| i.name.clone()),
            category: aisle(item, catalog),
            primary: known.map(|(_, i)| i.unit.dimension()),
            count: known.and_then(|(_, i)| i.count.clone()),
            totals: BTreeMap::new(),
            labelled: BTreeMap::new(),
            to_taste: false,
            optional: true,
            sources: Vec::new(),
        });
        acc.optional &= item.optional;
        if let Some(src) = source {
            if !acc.sources.iter().any(|s| s == src) {
                acc.sources.push(src.to_string());
            }
        }
        let Some(amount) = item.amount.map(|a| a * factor) else {
            acc.to_taste = true;
            return;
        };
        let info = known.map(|(_, i)| i);
        match measure(item, info) {
            Measure::Label(word) => {
                *acc.labelled.entry(word.to_lowercase()).or_insert(0.0) += amount;
            }
            Measure::Unit(unit) => {
                let q = Quantity::new(amount, unit);
                let (dimension, value) = match info {
                    Some(i) => match to_shopping(q, i.unit, i.count.as_ref()) {
                        Converted::Primary(v) => (i.unit.dimension(), v),
                        Converted::Other(d, v) => (d, v),
                    },
                    None => (q.dimension(), q.in_base()),
                };
                acc.primary.get_or_insert(dimension);
                *acc.totals.entry(dimension).or_insert(0.0) += value;
            }
        }
    };

    for entry in plan.entries_in(from, to) {
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
    let mut keyed: BTreeMap<String, (String, ShoppingLine)> = BTreeMap::new();
    for (key, acc) in lines {
        let mut subtotals: Vec<Subtotal> = acc
            .totals
            .into_iter()
            .map(|(dimension, amount)| Subtotal { dimension, amount })
            .collect();
        // Primary dimension first; the BTreeMap already ordered the rest.
        subtotals.sort_by_key(|s| Some(s.dimension) != acc.primary);
        let custom_quantities = acc
            .labelled
            .into_iter()
            .map(|(word, n)| crate::format::format_labelled(n, &word))
            .collect();
        keyed.insert(
            key.clone(),
            (
                acc.category,
                ShoppingLine {
                    key,
                    name: acc.name,
                    count: acc.count,
                    subtotals,
                    to_taste: acc.to_taste,
                    optional: acc.optional,
                    sources: acc.sources,
                    custom_quantities,
                },
            ),
        );
    }

    // Custom extras join the line of the same name (a recipe's unknown
    // ingredient, or another extra), else get their own in "Other"; quantities
    // are kept as typed.
    for extra in &plan.extras {
        let Extra::Custom { name, quantity } = extra else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let key = match catalog.lookup(name) {
            Some((k, _)) if keyed.contains_key(k) => k.to_string(),
            _ => custom_key(name),
        };
        let (_, line) = keyed.entry(key.clone()).or_insert_with(|| {
            (
                OTHER_CATEGORY.to_string(),
                ShoppingLine {
                    key,
                    name: capitalize(name),
                    count: None,
                    subtotals: Vec::new(),
                    to_taste: false,
                    optional: false,
                    sources: Vec::new(),
                    custom_quantities: Vec::new(),
                },
            )
        });
        line.optional = false;
        if let Some(q) = quantity.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            line.custom_quantities.push(q.to_string());
        }
    }
    for (category, line) in keyed.into_values() {
        by_category.entry(category).or_default().push(line);
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
            aliases: Vec::new(),
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
            name: key.into(),
            amount,
            unit: unit.map(Into::into),
            note: None,
            optional: false,
            aisle: None,
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
            author: None,
        }
    }

    fn line<'a>(cats: &'a [ShoppingCategory], key: &str) -> &'a ShoppingLine {
        cats.iter()
            .flat_map(|c| c.lines.iter())
            .find(|l| l.key == key)
            .unwrap_or_else(|| panic!("no line for {key}"))
    }

    /// `n` days after a fixed "today".
    fn d(n: i32) -> Date {
        Date::from_ymd(2026, 10, 25).unwrap().add_days(n)
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
            recipe("tacos", 2, vec![ing("onion", Some(100.0), Some("g"))]),
        ];
        let mut plan = MealPlan::default();
        plan.add_meal(d(0), "dal".into());
        plan.add_meal(d(1), "tacos".into());
        let cats = aggregate(&plan, d(0), d(13), &recipes, &catalog());

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
                ing("lemon", Some(100.0), Some("g")),
            ],
        )];
        let mut plan = MealPlan::default();
        plan.add_meal(d(0), "r".into());
        let cats = aggregate(&plan, d(0), d(13), &recipes, &catalog());
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
                vec![ing("olive_oil", Some(10.0), Some("ml")), seasoning.clone()],
            ),
            recipe("b", 1, vec![ing("salt", Some(5.0), Some("g"))]),
        ];
        let mut plan = MealPlan::default();
        plan.add_meal(d(0), "a".into());
        plan.set_servings(d(0), 0, Some(4)); // ×2
        plan.add_meal(d(2), "a".into()); // default servings
        let cats = aggregate(&plan, d(0), d(13), &recipes, &catalog());
        assert_eq!(
            sub(line(&cats, "olive_oil")),
            vec![(Dimension::Volume, 30.0)]
        );
        let salt = line(&cats, "salt");
        assert!(salt.to_taste && salt.subtotals.is_empty() && salt.optional);

        plan.add_meal(d(3), "b".into());
        let cats = aggregate(&plan, d(0), d(13), &recipes, &catalog());
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
        let cats = aggregate(&plan, d(0), d(13), &[], &catalog());
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
        plan.add_meal(d(0), "r".into());
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
        let recipes = vec![recipe("r", 1, vec![ing("salt", Some(1.0), Some("g"))])];
        let cats = aggregate(&plan, d(0), d(13), &recipes, &catalog());
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
    fn unknown_ingredients_get_their_own_line_and_unknown_recipes_are_skipped() {
        let mut anise = ing("Star anise", Some(2.0), None);
        anise.note = Some("whole".into());
        let mut chorizo = ing("chorizo", Some(100.0), Some("g"));
        chorizo.aisle = Some("Meat".into());
        let recipes = vec![
            recipe(
                "r",
                1,
                vec![
                    anise,
                    ing("star anise ", Some(1.0), None),
                    ing("coriander", Some(1.0), Some("bunch")),
                    ing("Coriander", Some(20.0), Some("g")),
                    chorizo,
                    ing("salt", Some(1.0), Some("g")),
                    ing("onion", Some(1.0), Some("pcs")),
                ],
            ),
            recipe("s", 2, vec![ing("coriander", Some(2.0), Some("bunch"))]),
        ];
        let mut plan = MealPlan::default();
        plan.add_meal(d(0), "r".into());
        plan.add_meal(d(1), "s".into());
        plan.add_meal(d(2), "ghost".into());
        plan.extras.push(Extra::Custom {
            name: "Star Anise".into(),
            quantity: Some("1 jar".into()),
        });
        let cats = aggregate(&plan, d(0), d(13), &recipes, &catalog());
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Produce", "Meat", "Spices & Oils", OTHER_CATEGORY]
        );
        // Same name in any case: one line; the custom extra joins it.
        let anise = line(&cats, "custom:star anise");
        assert_eq!(anise.name, "Star anise");
        assert_eq!(sub(anise), vec![(Dimension::Count, 3.0)]);
        assert_eq!(anise.custom_quantities, vec!["1 jar"]);
        // Piece words the app can't convert are summed by word, beside the rest.
        let coriander = line(&cats, "custom:coriander");
        assert_eq!(sub(coriander), vec![(Dimension::Mass, 20.0)]);
        assert_eq!(coriander.custom_quantities, vec!["3 bunches"]);
        assert_eq!(
            crate::format::format_line_quantity(coriander),
            "20 g + 3 bunches"
        );
        assert_eq!(line(&cats, "custom:chorizo").sources, vec!["r"]);
    }

    #[test]
    fn only_meals_in_range_count_and_extras_always_do() {
        let recipes = vec![
            recipe("a", 1, vec![ing("onion", Some(100.0), Some("g"))]),
            recipe("b", 1, vec![ing("onion", Some(50.0), Some("g"))]),
        ];
        let mut plan = MealPlan::default();
        plan.add_meal(d(0), "a".into());
        plan.add_meal(d(2), "b".into());
        plan.add_meal(d(5), "a".into());
        plan.extras
            .push(Extra::Catalog(ing("egg", Some(6.0), None)));

        let cats = aggregate(&plan, d(0), d(2), &recipes, &catalog());
        assert_eq!(sub(line(&cats, "onion")), vec![(Dimension::Mass, 150.0)]);
        assert_eq!(line(&cats, "onion").sources, vec!["a", "b"]);

        let cats = aggregate(&plan, d(0), d(0), &recipes, &catalog());
        assert_eq!(sub(line(&cats, "onion")), vec![(Dimension::Mass, 100.0)]);

        let cats = aggregate(&plan, d(3), d(4), &recipes, &catalog());
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Dairy & Eggs"], "only the extra");
    }
}
