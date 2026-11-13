//! The recipe form behind the editor page: plain data a person fills in field by
//! field, turned into a [`Recipe`] (or a list of what is still missing). Any
//! ingredient name is accepted; the dictionary only suggests names and units
//! and flags amounts the grocery list can't add up. Pure: no Leptos types, no
//! browser access.

use serde::{Deserialize, Serialize};

use crate::extras::parse_number;
use crate::models::{CatalogIngredient, IngredientCatalog, Recipe, RecipeIngredient};
use crate::quantity::{is_convertible, Unit};
use crate::scaffold::{capitalize, set_diet_tag, slug, unit_hint};

/// Servings a new form starts with.
pub const DEFAULT_SERVINGS: u32 = 2;

/// Units offered for any ingredient (the dictionary adds piece labels).
pub const COMMON_UNITS: [&str; 7] = ["g", "kg", "ml", "l", "tsp", "tbsp", "cup"];

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
    /// Kept from an edited recipe, so the edit replaces it even if the name
    /// changes; a new recipe's id comes from its name.
    pub id: Option<String>,
    /// Kept from an edited recipe.
    pub image: Option<String>,
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
            id: None,
            image: None,
        }
    }
}

/// One ingredient line of the form, as typed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IngredientRow {
    pub name: String,
    /// Empty means "to taste".
    pub amount: String,
    /// Empty means a plain count ("2" eggs).
    pub unit: String,
    pub note: String,
    pub optional: bool,
    /// Kept from an edited recipe; the form doesn't show it.
    pub aisle: Option<String>,
}

impl IngredientRow {
    fn is_blank(&self) -> bool {
        self.name.trim().is_empty() && self.amount.trim().is_empty()
    }
}

/// Units to suggest for an ingredient: the ones that add up on the grocery
/// list when the dictionary knows it (its piece label included, "clove"),
/// otherwise the common ones.
pub fn unit_suggestions(name: &str, catalog: &IngredientCatalog) -> Vec<String> {
    match catalog.lookup(name) {
        Some((_, info)) => unit_choices(info),
        None => COMMON_UNITS.iter().map(|u| u.to_string()).collect(),
    }
}

