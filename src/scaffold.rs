//! Pure logic for the host-only recipe scaffolder (`src/bin/scaffold.rs`): turn
//! a plain-text draft (title, optional headers, ingredient lines, steps) into a
//! [`Recipe`] checked against the catalog, then render it as recipe JSON in the
//! house style and as a placeholder SVG image. No Leptos types, no file I/O.
//!
//! ```text
//! Lentil Dal
//! author: Lorenzo
//! servings: 4
//! cuisine: Indian
//! tags: dinner, vegetarian
//!
//! 300 g lentils
//! 1 onion, finely chopped
//! 3 cloves garlic
//! 1/2 can coconut milk
//! chives (optional)
//! salt
//!
//! Steps
//! Soften the onion and garlic.
//! ```

use crate::extras::{parse_number, suggest};
use crate::models::{CatalogIngredient, IngredientCatalog, Recipe, RecipeIngredient};
use crate::quantity::{is_convertible, resolve_unit, Unit};

/// Step text written when a draft has no steps. The data validation suite
/// rejects it, so a placeholder can't ship.
pub const TODO_STEP: &str = "TODO";

/// Servings used when the draft has no `servings:` header.
const DEFAULT_SERVINGS: u32 = 2;

/// A problem with one line of the draft (1-based line number).
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub line: usize,
    pub message: String,
}

/// A parsed draft: the recipe plus non-fatal warnings.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub recipe: Recipe,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Title,
    Headers,
    Ingredients,
    Steps,
}

/// Parse a whole draft. Every problem is collected, so one run lists them all.
pub fn parse_draft(text: &str, catalog: &IngredientCatalog) -> Result<Draft, Vec<Problem>> {
    let mut problems = Vec::new();
    let mut section = Section::Title;
    let mut title = String::new();
    let mut id = None;
    let mut servings = DEFAULT_SERVINGS;
    let mut recipe_meta = Recipe {
        id: String::new(),
        title: String::new(),
        servings: DEFAULT_SERVINGS,
        ingredients: Vec::new(),
        steps: Vec::new(),
        description: None,
        cuisine: None,
        tags: Vec::new(),
        prep_time: None,
        cook_time: None,
        image: None,
        author: None,
    };

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        let mut problem = |message: String| {
            problems.push(Problem {
                line: line_no,
                message,
            })
        };
        if line.is_empty() {
            if section == Section::Headers {
                section = Section::Ingredients;
            }
            continue;
        }
        if section != Section::Title && is_steps_marker(line) {
            section = Section::Steps;
            continue;
        }
        match section {
            Section::Title => {
                title = line.to_string();
                section = Section::Headers;
            }
            Section::Headers => match line.split_once(':') {
                Some((key, value)) => {
                    let value = value.trim();
                    match key.trim().to_ascii_lowercase().as_str() {
                        "id" => id = Some(value.to_string()),
                        "servings" | "serves" => match value.parse::<u32>() {
                            Ok(n) if n > 0 => servings = n,
                            _ => problem(format!("servings must be a whole number > 0, got `{value}`")),
                        },
                        "author" => recipe_meta.author = Some(value.to_string()),
                        "cuisine" => recipe_meta.cuisine = Some(value.to_string()),
                        "description" => recipe_meta.description = Some(value.to_string()),
                        "tags" => {
                            recipe_meta.tags = value
                                .split(',')
                                .map(|t| t.trim().to_lowercase())
                                .filter(|t| !t.is_empty())
                                .collect()
                        }
                        "prep" | "prep_time" => match parse_minutes(value) {
                            Some(m) => recipe_meta.prep_time = Some(m),
                            None => problem(format!("prep must be minutes, got `{value}`")),
                        },
                        "cook" | "cook_time" => match parse_minutes(value) {
                            Some(m) => recipe_meta.cook_time = Some(m),
                            None => problem(format!("cook must be minutes, got `{value}`")),
                        },
                        other => problem(format!(
                            "unknown header `{other}` (use author, id, servings, cuisine, tags, prep, cook, description)"
                        )),
                    }
                }
                // No header form: the ingredients started without a blank line.
                None => {
                    section = Section::Ingredients;
                    match parse_line(line, catalog) {
                        Ok(item) => recipe_meta.ingredients.push(item),
                        Err(message) => problem(message),
                    }
                }
            },
            Section::Ingredients => match parse_line(line, catalog) {
                Ok(item) => recipe_meta.ingredients.push(item),
                Err(message) => problem(message),
            },
            Section::Steps => recipe_meta.steps.push(strip_numbering(line).to_string()),
        }
    }

    if title.is_empty() {
        problems.push(Problem {
            line: 1,
            message: "the first line must be the recipe title".into(),
        });
    }
    if !title.is_empty() && recipe_meta.author.as_deref().is_none_or(str::is_empty) {
        problems.push(Problem {
            line: 2,
            message: "add an `author:` line (who wrote the recipe)".into(),
        });
    }
    if recipe_meta.ingredients.is_empty() && problems.is_empty() {
        problems.push(Problem {
            line: text.lines().count().max(1),
            message: "no ingredient lines".into(),
        });
    }
    let id = id.unwrap_or_else(|| slug(&title));
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        problems.push(Problem {
            line: 1,
            message: format!("id `{id}` must be non-empty lowercase a-z, 0-9 and _"),
        });
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let mut warnings = Vec::new();
    if recipe_meta.steps.is_empty() {
        recipe_meta.steps.push(TODO_STEP.into());
        warnings.push(format!(
            "no steps: wrote a `{TODO_STEP}` step; `cargo test` fails until it is replaced"
        ));
    }
    Ok(Draft {
        recipe: Recipe {
            image: Some(format!("images/{id}.svg")),
            id,
            title,
            servings,
            ..recipe_meta
        },
        warnings,
    })
}

