//! "Paste a recipe": turn recipe text copied from anywhere (a website, a
//! message, a note) into a [`RecipeForm`] to check and finish. Best effort and
//! never guessing a number: lines it recognises become picked ingredient rows,
//! the rest become rows that ask for a pick (keeping the amount and unit typed).
//! Pure: no Leptos, no browser access.

use crate::editor::{row_from_ingredient, IngredientRow, RecipeForm};
use crate::extras::parse_number;
use crate::models::IngredientCatalog;
use crate::quantity::Unit;
use crate::scaffold::{find_ingredient, parse_line, strip_numbering};

/// What a paste filled in, for the confirmation message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteSummary {
    pub ingredients: usize,
    /// Ingredient rows that still need a pick.
    pub to_pick: usize,
    pub steps: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Title,
    Intro,
    Ingredients,
    Steps,
}

/// Lines longer than this are never metadata ("Serves 4") or an ingredient.
const SHORT_LINE: usize = 60;

/// Fill a form from pasted text. Headers like "Ingredients" and "Method" are
/// followed when present; without them, the first line is the title, lines
/// that look like ingredients come next, and the rest are steps.
pub fn form_from_text(text: &str, catalog: &IngredientCatalog) -> (RecipeForm, PasteSummary) {
    let mut form = RecipeForm {
        ingredients: Vec::new(),
        steps: Vec::new(),
        ..RecipeForm::default()
    };
    let mut part = Part::Title;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match header(line) {
            Some(Part::Ingredients) => {
                part = Part::Ingredients;
                continue;
            }
            Some(Part::Steps) => {
                part = Part::Steps;
                continue;
            }
            _ => {}
        }
        if part != Part::Steps && read_metadata(line, &mut form) {
            if part == Part::Title {
                part = Part::Intro;
            }
            continue;
        }
        match part {
            Part::Title => {
                if looks_like_ingredient(line, catalog) {
                    part = Part::Ingredients;
                    form.ingredients.push(ingredient_row(line, catalog));
                } else {
                    form.title = clean_title(line);
                    part = Part::Intro;
                }
            }
            Part::Intro => {
                if looks_like_ingredient(line, catalog) {
                    part = Part::Ingredients;
                    form.ingredients.push(ingredient_row(line, catalog));
                } else if form.description.is_empty() && line.len() <= 240 {
                    form.description = line.to_string();
                }
            }
            Part::Ingredients => {
                // "For the dressing:" groups ingredients; the form has no groups.
                if line.ends_with(':') && line.len() <= SHORT_LINE {
                    continue;
                }
                if looks_like_step(line) && !looks_like_ingredient(line, catalog) {
                    part = Part::Steps;
                    push_step(&mut form, line);
                } else {
                    form.ingredients.push(ingredient_row(line, catalog));
                }
            }
            Part::Steps => push_step(&mut form, line),
        }
    }

    let summary = PasteSummary {
        ingredients: form.ingredients.len(),
        to_pick: form.ingredients.iter().filter(|r| r.key.is_none()).count(),
        steps: form.steps.len(),
    };
    if form.ingredients.is_empty() {
        form.ingredients.push(IngredientRow::default());
    }
    if form.steps.is_empty() {
        form.steps.push(String::new());
    }
    (form, summary)
}

/// "Ingredients", "## Method:", "INSTRUCTIONS" → the part they start.
fn header(line: &str) -> Option<Part> {
    let word = line
        .trim_matches(|c: char| c == '#' || c == '*' || c == ':' || c.is_whitespace())
        .to_lowercase();
    match word.as_str() {
        "ingredients" | "ingredient list" | "you will need" | "you'll need" | "what you need" => {
            Some(Part::Ingredients)
        }
        "method" | "steps" | "instructions" | "directions" | "preparation" | "how to make it"
        | "how to make" => Some(Part::Steps),
        _ => None,
    }
}