/// The units that convert into this ingredient's shopping unit; pieces as its
/// label ("clove") when it has one.
pub fn unit_choices(info: &CatalogIngredient) -> Vec<String> {
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
    .filter_map(|u| match u {
        // A plain count needs no unit; a label reads better ("3 cloves").
        Unit::Piece => count.and_then(|c| c.label.clone()),
        u => Some(u.as_str().to_string()),
    })
    .collect()
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
    /// The id the recipe will get.
    pub fn id(&self) -> String {
        self.id.clone().unwrap_or_else(|| slug(&self.title))
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
    /// are ignored.
    pub fn to_recipe(&self, catalog: &IngredientCatalog) -> Result<Recipe, Vec<Issue>> {
        let mut issues = Vec::new();
        let mut issue = |field, message: String| issues.push(Issue { field, message });

        let title = self.title.trim();
        let id = self.id();
        if title.is_empty() {
            issue(Field::Title, "Give the recipe a name.".into());
        } else if id.is_empty() {
            issue(Field::Title, "The name needs a letter or a number.".into());
        }
        if self.author.trim().is_empty() {
            issue(Field::Author, "Say who the recipe is by.".into());
        }
        let prep = minutes(&self.prep);
        if prep.is_err() {
            issue(Field::Prep, "Prep time should be minutes, like 15.".into());
        }
        let cook = minutes(&self.cook);
        if cook.is_err() {
            issue(Field::Cook, "Cook time should be minutes, like 30.".into());
        }

        let mut ingredients = Vec::new();
        for (i, row) in self.ingredients.iter().enumerate() {
            if row.is_blank() {
                continue;
            }
            match row_to_ingredient(row) {
                Ok(item) => ingredients.push(item),
                Err(message) => issue(Field::Ingredient(i), message),
            }
        }
        if self.ingredients.iter().all(IngredientRow::is_blank) {
            issue(Field::Ingredients, "Add at least one ingredient.".into());
        }
        let steps: Vec<String> = self
            .steps
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if steps.is_empty() {
            issue(Field::Steps, "Add at least one step.".into());
        }
        if !issues.is_empty() {
            return Err(issues);
        }

        let text = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        let mut tags: Vec<String> = Vec::new();
        for t in &self.tags {
            let t = t.trim().to_lowercase();
            if !t.is_empty() && !tags.contains(&t) {
                tags.push(t);
            }
        }
        let mut recipe = Recipe {
            image: Some(
                self.image
                    .clone()
                    .unwrap_or_else(|| format!("images/{id}.svg")),
            ),
            id,
            title: title.to_string(),
            author: text(&self.author),
            servings: self.servings.max(1),
            ingredients,
            steps,
            description: text(&self.description),
            cuisine: text(&self.cuisine),
            tags,
            prep_time: prep.ok().flatten(),
            cook_time: cook.ok().flatten(),
        };
        set_diet_tag(&mut recipe, catalog);
        Ok(recipe)
    }

    /// Heads-ups per ingredient row (never blocking): a unit the grocery list
    /// can't add up for an ingredient the dictionary knows.
    pub fn hint(&self, i: usize, catalog: &IngredientCatalog) -> Option<String> {
        let row = self.ingredients.get(i)?;
        let item = row_to_ingredient(row).ok()?;
        unit_hint(&item, catalog)
    }

    /// Fill a form from an existing recipe (for editing it).
    pub fn from_recipe(recipe: &Recipe) -> RecipeForm {
        RecipeForm {
            title: recipe.title.clone(),
            author: recipe.author.clone().unwrap_or_default(),
            description: recipe.description.clone().unwrap_or_default(),
            servings: recipe.servings,
            prep: recipe.prep_time.map(|m| m.to_string()).unwrap_or_default(),
            cook: recipe.cook_time.map(|m| m.to_string()).unwrap_or_default(),
            cuisine: recipe.cuisine.clone().unwrap_or_default(),
            tags: recipe.tags.clone(),
            ingredients: recipe.ingredients.iter().map(row_from_ingredient).collect(),
            steps: recipe.steps.clone(),
            id: Some(recipe.id.clone()),
            image: recipe.image.clone(),
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
            Some(a / b)
        }
        None => parse_number(s),
    }
}

fn row_to_ingredient(row: &IngredientRow) -> Result<RecipeIngredient, String> {
    let name = row.name.trim();
    if name.is_empty() {
        return Err(format!(
            "Say what the ingredient with “{}” is.",
            row.amount.trim()
        ));
    }
    let amount = match row.amount.trim() {
        "" => None,
        typed => match parse_amount(typed) {
            Some(a) if a.is_finite() => Some(a),
            _ => {
                return Err(format!(
                    "{name}: the amount should be a number, like 200 or 1/2."
                ))
            }
        },
    };
    let text = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    Ok(RecipeIngredient {
        name: capitalize(name),
        amount,
        unit: amount.and_then(|_| text(&row.unit)),
        note: text(&row.note),
        optional: row.optional,
        aisle: row.aisle.clone(),
    })
}

pub(crate) fn row_from_ingredient(item: &RecipeIngredient) -> IngredientRow {
    IngredientRow {
        name: item.name.clone(),
        amount: item.amount.map(|a| format!("{a}")).unwrap_or_default(),
        unit: item.unit.clone().unwrap_or_default(),
        note: item.note.clone().unwrap_or_default(),
        optional: item.optional,
        aisle: item.aisle.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::{load_catalog, load_recipes};

    fn catalog() -> IngredientCatalog {
        load_catalog()
    }

    fn row(name: &str, amount: &str, unit: &str) -> IngredientRow {
        IngredientRow {
            name: name.into(),
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
                row("Garlic", "3", "cloves"),
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
        assert_eq!(r.ingredients[0].name, "Lentils");
        assert_eq!(r.ingredients[0].unit.as_deref(), Some("g"));
        assert_eq!(r.ingredients[1].unit.as_deref(), Some("cloves"));
        // No amount: to taste, no unit.
        assert_eq!(
            (r.ingredients[2].amount, r.ingredients[2].unit.clone()),
            (None, None)
        );
    }

    #[test]
    fn any_ingredient_is_accepted() {
        let mut form = filled();
        form.ingredients.push(row("star anise", "2", ""));
        form.ingredients.push(row("coriander", "1", "bunch"));
        let r = form.to_recipe(&catalog()).unwrap();
        assert_eq!(r.ingredients[3].name, "Star anise");
        assert_eq!(r.ingredients[4].unit.as_deref(), Some("bunch"));
        // Unknown ingredients: the vegetarian tag is the author's call.
        assert_eq!(r.tags, ["dinner"]);
        form.tags.push("vegetarian".into());
        let r = form.to_recipe(&catalog()).unwrap();
        assert_eq!(r.tags, ["dinner", "vegetarian"]);
    }

    #[test]
    fn what_is_missing_is_said_in_plain_words() {
        let form = RecipeForm {
            prep: "soon".into(),
            ingredients: vec![row("", "2", ""), row("lentils", "lots", "g")],
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
                Field::Steps,
            ]
        );
        let blank = RecipeForm::default().to_recipe(&catalog()).unwrap_err();
        assert!(blank.iter().any(|i| i.field == Field::Ingredients));
    }

    #[test]
    fn units_that_dont_add_up_get_a_hint() {
        let form = RecipeForm {
            ingredients: vec![
                row("flour", "1", "cup"),
                row("lentils", "300", "g"),
                row("star anise", "2", "cup"),
            ],
            ..RecipeForm::default()
        };
        let c = catalog();
        assert!(form.hint(0, &c).is_some_and(|h| h.contains("cup")));
        assert_eq!(form.hint(1, &c), None);
        // Unknown to the dictionary: nothing to compare against.
        assert_eq!(form.hint(2, &c), None);
    }

    #[test]
    fn unit_suggestions_follow_the_dictionary() {
        let c = catalog();
        assert_eq!(unit_suggestions("lentils", &c), ["g", "kg"]);
        assert!(unit_suggestions("garlic", &c).contains(&"clove".to_string()));
        assert_eq!(unit_suggestions("star anise", &c).len(), COMMON_UNITS.len());
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
            let back = RecipeForm::from_recipe(&recipe)
                .to_recipe(&c)
                .unwrap_or_else(|e| panic!("{}: {e:?}", recipe.id));
            assert_eq!(back, recipe, "{}", recipe.id);
        }
    }

    #[test]
    fn a_fresh_form_is_blank() {
        assert!(RecipeForm::default().is_blank());
        assert!(!filled().is_blank());
    }
}