fn is_steps_marker(line: &str) -> bool {
    matches!(
        line.trim_end_matches(':').to_ascii_lowercase().as_str(),
        "steps" | "method"
    )
}

/// "1. Boil" / "2) Boil" / "- Boil" → "Boil".
fn strip_numbering(line: &str) -> &str {
    let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
    let rest = if rest.len() < line.len() {
        rest.strip_prefix('.')
            .or_else(|| rest.strip_prefix(')'))
            .unwrap_or(line)
    } else {
        rest.strip_prefix('-')
            .or_else(|| rest.strip_prefix('*'))
            .unwrap_or(line)
    };
    rest.trim()
}

/// "10" / "10 min" / "10 minutes" → 10.
fn parse_minutes(value: &str) -> Option<u32> {
    let number = value
        .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c.is_whitespace())
        .trim();
    number.parse().ok()
}

/// An amount: a number (decimal comma accepted) or a fraction `a/b`.
fn parse_amount(token: &str) -> Option<f64> {
    match token.split_once('/') {
        Some((a, b)) => {
            let (a, b) = (parse_number(a)?, parse_number(b)?);
            Some(a / b)
        }
        None => parse_number(token),
    }
}

/// Parse one ingredient line (`"2 tbsp olive oil, warmed (optional)"`) into a
/// recipe ingredient, or explain why it can't be used.
///
/// A leading number is the amount. The next token is the unit only if it is a
/// [`Unit`] spelling or the matched ingredient's piece label, so `"1 red onion"`
/// keeps `red onion` as the name. A bare number counts pieces (`"2 onion"` is two
/// onions, not 2 g), and every unit must convert into the shopping unit.
pub fn parse_line(line: &str, catalog: &IngredientCatalog) -> Result<RecipeIngredient, String> {
    let mut body = line.trim();
    let mut optional = false;
    if let Some(stripped) = strip_suffix_ci(body, "(optional)") {
        body = stripped.trim_end();
        optional = true;
    }
    let (body, note) = match note_comma(body) {
        Some(at) => (
            body[..at].trim(),
            Some(body[at + 1..].trim()).filter(|n| !n.is_empty()),
        ),
        None => (body, None),
    };
    let tokens: Vec<&str> = body.split_whitespace().collect();
    let Some(first) = tokens.first() else {
        return Err(format!("`{line}`: no ingredient name"));
    };

    let amount = parse_amount(first);
    let (unit, name): (Option<&str>, Vec<&str>) = match amount {
        None => (None, tokens.clone()),
        Some(_) => {
            let rest = &tokens[1..];
            match rest {
                [] => return Err(format!("`{line}`: an amount but no ingredient")),
                [unit, name @ ..] if !name.is_empty() && Unit::parse(unit).is_some() => {
                    (Some(*unit), name.to_vec())
                }
                [label, name @ ..]
                    if !name.is_empty()
                        && find_ingredient(catalog, &name.join(" ")).is_some_and(
                            |(_, info)| info.count.as_ref().is_some_and(|c| c.matches_label(label)),
                        ) =>
                {
                    (Some(*label), name.to_vec())
                }
                _ => (None, rest.to_vec()),
            }
        }
    };
    let name = name.join(" ");
    let Some((key, info)) = find_ingredient(catalog, &name) else {
        return Err(unknown_ingredient(
            catalog,
            &name,
            amount.is_some() && unit.is_none(),
        ));
    };

    // A bare number counts pieces. For an ingredient shopped in pieces that is
    // its shopping unit, so the unit stays implicit (`"egg", 4`), as does an
    // explicit shopping unit ("300 g lentils" → `"lentils", 300`).
    let unit = match (amount, unit) {
        (Some(_), None) if info.unit != Unit::Piece => Some("pcs"),
        (_, Some(u)) if Unit::parse(u) == Some(info.unit) => None,
        _ => unit,
    };
    if amount.is_some() {
        let resolved = resolve_unit(unit, info.unit, info.count.as_ref())
            .map_err(|e| format!("`{line}`: unknown unit `{}`", e.0))?;
        if !is_convertible(resolved, info.unit, info.count.as_ref()) {
            return Err(if unit == Some("pcs") && info.unit != Unit::Piece {
                format!(
                    "`{line}`: a bare number counts pieces, but {} has no piece size; add a unit (g, ml, tbsp…) or `count.per_piece` to its catalog entry",
                    info.name
                )
            } else {
                format!(
                    "`{line}`: `{}` does not convert into {}'s shopping unit `{}`",
                    resolved.as_str(),
                    info.name,
                    info.unit.as_str()
                )
            });
        }
    }

    Ok(RecipeIngredient {
        ingredient: key.to_string(),
        amount,
        unit: unit.map(str::to_string),
        note: note.map(str::to_string),
        optional,
    })
}