/// Servings, times and author lines; true when the line was one of them.
fn read_metadata(line: &str, form: &mut RecipeForm) -> bool {
    if line.len() > SHORT_LINE {
        return false;
    }
    let lower = line.to_lowercase();
    let starts = |words: &[&str]| words.iter().any(|w| lower.starts_with(w));
    if starts(&[
        "serves", "servings", "serving", "makes", "yield", "portions",
    ]) {
        if let Some(n) = first_number(&lower) {
            form.servings = n.clamp(1, 99);
            return true;
        }
    }
    if starts(&["prep"]) {
        if let Some(m) = duration_minutes(&lower) {
            form.prep = m.to_string();
            return true;
        }
    }
    if starts(&["cook", "bake", "baking"]) {
        if let Some(m) = duration_minutes(&lower) {
            form.cook = m.to_string();
            return true;
        }
    }
    if starts(&["total"]) && duration_minutes(&lower).is_some() {
        return true;
    }
    for prefix in ["author:", "recipe by ", "by "] {
        if lower.starts_with(prefix) && line.len() - prefix.len() <= 40 {
            form.author = line[prefix.len()..].trim().to_string();
            return true;
        }
    }
    false
}

fn first_number(s: &str) -> Option<u32> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let digits: String = s[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// "1 hour 10 mins" → 70, "25 minutes" → 25, "1.5 hours" → 90.
fn duration_minutes(s: &str) -> Option<u32> {
    let mut total = 0.0;
    let mut found = false;
    let tokens: Vec<&str> = s
        .split(|c: char| c.is_whitespace() || c == ':')
        .filter(|t| !t.is_empty())
        .collect();
    for (i, token) in tokens.iter().enumerate() {
        let digits_end = token
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
            .unwrap_or(token.len());
        let Some(n) = parse_number(&token[..digits_end]) else {
            continue;
        };
        let unit = if digits_end < token.len() {
            &token[digits_end..]
        } else {
            tokens.get(i + 1).copied().unwrap_or("")
        };
        found = true;
        total += if unit.starts_with('h') { n * 60.0 } else { n };
    }
    (found && total > 0.0).then(|| total.round() as u32)
}

fn clean_title(line: &str) -> String {
    line.trim_matches(|c: char| c == '#' || c == '*' || c.is_whitespace())
        .to_string()
}

/// Bullets and checkboxes that start list lines in copied recipes.
fn strip_bullet(line: &str) -> &str {
    line.trim_start_matches(|c: char| "-•*–·▢□☐◦".contains(c))
        .trim_start()
}

/// Unicode fractions as numbers, mixed numbers joined ("1½", "1 1/2" → "1.5"),
/// and units glued to their number split off ("300g" → "300 g").
fn normalize(line: &str) -> String {
    let mut text = String::new();
    for c in strip_bullet(line).chars() {
        let frac = match c {
            '½' => Some(0.5),
            '¼' => Some(0.25),
            '¾' => Some(0.75),
            '⅓' => Some(1.0 / 3.0),
            '⅔' => Some(2.0 / 3.0),
            '⅛' => Some(0.125),
            _ => None,
        };
        match frac {
            Some(f) => {
                let kept = text.trim_end_matches(' ').len();
                let digits = text[..kept]
                    .chars()
                    .rev()
                    .take_while(char::is_ascii_digit)
                    .count();
                let whole: f64 = text[kept - digits..kept].parse().unwrap_or(0.0);
                text.truncate(kept - digits);
                text.push_str(&trim_number(whole + f));
            }
            None => text.push(c),
        }
    }
    let mut tokens: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        // "300g" → "300" "g"
        let split = token.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',' || c == '/'));
        match split {
            Some(at) if at > 0 && Unit::parse(&token[at..]).is_some() => {
                tokens.push(token[..at].to_string());
                tokens.push(token[at..].to_string());
            }
            _ => tokens.push(token.to_string()),
        }
    }
    // "1 1/2" → "1.5"
    if let [whole, frac, ..] = tokens.as_slice() {
        if let (Ok(w), Some((a, b))) = (whole.parse::<u32>(), frac.split_once('/')) {
            if let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>()) {
                if b > 0.0 {
                    let joined = trim_number(f64::from(w) + a / b);
                    tokens.splice(0..2, [joined]);
                }
            }
        }
    }
    tokens.join(" ")
}

fn trim_number(n: f64) -> String {
    let s = format!("{:.3}", n);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn starts_with_amount(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|t| t.starts_with(|c: char| c.is_ascii_digit()))
}

