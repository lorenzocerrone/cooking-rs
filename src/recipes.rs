//! Data loading boundary.
//!
//! Recipes (`recipes/*.json`) and the ingredient catalog (`ingredients.json`) are
//! embedded into the wasm binary at compile time. `load_data` is intentionally
//! `async` so the rest of the app consumes it through `Resource` + `<Suspense>`.
//! To switch to runtime HTTP fetching later, only the loader bodies here change.
//!
//! Data *validity* (schema, references, units, images) is enforced by the test
//! suite in `src/validate.rs`, which runs on the host under `cargo test`.

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

/// Every embedded `recipes/*.json` file with its parse result, by file name.
/// Exposed so validation can report *which* file failed instead of silently
/// shipping without it.
pub fn recipe_files() -> Vec<(String, Result<Recipe, serde_json::Error>)> {
    let mut files: Vec<_> = RECIPE_DIR
        .files()
        .filter(|f| f.path().extension().is_some_and(|ext| ext == "json"))
        .map(|f| {
            let name = f
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            (name, serde_json::from_slice::<Recipe>(f.contents()))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// All recipes that parse, sorted by title.
pub fn load_recipes() -> Vec<Recipe> {
    let mut recipes: Vec<Recipe> = recipe_files()
        .into_iter()
        .filter_map(|(_, r)| r.ok())
        .collect();
    recipes.sort_by(|a, b| a.title.cmp(&b.title));
    recipes
}

/// Parse the embedded ingredient catalog (and its parse error, for validation).
pub fn parse_catalog() -> Result<IngredientCatalog, serde_json::Error> {
    serde_json::from_str(CATALOG_JSON)
}

/// The embedded ingredient catalog, empty if it fails to parse.
pub fn load_catalog() -> IngredientCatalog {
    parse_catalog().unwrap_or_default()
}
