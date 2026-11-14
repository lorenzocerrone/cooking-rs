//! How well the recipes' ingredient names fit the dictionary: names it doesn't
//! know (with near matches) and amounts the grocery list can't add up. A
//! to-do list for tidying ingredients (by hand or by an agent), never a
//! failure: any name is allowed. Pure; `cargo run --bin ingredients` prints it.

use std::collections::BTreeMap;
use std::fmt;

use crate::extras::suggest;
use crate::models::{normalize_name, IngredientCatalog, Recipe};
use crate::scaffold::unit_hint;

/// A name no dictionary entry matches, where it is used and what it may be.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownName {
    /// As first written.
    pub name: String,
    /// Recipe ids using it (any spelling of the same normalized name).
    pub recipes: Vec<String>,
    /// Dictionary names that contain it or start with it: candidates for an alias.
    pub near: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IngredientReport {
    pub unknown: Vec<UnknownName>,
    /// `(recipe id, hint)` for amounts the grocery list can't add up.
    pub units: Vec<(String, String)>,
}

impl IngredientReport {
    pub fn is_empty(&self) -> bool {
        self.unknown.is_empty() && self.units.is_empty()
    }
}

pub fn ingredient_report(recipes: &[Recipe], catalog: &IngredientCatalog) -> IngredientReport {
    let mut unknown: BTreeMap<String, UnknownName> = BTreeMap::new();
    let mut units = Vec::new();
    for r in recipes {
        for item in &r.ingredients {
            if catalog.lookup(&item.name).is_none() {
                let entry = unknown
                    .entry(normalize_name(&item.name))
                    .or_insert_with(|| UnknownName {
                        name: item.name.trim().to_string(),
                        recipes: Vec::new(),
                        near: near_names(&item.name, catalog),
                    });
                if !entry.recipes.contains(&r.id) {
                    entry.recipes.push(r.id.clone());
                }
            }
            if let Some(hint) = unit_hint(item, catalog) {
                units.push((r.id.clone(), hint));
            }
        }
    }
    IngredientReport {
        unknown: unknown.into_values().collect(),
        units,
    }
}

/// Dictionary names close to `name`: matches for the whole name, then for
/// each of its words ("red lentils" → Lentils).
fn near_names(name: &str, catalog: &IngredientCatalog) -> Vec<String> {
    let mut near: Vec<String> = Vec::new();
    let words = std::iter::once(name).chain(name.split_whitespace().filter(|w| w.len() > 2));
    for word in words {
        for s in suggest(catalog, word, 3) {
            if !near.contains(&s.name) {
                near.push(s.name);
            }
        }
    }
    near.truncate(4);
    near
}

impl fmt::Display for IngredientReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return writeln!(f, "Every ingredient matches the dictionary and adds up.");
        }
        if !self.unknown.is_empty() {
            writeln!(f, "Not in the dictionary ({}):", self.unknown.len())?;
            for u in &self.unknown {
                write!(f, "  {} — in {}", u.name, u.recipes.join(", "))?;
                if !u.near.is_empty() {
                    write!(f, " — near: {}", u.near.join(", "))?;
                }
                writeln!(f)?;
            }
        }
        if !self.units.is_empty() {
            writeln!(
                f,
                "Amounts the grocery list can't add up ({}):",
                self.units.len()
            )?;
            for (id, hint) in &self.units {
                writeln!(f, "  {id}: {hint}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RecipeIngredient;
    use crate::recipes::{load_catalog, load_recipes};

    #[test]
    fn the_bundled_recipes_all_match() {
        let report = ingredient_report(&load_recipes(), &load_catalog());
        assert!(report.is_empty(), "{report}");
    }

    #[test]
    fn unknown_names_and_odd_units_are_listed() {
        let mut recipes = load_recipes();
        recipes.truncate(2);
        let mut flour = RecipeIngredient::new("Flour", Some(1.0));
        flour.unit = Some("cup".into());
        recipes[0]
            .ingredients
            .push(RecipeIngredient::new("Red lentils", Some(1.0)));
        recipes[1]
            .ingredients
            .push(RecipeIngredient::new("red  lentils", None));
        recipes[1].ingredients.push(flour);
        let report = ingredient_report(&recipes, &load_catalog());
        assert_eq!(report.unknown.len(), 1);
        let lentils = &report.unknown[0];
        assert_eq!(
            lentils.recipes,
            [recipes[0].id.clone(), recipes[1].id.clone()]
        );
        assert!(lentils.near.contains(&"Lentils".to_string()), "{report}");
        assert_eq!(report.units.len(), 1);
        assert!(report.to_string().contains("near: "), "{report}");
        // Aliases are found too.
        let near = near_names("courgette", &load_catalog());
        assert_eq!(near, ["Zucchini"]);
    }
}
