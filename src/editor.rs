//! The recipe form behind the editor page: plain data a person fills in field by
//! field, turned into a checked [`Recipe`] (or a list of what is still missing)
//! against the ingredient catalog. New ingredients made in the form travel with
//! it. Pure: no Leptos types, no browser access.

use serde::{Deserialize, Serialize};

use crate::extras::parse_number;
use crate::models::{CatalogIngredient, IngredientCatalog, Recipe, RecipeIngredient};
use crate::quantity::{is_convertible, resolve_unit, CountSpec, Unit};
use crate::scaffold::{set_diet_tag, slug};

/// Servings a new form starts with.
pub const DEFAULT_SERVINGS: u32 = 2;

/// Everything the editor's fields hold. Text fields keep what was typed, so a
/// half-typed amount is never lost; [`RecipeForm::to_recipe`] reads them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecipeForm {
    pub title: String,
    pub author: String,
    pub description: String,
    pub servings: u32,
    /// Minutes, as typed.
    pub prep: String,
    pub cook: String,
    pub cuisine: String,
    pub tags: Vec<String>,
    pub ingredients: Vec<IngredientRow>,
    pub steps: Vec<String>,
    /// Ingredients made in this form, not yet in the catalog.
    pub new_ingredients: Vec<NewIngredient>,
}

impl Default for RecipeForm {
    fn default() -> Self {
        RecipeForm {
            title: String::new(),
            author: String::new(),
            description: String::new(),
            servings: DEFAULT_SERVINGS,
            prep: String::new(),
            cook: String::new(),
            cuisine: String::new(),
            tags: Vec::new(),
            ingredients: vec![IngredientRow::default()],
            steps: vec![String::new()],
            new_ingredients: Vec::new(),
        }
    }
}

/// One ingredient line of the form.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IngredientRow {
    /// What was typed in the search box (kept for the "pick one" hint).
    pub name: String,
    /// The catalog key once an ingredient is picked.
    pub key: Option<String>,
    /// As typed; empty means "to taste".
    pub amount: String,
    /// A [`UnitChoice::value`] for the picked ingredient.
    pub unit: String,
    pub note: String,
    pub optional: bool,
}

/// How a new ingredient is measured and bought.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measure {
    #[default]
    Weight,
    Volume,
    Count,
}

impl Measure {
    pub const ALL: [Measure; 3] = [Measure::Weight, Measure::Volume, Measure::Count];

    pub fn label(self) -> &'static str {
        match self {
            Measure::Weight => "By weight",
            Measure::Volume => "By volume",
            Measure::Count => "By the piece",
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Measure::Weight => Unit::G,
            Measure::Volume => Unit::Ml,
            Measure::Count => Unit::Piece,
        }
    }
}

/// An ingredient added from the form: a catalog entry in the making.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewIngredient {
    pub key: String,
    pub name: String,
    pub category: String,
    pub measure: Measure,
}

impl NewIngredient {
    pub fn new(name: &str, category: &str, measure: Measure) -> Self {
        NewIngredient {
            key: slug(name),
            name: crate::scaffold::capitalize(name.trim()),
            category: category.to_string(),
            measure,
        }
    }

    pub fn entry(&self) -> CatalogIngredient {
        CatalogIngredient {
            name: self.name.clone(),
            unit: self.measure.unit(),
            category: self.category.clone(),
            count: None,
        }
    }

    /// The line to add to `ingredients.json`.
    pub fn catalog_line(&self) -> String {
        let entry = serde_json::to_string(&self.entry()).expect("entries always serialize");
        format!("\"{}\": {entry},", self.key)
    }
}

/// A unit offered for an ingredient: `value` is stored in the row and the
/// recipe, `label` is what the picker shows.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitChoice {
    pub value: String,
    pub label: String,
}

/// The units a recipe may use for this ingredient: every one that converts
/// into its shopping unit, so the grocery list can always add them up. Pieces
/// use the ingredient's own label ("cloves") when it has one.
pub fn unit_choices(info: &CatalogIngredient) -> Vec<UnitChoice> {
    let count = info.count.as_ref();
    [
        Unit::G,
        Unit::Kg,
        Unit::Ml,
        Unit::L,
        Unit::Tsp,
        Unit::Tbsp,
        Unit::Cup,
        Unit::Piece,
    ]
    .into_iter()
    .filter(|u| is_convertible(*u, info.unit, count))
    .map(|u| match (u, count.and_then(|c| c.label.clone())) {
        (Unit::Piece, Some(label)) => UnitChoice {
            label: count
                .and_then(CountSpec::plural_label)
                .unwrap_or(label.clone()),
            value: label,
        },
        (Unit::Piece, None) => UnitChoice {
            value: Unit::Piece.as_str().into(),
            label: "pieces".into(),
        },
        (u, _) => UnitChoice {
            value: u.as_str().into(),
            label: u.as_str().into(),
        },
    })
    .collect()
}

