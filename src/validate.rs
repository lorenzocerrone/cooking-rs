//! Data validation for the embedded recipes and ingredient catalog.
//!
//! These are `cargo test` tests (host only): CI runs them on every push, so a
//! recipe with a typo'd field, a broken ingredient line, a wrong vegetarian tag
//! or a missing image fails the build instead of silently degrading the app.
//! Ingredient names are free; how well they match the dictionary is a report
//! (`cargo run --bin ingredients`), not a failure. Each
//! test collects *all* violations of one rule before failing so a data pass
//! reads as a checklist.

use std::collections::HashSet;
use std::path::Path;

use crate::filter::{meat_ingredients, unknown_ingredients, VEGETARIAN_TAG};
use crate::models::{normalize_name, IngredientCatalog, Recipe};
use crate::quantity::Unit;
use crate::recipes::{parse_catalog, recipe_files};
use crate::scaffold::TODO_STEP;
use crate::shopping::CATEGORY_ORDER;

fn catalog() -> IngredientCatalog {
    parse_catalog().expect("ingredients.json must parse (see all_data_files_parse)")
}

fn recipes() -> Vec<Recipe> {
    recipe_files()
        .into_iter()
        .filter_map(|(_, r)| r.ok())
        .collect()
}

fn assert_no_problems(rule: &str, problems: Vec<String>) {
    assert!(
        problems.is_empty(),
        "{rule}: {} problem(s)\n  - {}",
        problems.len(),
        problems.join("\n  - ")
    );
}

#[test]
fn all_data_files_parse() {
    let mut problems = Vec::new();
    match parse_catalog() {
        Ok(c) if c.0.is_empty() => problems.push("ingredients.json: catalog is empty".into()),
        Ok(_) => {}
        Err(e) => problems.push(format!("ingredients.json: {e}")),
    }
    let files = recipe_files();
    if files.is_empty() {
        problems.push("recipes/: no recipe files found".into());
    }
    for (name, result) in &files {
        if let Err(e) = result {
            problems.push(format!("recipes/{name}: {e}"));
        }
    }
    assert_no_problems("every data file parses", problems);
}

#[test]
fn recipe_metadata_is_sound() {
    let mut problems = Vec::new();
    let mut seen = HashSet::new();
    for (name, result) in recipe_files() {
        let Ok(r) = result else { continue };
        let stem = Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if r.id != stem {
            problems.push(format!("{name}: id `{}` does not match file name", r.id));
        }
        if !seen.insert(r.id.clone()) {
            problems.push(format!("{name}: duplicate id `{}`", r.id));
        }
        if r.title.trim().is_empty() {
            problems.push(format!("{name}: empty title"));
        }
        if r.author.as_deref().is_none_or(|a| a.trim().is_empty()) {
            problems.push(format!(
                "{name}: missing `author` (who wrote it; \"Claude\" for generated recipes)"
            ));
        }
        if r.servings == 0 {
            problems.push(format!("{name}: servings must be >= 1"));
        }
        if r.steps.is_empty() || r.steps.iter().any(|s| s.trim().is_empty()) {
            problems.push(format!("{name}: steps must be non-empty"));
        }
        if r.steps.iter().any(|s| s.trim() == TODO_STEP) {
            problems.push(format!(
                "{name}: replace the scaffolder's `{TODO_STEP}` step"
            ));
        }
        if r.ingredients.is_empty() {
            problems.push(format!("{name}: no ingredients"));
        }
    }
    assert_no_problems("recipe metadata", problems);
}