fn looks_like_ingredient(line: &str, catalog: &IngredientCatalog) -> bool {
    let normalized = normalize(line);
    line.len() <= SHORT_LINE
        && (starts_with_amount(&normalized)
            || strip_bullet(line).len() < line.len()
            || parse_line(&normalized, catalog).is_ok())
}

fn looks_like_step(line: &str) -> bool {
    let body = strip_numbering(line);
    body.len() > SHORT_LINE || body.ends_with('.') || body.ends_with('!')
}

fn push_step(form: &mut RecipeForm, line: &str) {
    let body = strip_numbering(strip_bullet(line));
    // A bare "Step 2" line introduces the next one.
    let lower = body.to_lowercase();
    if lower.starts_with("step")
        && lower[4..]
            .trim()
            .chars()
            .all(|c| c.is_ascii_digit() || c == ':' || c == '.')
    {
        return;
    }
    if !body.is_empty() {
        form.steps.push(body.to_string());
    }
}

/// One ingredient line as a form row: picked when the catalog knows it,
/// otherwise kept as typed for the person to pick.
fn ingredient_row(line: &str, catalog: &IngredientCatalog) -> IngredientRow {
    let normalized = normalize(line);
    let mut row = parse_row(&normalized, catalog);
    // An empty amount already reads "to taste".
    if row.amount.is_empty() && row.note.eq_ignore_ascii_case("to taste") {
        row.note.clear();
    }
    row
}