/// The unit a freshly picked ingredient starts with: pieces when it can be
/// counted, otherwise its shopping unit.
pub fn default_unit(info: &CatalogIngredient) -> String {
    let choices = unit_choices(info);
    let pieces = choices
        .iter()
        .find(|c| resolve_unit(Some(&c.value), info.unit, info.count.as_ref()) == Ok(Unit::Piece));
    match (&info.count, pieces) {
        (Some(_), Some(p)) => p.value.clone(),
        _ => choices
            .into_iter()
            .find(|c| Unit::parse(&c.value) == Some(info.unit))
            .map(|c| c.value)
            .unwrap_or_default(),
    }
}

/// The unit a row keeps when its ingredient is picked: the matching choice
/// for what was typed or pasted ("tablespoons" → `tbsp`, "cloves" → `clove`),
/// the ingredient's usual unit when nothing was, and the typed text otherwise
/// (so the form asks for a unit instead of silently changing the amount).
pub fn fit_unit(typed: &str, info: &CatalogIngredient) -> String {
    let typed = typed.trim();
    if typed.is_empty() {
        return default_unit(info);
    }
    let count = info.count.as_ref();
    let Ok(wanted) = resolve_unit(Some(typed), info.unit, count) else {
        return typed.to_string();
    };
    unit_choices(info)
        .into_iter()
        .find(|c| resolve_unit(Some(&c.value), info.unit, count) == Ok(wanted))
        .map(|c| c.value)
        .unwrap_or_else(|| typed.to_string())
}

/// Which part of the form something is wrong with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Author,
    Prep,
    Cook,
    Ingredients,
    Ingredient(usize),
    Steps,
}

/// Something to fix before the recipe is complete, in plain words.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub field: Field,
    pub message: String,
}

impl RecipeForm {
    /// The catalog plus the ingredients made in this form.
    pub fn catalog_with(&self, base: &IngredientCatalog) -> IngredientCatalog {
        let mut catalog = base.clone();
        for n in &self.new_ingredients {
            catalog.0.entry(n.key.clone()).or_insert_with(|| n.entry());
        }
        catalog
    }

    /// Only the new ingredients some row still uses (a removed row leaves none behind).
    pub fn used_new_ingredients(&self) -> Vec<&NewIngredient> {
        self.new_ingredients
            .iter()
            .filter(|n| {
                self.ingredients
                    .iter()
                    .any(|r| r.key.as_deref() == Some(n.key.as_str()))
            })
            .collect()
    }

    /// The id the recipe will get.
    pub fn id(&self) -> String {
        slug(&self.title)
    }

    /// True when nothing has been filled in yet.
    pub fn is_blank(&self) -> bool {
        let fresh = RecipeForm::default();
        RecipeForm {
            servings: fresh.servings,
            ..self.clone()
        } == fresh
    }

