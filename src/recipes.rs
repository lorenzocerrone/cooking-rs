//! Data loading boundary.
//!
//! Recipes (`recipes/*.json`) and the ingredient catalog (`ingredients.json`) are
//! embedded into the wasm binary at compile time. `load_data` is intentionally
//! `async` so the rest of the app consumes it through `Resource` + `<Suspense>`.
//! To switch to runtime HTTP fetching later, only the loader bodies here change.

use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

use crate::models::{IngredientCatalog, Recipe};

static RECIPE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/recipes");
static CATALOG_JSON: &str = include_str!("../ingredients.json");

/// Bundle of everything the pages need, loaded together so they share one resource.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppData {
    pub recipes: Vec<Recipe>,
    pub catalog: IngredientCatalog,
}

/// Load recipes + catalog. Embedded data resolves immediately; the `async`
/// signature keeps the call site identical for a future fetch-based loader.
pub async fn load_data() -> AppData {
    AppData {
        recipes: load_recipes(),
        catalog: load_catalog(),
    }
}

/// Parse and sort all embedded recipes by title.
pub fn load_recipes() -> Vec<Recipe> {
    let mut recipes: Vec<Recipe> = RECIPE_DIR
        .files()
        .filter(|f| f.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|f| serde_json::from_slice(f.contents()).ok())
        .collect();

    recipes.sort_by(|a, b| a.title.cmp(&b.title));
    recipes
}

/// Parse the embedded ingredient catalog.
pub fn load_catalog() -> IngredientCatalog {
    serde_json::from_str(CATALOG_JSON).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recipe_ingredient_exists_in_catalog() {
        let catalog = load_catalog();
        assert!(!catalog.0.is_empty(), "catalog failed to parse / is empty");

        let recipes = load_recipes();
        assert!(!recipes.is_empty(), "no recipes parsed");

        for recipe in &recipes {
            for item in &recipe.ingredients {
                assert!(
                    catalog.get(&item.ingredient).is_some(),
                    "recipe `{}` references unknown ingredient `{}`",
                    recipe.id,
                    item.ingredient,
                );
            }
        }
    }

    #[test]
    fn every_recipe_has_an_image() {
        for recipe in &load_recipes() {
            assert!(
                recipe.image.as_deref().is_some_and(|s| !s.is_empty()),
                "recipe `{}` is missing an image",
                recipe.id,
            );
        }
    }
}