#[test]
fn vegetarian_tag_matches_the_ingredients() {
    // The Diet filter reads the tag. Meat or fish (a dictionary aisle, or the
    // line's own) rules it out; when the dictionary knows every ingredient and
    // none is, it is required. With an unknown ingredient it is the author's call.
    let catalog = catalog();
    let mut problems = Vec::new();
    for r in recipes() {
        let meat = meat_ingredients(&r, &catalog);
        let tagged = r.tags.iter().any(|t| t == VEGETARIAN_TAG);
        if tagged && !meat.is_empty() {
            problems.push(format!(
                "{}: tagged `{VEGETARIAN_TAG}` but uses {}",
                r.id,
                meat.join(", ")
            ));
        } else if !tagged && meat.is_empty() && unknown_ingredients(&r, &catalog).is_empty() {
            problems.push(format!(
                "{}: no meat or fish, add the `{VEGETARIAN_TAG}` tag",
                r.id
            ));
        }
    }
    assert_no_problems("vegetarian tag", problems);
}

#[test]
fn every_ingredient_line_is_sound() {
    // Any name is allowed; what must hold is that the line makes sense on its
    // own. (Names the dictionary doesn't know, and units the grocery list can't
    // add up, are reported by `cargo run --bin ingredients`, not failed.)
    let mut problems = Vec::new();
    for r in recipes() {
        for (n, item) in r.ingredients.iter().enumerate() {
            let at = format!("{}/ingredients[{n}] `{}`", r.id, item.name);
            if item.name.trim().is_empty() {
                problems.push(format!("{at}: empty name"));
            }
            match item.amount {
                None if item.unit.is_some() => problems.push(format!(
                    "{at}: `unit` without `amount` (\"to taste\" has no unit)"
                )),
                Some(a) if a.is_nan() || a <= 0.0 => {
                    problems.push(format!("{at}: amount must be > 0, got {a}"))
                }
                _ => {}
            }
            for (field, value) in [
                ("unit", &item.unit),
                ("note", &item.note),
                ("aisle", &item.aisle),
            ] {
                if value.as_deref().is_some_and(|v| v.trim().is_empty()) {
                    problems.push(format!("{at}: empty {field}"));
                }
            }
        }
    }
    assert_no_problems("ingredient lines", problems);
}

#[test]
fn catalog_entries_are_sound() {
    let catalog = catalog();
    let mut problems = Vec::new();
    for (key, info) in &catalog.0 {
        if !info.unit.is_base() {
            problems.push(format!(
                "{key}: shopping unit must be one of g/ml/pcs, got `{}`",
                info.unit.as_str()
            ));
        }
        if info.name.trim().is_empty() {
            problems.push(format!("{key}: name must be non-empty"));
        }
        if !CATEGORY_ORDER.contains(&info.category.as_str()) {
            problems.push(format!(
                "{key}: category `{}` is not an aisle in shopping::CATEGORY_ORDER",
                info.category
            ));
        }
        if let Some(c) = &info.count {
            if c.per_piece.is_some_and(|p| p.is_nan() || p <= 0.0) {
                problems.push(format!("{key}: count.per_piece must be > 0"));
            }
            if info.unit == Unit::Piece && c.label.is_none() && c.per_piece.is_none() {
                problems.push(format!("{key}: empty count spec"));
            }
            if c.plural.is_some() && c.label.is_none() {
                problems.push(format!("{key}: count.plural without count.label"));
            }
        }
    }
    // Every name a recipe might use must point at one entry only.
    let mut seen: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
    for (key, info) in &catalog.0 {
        let names = std::iter::once(key.replace('_', " "))
            .chain(std::iter::once(info.name.clone()))
            .chain(info.aliases.iter().cloned());
        for name in names {
            let name = normalize_name(&name);
            if name.is_empty() {
                problems.push(format!("{key}: empty alias"));
            } else if let Some(other) = seen.insert(name.clone(), key) {
                if other != key {
                    problems.push(format!("`{name}` names both {other} and {key}"));
                }
            }
        }
    }
    assert_no_problems("catalog entries", problems);
}

#[test]
fn every_recipe_image_exists_on_disk() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut problems = Vec::new();
    for r in recipes() {
        match r.image.as_deref() {
            None | Some("") => problems.push(format!("{}: missing image", r.id)),
            Some(path) if !root.join(path).is_file() => {
                problems.push(format!("{}: image `{path}` not found", r.id));
            }
            Some(_) => {}
        }
    }
    assert_no_problems("recipe images", problems);
}