    /// The recipe, or everything still to fix. Blank ingredient rows and steps
    /// are ignored. `catalog` is the base catalog; new ingredients are added.
    pub fn to_recipe(&self, base: &IngredientCatalog) -> Result<Recipe, Vec<Issue>> {
        let catalog = self.catalog_with(base);
        let mut issues = Vec::new();
        let mut issue = |field, message: &str| {
            issues.push(Issue {
                field,
                message: message.to_string(),
            })
        };

        let title = self.title.trim();
        let id = self.id();
        if title.is_empty() {
            issue(Field::Title, "Give the recipe a name.");
        } else if id.is_empty() {
            issue(Field::Title, "The name needs a letter or a number.");
        }
        if self.author.trim().is_empty() {
            issue(Field::Author, "Say who the recipe is by.");
        }
        let prep = minutes(&self.prep);
        if prep.is_err() {
            issue(Field::Prep, "Prep time should be minutes, like 15.");
        }
        let cook = minutes(&self.cook);
        if cook.is_err() {
            issue(Field::Cook, "Cook time should be minutes, like 30.");
        }

        let mut ingredients = Vec::new();
        for (i, row) in self.ingredients.iter().enumerate() {
            if row.key.is_none() && row.name.trim().is_empty() {
                continue;
            }
            match row_to_ingredient(row, &catalog) {
                Ok(item) => ingredients.push(item),
                Err(message) => issue(Field::Ingredient(i), &message),
            }
        }
        if ingredients.is_empty()
            && !self
                .ingredients
                .iter()
                .any(|r| r.key.is_some() || !r.name.trim().is_empty())
        {
            issue(Field::Ingredients, "Add at least one ingredient.");
        }
        let steps: Vec<String> = self
            .steps
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if steps.is_empty() {
            issue(Field::Steps, "Add at least one step.");
        }
        if !issues.is_empty() {
            return Err(issues);
        }

        let text = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        let mut recipe = Recipe {
            image: Some(format!("images/{id}.svg")),
            id,
            title: title.to_string(),
            author: text(&self.author),
            servings: self.servings.max(1),
            ingredients,
            steps,
            description: text(&self.description),
            cuisine: text(&self.cuisine),
            tags: self
                .tags
                .iter()
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect(),
            prep_time: prep.ok().flatten(),
            cook_time: cook.ok().flatten(),
        };
        recipe.tags.dedup();
        set_diet_tag(&mut recipe, &catalog);
        Ok(recipe)
    }

    /// Fill a form from an existing recipe (for editing it).
    pub fn from_recipe(recipe: &Recipe, catalog: &IngredientCatalog) -> RecipeForm {
        RecipeForm {
            title: recipe.title.clone(),
            author: recipe.author.clone().unwrap_or_default(),
            description: recipe.description.clone().unwrap_or_default(),
            servings: recipe.servings,
            prep: recipe.prep_time.map(|m| m.to_string()).unwrap_or_default(),
            cook: recipe.cook_time.map(|m| m.to_string()).unwrap_or_default(),
            cuisine: recipe.cuisine.clone().unwrap_or_default(),
            tags: recipe
                .tags
                .iter()
                .filter(|t| *t != crate::filter::VEGETARIAN_TAG)
                .cloned()
                .collect(),
            ingredients: recipe
                .ingredients
                .iter()
                .map(|item| row_from_ingredient(item, catalog))
                .collect(),
            steps: recipe.steps.clone(),
            new_ingredients: Vec::new(),
        }
    }
}

/// Empty is "not given"; otherwise whole minutes ("15", "15 min").
fn minutes(s: &str) -> Result<Option<u32>, ()> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let digits = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (number, rest) = s.split_at(digits);
    match rest.trim().to_lowercase().as_str() {
        "" | "m" | "min" | "mins" | "minute" | "minutes" => {
            number.parse().map(Some).map_err(|_| ())
        }
        _ => Err(()),
    }
}

/// An amount: a number (decimal comma accepted) or a fraction like `1/2`.
pub fn parse_amount(s: &str) -> Option<f64> {
    let s = s.trim();
    match s.split_once('/') {
        Some((a, b)) => {
            let (a, b) = (parse_number(a.trim())?, parse_number(b.trim())?);
            (b != 0.0).then(|| a / b)
        }
        None => parse_number(s),
    }
}

fn row_to_ingredient(
    row: &IngredientRow,
    catalog: &IngredientCatalog,
) -> Result<RecipeIngredient, String> {
    let Some((key, info)) = row.key.as_deref().and_then(|k| catalog.0.get_key_value(k)) else {
        let name = row.name.trim();
        return Err(format!(
            "Pick “{name}” from the list, or add it as a new ingredient."
        ));
    };
    let amount = match row.amount.trim() {
        "" => None,
        typed => match parse_amount(typed) {
            Some(a) if a > 0.0 && a.is_finite() => Some(a),
            _ => {
                return Err(format!(
                    "{}: the amount should be a number, like 200 or 1/2.",
                    info.name
                ))
            }
        },
    };
    // The shopping unit is implicit ("300 g lentils" is stored as just 300).
    let unit = match row.unit.trim() {
        "" => None,
        u if Unit::parse(u) == Some(info.unit) => None,
        u => Some(u.to_string()),
    };
    if amount.is_some() {
        let ok = resolve_unit(unit.as_deref(), info.unit, info.count.as_ref())
            .is_ok_and(|u| is_convertible(u, info.unit, info.count.as_ref()));
        if !ok {
            return Err(format!("{}: pick a unit from the list.", info.name));
        }
    }
    let note = Some(row.note.trim().to_string()).filter(|n| !n.is_empty());
    Ok(RecipeIngredient {
        ingredient: key.clone(),
        amount,
        unit: if amount.is_some() { unit } else { None },
        note,
        optional: row.optional,
    })
}