/// The comma that starts the note: the first one not inside a decimal number
/// (`"0,5 l milk, warm"` splits at the second comma).
fn note_comma(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    (0..bytes.len()).find(|&i| {
        bytes[i] == b','
            && !(i > 0
                && bytes[i - 1].is_ascii_digit()
                && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
    })
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let split = s.len().checked_sub(suffix.len())?;
    (s.is_char_boundary(split) && s[split..].eq_ignore_ascii_case(suffix)).then(|| &s[..split])
}

/// Find a catalog ingredient by key or display name, case-insensitive, also
/// trying singular and plural forms (`"tomatoes"` → `tomato`, `"olive"` →
/// `olives`). Never guesses beyond that.
pub fn find_ingredient<'a>(
    catalog: &'a IngredientCatalog,
    name: &str,
) -> Option<(&'a str, &'a CatalogIngredient)> {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return None;
    }
    let mut candidates = vec![name.clone()];
    if let Some(s) = name.strip_suffix("es") {
        candidates.push(s.to_string());
    }
    if let Some(s) = name.strip_suffix('s') {
        candidates.push(s.to_string());
    }
    candidates.push(format!("{name}s"));
    candidates.iter().find_map(|c| {
        let key = slug(c);
        catalog
            .0
            .get_key_value(&key)
            .or_else(|| catalog.0.iter().find(|(_, i)| i.name.to_lowercase() == *c))
            .map(|(k, i)| (k.as_str(), i))
    })
}