fn parse_row(normalized: &str, catalog: &IngredientCatalog) -> IngredientRow {
    if let Ok(item) = parse_line(normalized, catalog) {
        return row_from_ingredient(&item, catalog);
    }

    // Split by hand: optional, note after a comma, amount, unit, name.
    let mut body = normalized;
    let mut optional = false;
    let lower = body.to_lowercase();
    if lower.ends_with("(optional)") {
        body = body[..body.len() - "(optional)".len()].trim_end();
        optional = true;
    }
    let (body, mut note) = match body.split_once(',') {
        Some((b, n)) => (b.trim(), n.trim().to_string()),
        None => (body, String::new()),
    };
    let mut tokens: Vec<&str> = body.split_whitespace().collect();
    let amount = match tokens.first() {
        Some(t) if crate::editor::parse_amount(t).is_some() => tokens.remove(0).to_string(),
        _ => String::new(),
    };
    let mut unit = match tokens.first() {
        Some(t) if !amount.is_empty() && tokens.len() > 1 && Unit::parse(t).is_some() => {
            tokens.remove(0).to_string()
        }
        _ => String::new(),
    };

    // "red lentils" → lentils (note "red"); "cloves fresh garlic" → garlic in cloves.
    for start in 0..tokens.len() {
        let name = tokens[start..].join(" ");
        if let Some((key, info)) = find_ingredient(catalog, &name) {
            let mut extra: Vec<&str> = tokens[..start].to_vec();
            if unit.is_empty() && !amount.is_empty() {
                if let Some(first) = extra.first() {
                    // British "tin" is the catalog's "can".
                    let word = match first.to_lowercase().as_str() {
                        "tin" | "tins" => "can".to_string(),
                        w => w.to_string(),
                    };
                    if info.count.as_ref().is_some_and(|c| c.matches_label(&word)) {
                        unit = info
                            .count
                            .as_ref()
                            .and_then(|c| c.label.clone())
                            .unwrap_or_default();
                        extra.remove(0);
                    }
                }
            }
            if !extra.is_empty() {
                let words = extra.join(" ");
                note = if note.is_empty() {
                    words
                } else {
                    format!("{words}, {note}")
                };
            }
            let unit = if unit.is_empty() && !amount.is_empty() {
                crate::editor::default_unit(info)
            } else {
                unit
            };
            return IngredientRow {
                name: info.name.clone(),
                key: Some(key.to_string()),
                amount,
                unit,
                note,
                optional,
            };
        }
    }
    IngredientRow {
        name: tokens.join(" "),
        key: None,
        amount,
        unit,
        note,
        optional,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::load_catalog;

    fn paste(text: &str) -> (RecipeForm, PasteSummary) {
        form_from_text(text, &load_catalog())
    }

    #[test]
    fn a_web_recipe_with_headers() {
        let (form, summary) = paste(
            "## Creamy Lentil Soup\n\
             A cosy weeknight soup.\n\
             Serves 4\n\
             Prep time: 10 mins\n\
             Cook time: 1 hour 5 minutes\n\
             \n\
             Ingredients\n\
             • 300g red lentils\n\
             • 1 large onion, finely chopped\n\
             • 2 cloves fresh garlic\n\
             • 1½ tbsp olive oil\n\
             • 1 tin coconut milk\n\
             • 1 cup flour\n\
             • Salt, to taste\n\
             • 2 star anise\n\
             \n\
             Method\n\
             Step 1\n\
             Heat the oil and soften the onion and garlic.\n\
             2. Add the lentils and simmer for 25 minutes.\n",
        );
        assert_eq!(form.title, "Creamy Lentil Soup");
        assert_eq!(form.description, "A cosy weeknight soup.");
        assert_eq!(form.servings, 4);
        assert_eq!((form.prep.as_str(), form.cook.as_str()), ("10", "65"));
        assert_eq!(
            summary,
            PasteSummary {
                ingredients: 8,
                to_pick: 1,
                steps: 2
            }
        );
        let rows = &form.ingredients;
        let lentils = &rows[0];
        assert_eq!(
            (
                lentils.key.as_deref(),
                lentils.amount.as_str(),
                lentils.unit.as_str(),
                lentils.note.as_str()
            ),
            (Some("lentils"), "300", "g", "red")
        );
        assert_eq!(
            (rows[1].key.as_deref(), rows[1].note.as_str()),
            (Some("onion"), "large, finely chopped")
        );
        assert_eq!(
            (
                rows[2].key.as_deref(),
                rows[2].unit.as_str(),
                rows[2].note.as_str()
            ),
            (Some("garlic"), "clove", "fresh")
        );
        assert_eq!(
            (rows[3].key.as_deref(), rows[3].amount.as_str()),
            (Some("olive_oil"), "1.5")
        );
        assert_eq!(
            (
                rows[4].key.as_deref(),
                rows[4].unit.as_str(),
                rows[4].note.as_str()
            ),
            (Some("coconut_milk"), "can", "")
        );
        // A cup of flour can't be shopped (no density): picked, the unit asked about.
        assert_eq!(
            (rows[5].key.as_deref(), rows[5].unit.as_str()),
            (Some("flour"), "cup")
        );
        assert_eq!(
            (
                rows[6].key.as_deref(),
                rows[6].amount.as_str(),
                rows[6].note.as_str()
            ),
            (Some("salt"), "", "")
        );
        assert_eq!(
            (
                rows[7].key.as_deref(),
                rows[7].name.as_str(),
                rows[7].amount.as_str()
            ),
            (None, "star anise", "2")
        );
        assert_eq!(
            form.steps,
            [
                "Heat the oil and soften the onion and garlic.",
                "Add the lentils and simmer for 25 minutes."
            ]
        );
    }

    #[test]
    fn a_note_without_headers() {
        let (form, summary) = paste(
            "Pasta with peas\n\
             by Nonna\n\
             200 g spaghetti\n\
             150g peas\n\
             parmesan\n\
             Boil the pasta, add the peas for the last 3 minutes.\n\
             Toss with parmesan\n",
        );
        assert_eq!(form.title, "Pasta with peas");
        assert_eq!(form.author, "Nonna");
        assert_eq!(summary.ingredients, 3);
        assert_eq!(summary.to_pick, 0);
        assert_eq!(form.steps.len(), 2);
    }

    #[test]
    fn nothing_recognisable_still_gives_a_usable_form() {
        let (form, summary) = paste("");
        assert_eq!(summary, PasteSummary::default());
        assert_eq!(form.ingredients.len(), 1);
        assert_eq!(form.steps.len(), 1);
    }

    #[test]
    fn numbers_are_normalized() {
        assert_eq!(normalize("1 1/2 cups flour"), "1.5 cups flour");
        assert_eq!(normalize("½ tsp salt"), "0.5 tsp salt");
        assert_eq!(normalize("2¼kg potatoes"), "2.25 kg potatoes");
        assert_eq!(normalize("- 300g lentils"), "300 g lentils");
        assert_eq!(duration_minutes("cook: 1.5 hours"), Some(90));
        assert_eq!(duration_minutes("cook the onion"), None);
    }
}