pub(crate) fn row_from_ingredient(
    item: &RecipeIngredient,
    catalog: &IngredientCatalog,
) -> IngredientRow {
    let info = catalog.get(&item.ingredient);
    let unit = info
        .map(|info| {
            let wanted = resolve_unit(item.unit.as_deref(), info.unit, info.count.as_ref()).ok();
            unit_choices(info)
                .into_iter()
                .find(|c| {
                    resolve_unit(Some(&c.value), info.unit, info.count.as_ref()).ok() == wanted
                })
                .map(|c| c.value)
                .unwrap_or_else(|| default_unit(info))
        })
        .unwrap_or_default();
    IngredientRow {
        name: info
            .map(|i| i.name.clone())
            .unwrap_or_else(|| item.ingredient.clone()),
        key: Some(item.ingredient.clone()),
        amount: item.amount.map(|a| format!("{a}")).unwrap_or_default(),
        unit,
        note: item.note.clone().unwrap_or_default(),
        optional: item.optional,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::{load_catalog, load_recipes};

    fn catalog() -> IngredientCatalog {
        load_catalog()
    }

    fn row(key: &str, amount: &str, unit: &str) -> IngredientRow {
        IngredientRow {
            name: key.into(),
            key: Some(key.into()),
            amount: amount.into(),
            unit: unit.into(),
            ..IngredientRow::default()
        }
    }

    fn filled() -> RecipeForm {
        RecipeForm {
            title: "Lentil Soup".into(),
            author: "Lorenzo".into(),
            prep: "10".into(),
            cook: "30 min".into(),
            tags: vec!["Dinner".into()],
            ingredients: vec![
                row("lentils", "300", "g"),
                row("garlic", "3", "clove"),
                row("salt", "", "g"),
                IngredientRow::default(),
            ],
            steps: vec!["Simmer.".into(), "  ".into()],
            ..RecipeForm::default()
        }
    }

    #[test]
    fn a_filled_form_becomes_a_recipe() {
        let r = filled().to_recipe(&catalog()).unwrap();
        assert_eq!(r.id, "lentil_soup");
        assert_eq!(r.image.as_deref(), Some("images/lentil_soup.svg"));
        assert_eq!((r.prep_time, r.cook_time), (Some(10), Some(30)));
        assert_eq!(r.tags, ["dinner", "vegetarian"]);
        assert_eq!(r.steps, ["Simmer."]);
        assert_eq!(r.ingredients.len(), 3);
        // The shopping unit is left implicit; a piece label is kept.
        assert_eq!(r.ingredients[0].unit, None);
        assert_eq!(r.ingredients[0].amount, Some(300.0));
        assert_eq!(r.ingredients[1].unit.as_deref(), Some("clove"));
        // No amount: to taste, no unit.
        assert_eq!(
            (r.ingredients[2].amount, r.ingredients[2].unit.clone()),
            (None, None)
        );
    }

    #[test]
    fn what_is_missing_is_said_in_plain_words() {
        let form = RecipeForm {
            prep: "soon".into(),
            ingredients: vec![
                IngredientRow {
                    name: "dragonfruit".into(),
                    ..IngredientRow::default()
                },
                row("lentils", "lots", "g"),
                row("lentils", "2", "tbsp"),
            ],
            ..RecipeForm::default()
        };
        let fields: Vec<Field> = form
            .to_recipe(&catalog())
            .unwrap_err()
            .into_iter()
            .map(|i| i.field)
            .collect();
        assert_eq!(
            fields,
            [
                Field::Title,
                Field::Author,
                Field::Prep,
                Field::Ingredient(0),
                Field::Ingredient(1),
                Field::Ingredient(2),
                Field::Steps,
            ]
        );
        let blank = RecipeForm::default().to_recipe(&catalog()).unwrap_err();
        assert!(blank.iter().any(|i| i.field == Field::Ingredients));
    }

    #[test]
    fn units_offered_are_the_ones_that_convert() {
        let c = catalog();
        let values = |key: &str| -> Vec<String> {
            unit_choices(c.get(key).unwrap())
                .into_iter()
                .map(|u| u.value)
                .collect()
        };
        assert_eq!(values("lentils"), ["g", "kg"]);
        assert!(values("garlic").contains(&"clove".to_string()));
        assert_eq!(default_unit(c.get("garlic").unwrap()), "clove");
        assert_eq!(default_unit(c.get("lentils").unwrap()), "g");
        let labels: Vec<String> = unit_choices(c.get("garlic").unwrap())
            .into_iter()
            .map(|u| u.label)
            .collect();
        assert!(labels.contains(&"cloves".to_string()));
    }

    #[test]
    fn a_typed_unit_is_kept_when_it_fits() {
        let c = catalog();
        let garlic = c.get("garlic").unwrap();
        let flour = c.get("flour").unwrap();
        assert_eq!(fit_unit("cloves", garlic), "clove");
        assert_eq!(fit_unit("", garlic), "clove");
        assert_eq!(fit_unit("grams", flour), "g");
        // A cup of flour needs a density the app doesn't have: kept, so it is asked about.
        assert_eq!(fit_unit("cup", flour), "cup");
    }

    #[test]
    fn new_ingredients_join_the_catalog() {
        let mut form = filled();
        let fruit = NewIngredient::new("dragon fruit", "Produce", Measure::Count);
        assert_eq!(fruit.key, "dragon_fruit");
        assert_eq!(fruit.name, "Dragon fruit");
        form.new_ingredients.push(fruit.clone());
        form.ingredients.push(row("dragon_fruit", "2", "pcs"));
        let r = form.to_recipe(&catalog()).unwrap();
        assert_eq!(r.ingredients.last().unwrap().ingredient, "dragon_fruit");
        assert_eq!(r.ingredients.last().unwrap().unit, None);
        assert_eq!(form.used_new_ingredients(), [&fruit]);
        assert_eq!(
            fruit.catalog_line(),
            r#""dragon_fruit": {"name":"Dragon fruit","unit":"pcs","category":"Produce"},"#
        );
        form.ingredients.pop();
        assert!(form.used_new_ingredients().is_empty());
    }

    #[test]
    fn minutes_are_whole_numbers() {
        assert_eq!(minutes(""), Ok(None));
        assert_eq!(minutes("15"), Ok(Some(15)));
        assert_eq!(minutes("15 min"), Ok(Some(15)));
        assert_eq!(minutes("soon"), Err(()));
        assert_eq!(minutes("1.5"), Err(()));
    }

    #[test]
    fn amounts_accept_fractions_and_decimal_commas() {
        assert_eq!(parse_amount("1/2"), Some(0.5));
        assert_eq!(parse_amount("0,5"), Some(0.5));
        assert_eq!(parse_amount("1/0"), None);
        assert_eq!(parse_amount("a"), None);
    }

    #[test]
    fn every_bundled_recipe_round_trips_through_the_form() {
        let c = catalog();
        for recipe in load_recipes() {
            let form = RecipeForm::from_recipe(&recipe, &c);
            let back = form
                .to_recipe(&c)
                .unwrap_or_else(|e| panic!("{}: {e:?}", recipe.id));
            assert_eq!(back.title, recipe.title, "{}", recipe.id);
            assert_eq!(back.steps, recipe.steps, "{}", recipe.id);
            assert_eq!(back.tags.len(), recipe.tags.len(), "{}", recipe.id);
            assert_eq!(back.ingredients.len(), recipe.ingredients.len());
            for (a, b) in back.ingredients.iter().zip(&recipe.ingredients) {
                let info = c.get(&a.ingredient).unwrap();
                let unit = |i: &RecipeIngredient| {
                    resolve_unit(i.unit.as_deref(), info.unit, info.count.as_ref()).ok()
                };
                assert_eq!(
                    (&a.ingredient, a.amount, unit(a), &a.note, a.optional),
                    (&b.ingredient, b.amount, unit(b), &b.note, b.optional),
                    "{}",
                    recipe.id
                );
            }
        }
    }

    #[test]
    fn a_fresh_form_is_blank() {
        assert!(RecipeForm::default().is_blank());
        assert!(!filled().is_blank());
    }
}