fn unknown_ingredient(catalog: &IngredientCatalog, name: &str, counted: bool) -> String {
    let mut message = format!("unknown ingredient `{name}`");
    let hints: Vec<String> = suggest(catalog, name, 3)
        .into_iter()
        .map(|s| format!("{} (`{}`)", s.name, s.key))
        .collect();
    if !hints.is_empty() {
        message.push_str(&format!("; did you mean {}?", hints.join(", ")));
    }
    message.push_str("\n    add it to ingredients.json:\n    ");
    message.push_str(&catalog_stub(name, counted, None));
    // "2 sprigs rosemary": the first word may be a piece label of a new ingredient.
    if let Some((label, rest)) = name.split_once(' ').filter(|_| counted) {
        let label = label.strip_suffix('s').unwrap_or(label);
        message.push_str(&format!("\n    or, if `{label}` is a piece label:\n    "));
        message.push_str(&catalog_stub(rest, true, Some(label)));
    }
    message
}

/// A catalog entry to paste into `ingredients.json` for an unknown ingredient.
/// The category is a placeholder that validation rejects until it is chosen;
/// `counted` adds a piece size to fill in, `label` names one piece.
pub fn catalog_stub(name: &str, counted: bool, label: Option<&str>) -> String {
    let display = capitalize(name.trim());
    let count = match (counted, label) {
        (false, _) => String::new(),
        (true, None) => r#", "count": { "per_piece": 100 }"#.into(),
        (true, Some(l)) => format!(
            r#", "count": {{ "label": {}, "per_piece": 100 }}"#,
            json_str(l)
        ),
    };
    format!(
        r#""{}": {{ "name": {}, "unit": "g", "category": "?"{count} }},"#,
        slug(name),
        json_str(&display)
    )
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `"Chicken & Spinach Curry!"` → `chicken_spinach_curry`.
pub fn slug(title: &str) -> String {
    title
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("strings always serialize")
}

/// The recipe as JSON in the house style of `recipes/*.json`: fields in the
/// usual order, empty optional fields left out, one ingredient per line.
pub fn render_json(r: &Recipe) -> String {
    let mut fields = vec![
        format!(r#""id": {}"#, json_str(&r.id)),
        format!(r#""title": {}"#, json_str(&r.title)),
    ];
    if let Some(a) = &r.author {
        fields.push(format!(r#""author": {}"#, json_str(a)));
    }
    fields.extend([format!(r#""servings": {}"#, r.servings)]);
    if let Some(image) = &r.image {
        fields.push(format!(r#""image": {}"#, json_str(image)));
    }
    if let Some(d) = &r.description {
        fields.push(format!(r#""description": {}"#, json_str(d)));
    }
    if let Some(c) = &r.cuisine {
        fields.push(format!(r#""cuisine": {}"#, json_str(c)));
    }
    if !r.tags.is_empty() {
        let tags: Vec<String> = r.tags.iter().map(|t| json_str(t)).collect();
        fields.push(format!(r#""tags": [{}]"#, tags.join(", ")));
    }
    if let Some(m) = r.prep_time {
        fields.push(format!(r#""prep_time": {m}"#));
    }
    if let Some(m) = r.cook_time {
        fields.push(format!(r#""cook_time": {m}"#));
    }
    let ingredients: Vec<String> = r
        .ingredients
        .iter()
        .map(|i| format!("    {}", ingredient_json(i)))
        .collect();
    fields.push(format!(
        "\"ingredients\": [\n{}\n  ]",
        ingredients.join(",\n")
    ));
    let steps: Vec<String> = r
        .steps
        .iter()
        .map(|s| format!("    {}", json_str(s)))
        .collect();
    fields.push(format!("\"steps\": [\n{}\n  ]", steps.join(",\n")));
    format!("{{\n  {}\n}}\n", fields.join(",\n  "))
}

fn ingredient_json(i: &RecipeIngredient) -> String {
    let mut parts = vec![format!(r#""ingredient": {}"#, json_str(&i.ingredient))];
    if let Some(a) = i.amount {
        // `Display` prints 300.0 as "300" and 0.5 as "0.5".
        parts.push(format!(r#""amount": {a}"#));
    }
    if let Some(u) = &i.unit {
        parts.push(format!(r#""unit": {}"#, json_str(u)));
    }
    if let Some(n) = &i.note {
        parts.push(format!(r#""note": {}"#, json_str(n)));
    }
    if i.optional {
        parts.push(r#""optional": true"#.into());
    }
    format!("{{ {} }}", parts.join(", "))
}

/// Gradient pairs (top, bottom) in the family of the existing images.
const PALETTE: [(&str, &str); 6] = [
    ("#c1463a", "#8f2f27"),
    ("#c98a2b", "#9c6716"),
    ("#7a9a6d", "#566f4d"),
    ("#3a8fb0", "#286a85"),
    ("#6b7fb0", "#465579"),
    ("#cf7d2b", "#9e5a16"),
];

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A placeholder image like `images/*.svg`: gradient, plate icon, title and
/// cuisine. The colour pair is picked from the id, so it is stable across runs.
pub fn render_svg(r: &Recipe) -> String {
    let hash =
        r.id.bytes()
            .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let (top, bottom) = PALETTE[hash as usize % PALETTE.len()];
    let title = xml_escape(&r.title);
    let subtitle = xml_escape(r.cuisine.as_deref().unwrap_or(""));
    let font = "system-ui, -apple-system, Segoe UI, Roboto, sans-serif";
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360" role="img" aria-label="{title}">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{top}"/>
      <stop offset="1" stop-color="{bottom}"/>
    </linearGradient>
  </defs>
  <rect width="640" height="360" fill="url(#g)"/>
  <circle cx="320" cy="135" r="92" fill="#ffffff" opacity="0.10"/>
  <g fill="none" stroke="#ffffff" stroke-width="9" stroke-linecap="round" stroke-linejoin="round" opacity="0.92"><circle cx="320" cy="135" r="44"/><circle cx="320" cy="135" r="24"/><path d="M252 100 v70 M244 100 v22 a8 8 0 0 0 16 0 v-22"/><path d="M388 100 c14 10 14 34 0 42 v28"/></g>
  <text x="320" y="290" text-anchor="middle" font-family="{font}" font-size="38" font-weight="700" fill="#ffffff">{title}</text>
  <text x="320" y="324" text-anchor="middle" font-family="{font}" font-size="20" fill="#ffffff" opacity="0.85">{subtitle}</text>
</svg>
"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> IngredientCatalog {
        serde_json::from_str(
            r#"{
              "tomato":       { "name": "Tomato",       "unit": "g",   "category": "Produce", "count": { "per_piece": 120 } },
              "onion":        { "name": "Onion",        "unit": "g",   "category": "Produce", "count": { "per_piece": 150 } },
              "red_onion":    { "name": "Red onion",    "unit": "g",   "category": "Produce", "count": { "per_piece": 100 } },
              "garlic":       { "name": "Garlic",       "unit": "g",   "category": "Produce", "count": { "label": "clove", "per_piece": 4 } },
              "olives":       { "name": "Kalamata olives", "unit": "g", "category": "Produce" },
              "egg":          { "name": "Egg",          "unit": "pcs", "category": "Dairy & Eggs", "count": { "per_piece": 55 } },
              "lentils":      { "name": "Lentils",      "unit": "g",   "category": "Legumes" },
              "olive_oil":    { "name": "Olive oil",    "unit": "ml",  "category": "Spices & Oils" },
              "coconut_milk": { "name": "Coconut milk", "unit": "ml",  "category": "Pantry", "count": { "label": "can", "per_piece": 400 } },
              "salt":         { "name": "Salt",         "unit": "g",   "category": "Spices & Oils" }
            }"#,
        )
        .unwrap()
    }

    fn item(key: &str, amount: Option<f64>, unit: Option<&str>) -> RecipeIngredient {
        RecipeIngredient {
            unit: unit.map(Into::into),
            ..RecipeIngredient::new(key.into(), amount)
        }
    }

    fn ok(line: &str) -> RecipeIngredient {
        parse_line(line, &catalog()).unwrap_or_else(|e| panic!("{line}: {e}"))
    }

    #[test]
    fn spec_examples_parse() {
        // A bare number counts pieces.
        assert_eq!(ok("2 onion"), item("onion", Some(2.0), Some("pcs")));
        // Plural name, explicit unit.
        assert_eq!(ok("400 g tomatoes"), item("tomato", Some(400.0), None));
        assert_eq!(ok("0.4 kg tomatoes"), item("tomato", Some(0.4), Some("kg")));
        // No amount: to taste.
        assert_eq!(ok("salt"), item("salt", None, None));
    }

    #[test]
    fn piece_labels_units_and_names() {
        assert_eq!(
            ok("3 cloves garlic"),
            item("garlic", Some(3.0), Some("cloves"))
        );
        assert_eq!(
            ok("1 can coconut milk"),
            item("coconut_milk", Some(1.0), Some("can"))
        );
        assert_eq!(
            ok("2 tbsp olive oil"),
            item("olive_oil", Some(2.0), Some("tbsp"))
        );
        // "red" is neither a unit nor a label: it stays in the name.
        assert_eq!(ok("1 red onion"), item("red_onion", Some(1.0), Some("pcs")));
        // Shopped in pieces: the unit stays implicit.
        assert_eq!(ok("4 eggs"), item("egg", Some(4.0), None));
        // Display name and the plural key both match.
        assert_eq!(
            ok("100 g kalamata olives"),
            item("olives", Some(100.0), None)
        );
        assert_eq!(ok("50 grams olive"), item("olives", Some(50.0), None));
    }

    #[test]
    fn fractions_notes_and_optional() {
        assert_eq!(
            ok("1/2 can coconut milk"),
            item("coconut_milk", Some(0.5), Some("can"))
        );
        assert_eq!(
            ok("0,5 l olive oil"),
            item("olive_oil", Some(0.5), Some("l"))
        );
        let onion = ok("1 onion, finely chopped (Optional)");
        assert_eq!(onion.note.as_deref(), Some("finely chopped"));
        assert!(onion.optional);
        let salt = ok("salt (optional)");
        assert_eq!(salt.amount, None);
        assert!(salt.optional);
    }

    #[test]
    fn bad_lines_are_explained() {
        let c = catalog();
        let err = parse_line("300 lentils", &c).unwrap_err();
        assert!(err.contains("bare number counts pieces"), "{err}");
        let err = parse_line("1 cup salt", &c).unwrap_err();
        assert!(err.contains("does not convert"), "{err}");
        let err = parse_line("2", &c).unwrap_err();
        assert!(err.contains("no ingredient"), "{err}");

        let err = parse_line("2 zucchini", &c).unwrap_err();
        assert!(err.starts_with("unknown ingredient `zucchini`"), "{err}");
        assert!(err.contains(r#""zucchini": { "name": "Zucchini", "unit": "g", "category": "?", "count": { "per_piece": 100 } },"#), "{err}");
        let err = parse_line("2 sprigs rosemary", &c).unwrap_err();
        assert!(
            err.contains(r#""rosemary": { "name": "Rosemary", "unit": "g", "category": "?", "count": { "label": "sprig", "per_piece": 100 } },"#),
            "{err}"
        );
        let err = parse_line("200 g red lentil", &c).unwrap_err();
        assert!(err.contains("unknown ingredient `red lentil`"), "{err}");
        let err = parse_line("2 tbsp oil", &c).unwrap_err();
        assert!(
            err.contains("did you mean Olive oil (`olive_oil`)"),
            "{err}"
        );
    }

    const DRAFT: &str = "Lentil Dal\nauthor: Claude\nservings: 4\ncuisine: Indian\ntags: Dinner, vegetarian\nprep: 10 min\ncook: 30\n\n300 g lentils\n1 onion, finely chopped\nsalt\n\nSteps\n1. Soften the onion.\n2) Simmer the lentils.\n";

    #[test]
    fn a_draft_becomes_a_recipe() {
        let draft = parse_draft(DRAFT, &catalog()).unwrap();
        let r = &draft.recipe;
        assert!(draft.warnings.is_empty());
        assert_eq!(r.id, "lentil_dal");
        assert_eq!(r.title, "Lentil Dal");
        assert_eq!(r.author.as_deref(), Some("Claude"));
        assert_eq!(r.servings, 4);
        assert_eq!(r.image.as_deref(), Some("images/lentil_dal.svg"));
        assert_eq!(r.cuisine.as_deref(), Some("Indian"));
        assert_eq!(r.tags, vec!["dinner", "vegetarian"]);
        assert_eq!((r.prep_time, r.cook_time), (Some(10), Some(30)));
        assert_eq!(r.ingredients.len(), 3);
        assert_eq!(r.steps, vec!["Soften the onion.", "Simmer the lentils."]);
    }

    #[test]
    fn a_draft_without_headers_or_steps_gets_defaults() {
        let draft = parse_draft("Quick eggs\nauthor: Me\n2 eggs\nsalt\n", &catalog()).unwrap();
        assert_eq!(draft.recipe.servings, DEFAULT_SERVINGS);
        assert_eq!(draft.recipe.ingredients.len(), 2);
        assert_eq!(draft.recipe.steps, vec![TODO_STEP]);
        assert_eq!(draft.warnings.len(), 1);
    }

    #[test]
    fn draft_problems_are_all_reported_with_line_numbers() {
        let problems = parse_draft(
            "Soup\nservings: lots\nflavour: yes\nauthor: Me\n\n2 zucchini\nsalt\n1 cup salt\n",
            &catalog(),
        )
        .unwrap_err();
        let lines: Vec<usize> = problems.iter().map(|p| p.line).collect();
        assert_eq!(lines, vec![2, 3, 6, 8]);
        let problems = parse_draft("Soup\nsalt\n", &catalog()).unwrap_err();
        assert!(problems[0].message.contains("author"), "{problems:?}");
        assert!(parse_draft("", &catalog()).is_err());
        assert!(parse_draft("Title only\n", &catalog()).is_err());
    }

    #[test]
    fn rendered_json_parses_back_and_matches_house_style() {
        let recipe = parse_draft(DRAFT, &catalog()).unwrap().recipe;
        let json = render_json(&recipe);
        assert_eq!(serde_json::from_str::<Recipe>(&json).unwrap(), recipe);
        assert!(json.contains(
            "\n    { \"ingredient\": \"onion\", \"amount\": 1, \"unit\": \"pcs\", \"note\": \"finely chopped\" },\n"
        ));
        assert!(json.contains("\"tags\": [\"dinner\", \"vegetarian\"],"));
        assert!(!json.contains("null"));
        assert!(json.ends_with("}\n"));
    }

    #[test]
    fn slugs_and_svg_escaping() {
        assert_eq!(slug("Chicken & Spinach Curry!"), "chicken_spinach_curry");
        assert_eq!(slug("  Penne all'arrabbiata "), "penne_all_arrabbiata");
        let mut recipe = parse_draft(DRAFT, &catalog()).unwrap().recipe;
        recipe.title = "Mac & <Cheese>".into();
        let svg = render_svg(&recipe);
        assert!(svg.contains(">Mac &amp; &lt;Cheese&gt;</text>"));
        assert!(svg.contains(">Indian</text>"));
        assert_eq!(svg, render_svg(&recipe), "stable");
    }
}
