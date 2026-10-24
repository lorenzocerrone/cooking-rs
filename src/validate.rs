//! Data validation for the embedded recipes and ingredient catalog.
//!
//! These are `cargo test` tests (host only): CI runs them on every push, so a
//! recipe with a typo'd field, an unknown ingredient, an unconvertible unit or a
//! missing image fails the build instead of silently degrading the app. Each
//! test collects *all* violations of one rule before failing so a data pass
//! reads as a checklist.

use std::collections::HashSet;
use std::path::Path;

use crate::models::{IngredientCatalog, Recipe};
use crate::quantity::{is_convertible, resolve_unit, Unit};
use crate::recipes::{parse_catalog, recipe_files};

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
        if r.servings == 0 {
            problems.push(format!("{name}: servings must be >= 1"));
        }
        if r.steps.is_empty() || r.steps.iter().any(|s| s.trim().is_empty()) {
            problems.push(format!("{name}: steps must be non-empty"));
        }
        if r.ingredients.is_empty() {
            problems.push(format!("{name}: no ingredients"));
        }
    }
    assert_no_problems("recipe metadata", problems);
}

#[test]
fn every_recipe_ingredient_exists_in_catalog() {
    let catalog = catalog();
    let mut problems = Vec::new();
    for r in recipes() {
        for item in &r.ingredients {
            if catalog.get(&item.ingredient).is_none() {
                problems.push(format!(
                    "{}: unknown ingredient `{}`",
                    r.id, item.ingredient
                ));
            }
        }
    }
    assert_no_problems("ingredient references", problems);
}

#[test]
fn every_recipe_quantity_is_valid_and_convertible() {
    let catalog = catalog();
    let mut problems = Vec::new();
    for r in recipes() {
        for item in &r.ingredients {
            let Some(info) = catalog.get(&item.ingredient) else {
                continue;
            };
            let at = format!("{}/{}", r.id, item.ingredient);
            match item.amount {
                None => {
                    if item.unit.is_some() {
                        problems.push(format!(
                            "{at}: `unit` without `amount` (\"to taste\" has no unit)"
                        ));
                    }
                }
                Some(a) if a.is_nan() || a <= 0.0 => {
                    problems.push(format!("{at}: amount must be > 0, got {a}"))
                }
                Some(_) => match resolve_unit(item.unit.as_deref(), info.unit, info.count.as_ref())
                {
                    Err(e) => problems.push(format!("{at}: unknown unit `{}`", e.0)),
                    Ok(unit) if !is_convertible(unit, info.unit, info.count.as_ref()) => {
                        problems.push(format!(
                            "{at}: `{}` cannot be converted into the shopping unit `{}` (add `count.per_piece` to the catalog entry)",
                            unit.as_str(),
                            info.unit.as_str()
                        ));
                    }
                    Ok(_) => {}
                },
            }
            if item.note.as_deref().is_some_and(|n| n.trim().is_empty()) {
                problems.push(format!("{at}: empty note"));
            }
        }
    }
    assert_no_problems("recipe quantities", problems);
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
        if info.name.trim().is_empty() || info.category.trim().is_empty() {
            problems.push(format!("{key}: name and category must be non-empty"));
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
