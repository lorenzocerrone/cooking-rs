//! Pure logic for the grocery page's free-text "extras" line: parse what the
//! user typed, suggest matching catalog ingredients, and build the [`Extra`]
//! to store. No Leptos types.

use crate::models::{CatalogIngredient, Extra, IngredientCatalog, RecipeIngredient};
use crate::quantity::resolve_unit;

/// A typed extra, split into the item name and an optional quantity text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedExtra {
    pub name: String,
    pub quantity: Option<String>,
}

/// Split `"milk 1 l"` into `milk` / `"1 l"`. The first numeric token starts the
/// quantity; the token after it joins the quantity unless that would leave no
/// name (`"2 eggs"` → `eggs` / `"2"`). Decimal commas are accepted.
pub fn parse_extra(input: &str) -> ParsedExtra {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let Some(num_at) = tokens.iter().position(|t| is_number(t)) else {
        return ParsedExtra {
            name: tokens.join(" "),
            quantity: None,
        };
    };
    let mut name: Vec<&str> = tokens[..num_at].to_vec();
    let mut quantity: Vec<&str> = vec![tokens[num_at]];
    let mut rest: Vec<&str> = tokens[num_at + 1..].to_vec();
    if !rest.is_empty() && (!name.is_empty() || rest.len() > 1) {
        quantity.push(rest.remove(0));
    }
    name.extend(rest);
    ParsedExtra {
        name: name.join(" "),
        quantity: Some(quantity.join(" ")),
    }
}

fn is_number(token: &str) -> bool {
    parse_number(token).is_some()
}

fn parse_number(token: &str) -> Option<f64> {
    token
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

/// A catalog ingredient offered while typing.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub key: String,
    pub name: String,
    pub category: String,
}

/// Catalog ingredients matching `name` (case-insensitive): names starting with
/// it first, then names containing it, each group sorted by name. Empty input
/// gives nothing.
pub fn suggest(catalog: &IngredientCatalog, name: &str, max: usize) -> Vec<Suggestion> {
    let needle = name.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut prefix = Vec::new();
    let mut inner = Vec::new();
    for (key, info) in &catalog.0 {
        let lower = info.name.to_lowercase();
        let s = Suggestion {
            key: key.clone(),
            name: info.name.clone(),
            category: info.category.clone(),
        };
        if lower.starts_with(&needle) {
            prefix.push(s);
        } else if lower.contains(&needle) {
            inner.push(s);
        }
    }
    prefix.sort_by(|a, b| a.name.cmp(&b.name));
    inner.sort_by(|a, b| a.name.cmp(&b.name));
    prefix.into_iter().chain(inner).take(max).collect()
}

/// Build the extra to store. With a catalog pick, the quantity becomes an amount
/// in a unit that converts for that ingredient; if it doesn't (`"1 loaf"` for
/// bread sold by weight) the item is kept as typed instead, so nothing is
/// silently mis-measured. Without a pick, it is a custom item.
pub fn to_extra(parsed: &ParsedExtra, pick: Option<(&str, &CatalogIngredient)>) -> Extra {
    if let Some((key, info)) = pick {
        match parsed.quantity.as_deref() {
            None => return Extra::Catalog(RecipeIngredient::new(key.to_string(), None)),
            Some(q) => {
                let mut parts = q.split_whitespace();
                let amount = parts.next().and_then(parse_number);
                let unit = parts.next();
                let resolves = parts.next().is_none()
                    && resolve_unit(unit, info.unit, info.count.as_ref()).is_ok();
                if let (Some(amount), true) = (amount, resolves) {
                    // The unit text is kept as typed ("loaves"); it resolved, so
                    // the list can convert it.
                    return Extra::Catalog(RecipeIngredient {
                        ingredient: key.to_string(),
                        amount: Some(amount),
                        unit: unit.map(str::to_string),
                        note: None,
                        optional: false,
                    });
                }
                return Extra::Custom {
                    name: info.name.clone(),
                    quantity: Some(q.to_string()),
                };
            }
        }
    }
    Extra::Custom {
        name: capitalize(parsed.name.trim()),
        quantity: parsed.quantity.clone(),
    }
}

/// "bread" → "Bread" (first character only; the rest is kept as typed).
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantity::{CountSpec, Unit};
    use std::collections::HashMap;

    fn parsed(name: &str, quantity: Option<&str>) -> ParsedExtra {
        ParsedExtra {
            name: name.into(),
            quantity: quantity.map(Into::into),
        }
    }

    #[test]
    fn parsing_splits_name_and_quantity() {
        assert_eq!(parse_extra("milk 1 l"), parsed("milk", Some("1 l")));
        assert_eq!(parse_extra("2 eggs"), parsed("eggs", Some("2")));
        assert_eq!(parse_extra("bread"), parsed("bread", None));
        assert_eq!(parse_extra("bread 1 loaf"), parsed("bread", Some("1 loaf")));
        assert_eq!(parse_extra("1 loaf bread"), parsed("bread", Some("1 loaf")));
        assert_eq!(
            parse_extra("  olive   oil 0,5 l "),
            parsed("olive oil", Some("0,5 l"))
        );
        assert_eq!(parse_extra("coffee 250"), parsed("coffee", Some("250")));
        assert_eq!(parse_extra(""), parsed("", None));
    }

    fn catalog() -> IngredientCatalog {
        let mut m = HashMap::new();
        let entry =
            |name: &str, unit: Unit, category: &str, count: Option<CountSpec>| CatalogIngredient {
                name: name.into(),
                unit,
                category: category.into(),
                count,
            };
        m.insert("milk".into(), entry("Milk", Unit::Ml, "Dairy & Eggs", None));
        m.insert(
            "bread".into(),
            entry(
                "Bread",
                Unit::Piece,
                "Bakery",
                Some(CountSpec {
                    label: Some("loaf".into()),
                    plural: Some("loaves".into()),
                    per_piece: None,
                }),
            ),
        );
        m.insert(
            "breadcrumbs".into(),
            entry("Breadcrumbs", Unit::G, "Pantry", None),
        );
        m.insert("flour".into(), entry("Flour", Unit::G, "Pantry", None));
        IngredientCatalog(m)
    }

    #[test]
    fn suggestions_prefer_prefix_matches_and_cap_the_count() {
        let c = catalog();
        let names = |q: &str, max: usize| -> Vec<String> {
            suggest(&c, q, max).into_iter().map(|s| s.name).collect()
        };
        assert_eq!(names("bre", 5), vec!["Bread", "Breadcrumbs"]);
        assert_eq!(names("crumb", 5), vec!["Breadcrumbs"]);
        assert_eq!(names("B", 1), vec!["Bread"]);
        assert_eq!(names("  ", 5), Vec::<String>::new());
        assert_eq!(names("zzz", 5), Vec::<String>::new());
        assert_eq!(suggest(&c, "milk", 5)[0].category, "Dairy & Eggs");
    }

    #[test]
    fn catalog_pick_resolves_units_or_falls_back_to_custom() {
        let c = catalog();
        let milk = ("milk", c.get("milk").unwrap());
        let bread = ("bread", c.get("bread").unwrap());
        let flour = ("flour", c.get("flour").unwrap());

        assert_eq!(
            to_extra(&parsed("milk", Some("1 l")), Some(milk)),
            Extra::Catalog(RecipeIngredient {
                ingredient: "milk".into(),
                amount: Some(1.0),
                unit: Some("l".into()),
                note: None,
                optional: false,
            })
        );
        // No unit: the catalog's shopping unit.
        assert_eq!(
            to_extra(&parsed("flour", Some("500")), Some(flour)),
            Extra::Catalog(RecipeIngredient::new("flour".into(), Some(500.0)))
        );
        // Piece labels resolve too.
        assert_eq!(
            to_extra(&parsed("bread", Some("2 loaves")), Some(bread)),
            Extra::Catalog(RecipeIngredient {
                ingredient: "bread".into(),
                amount: Some(2.0),
                unit: Some("loaves".into()),
                note: None,
                optional: false,
            })
        );
        // No quantity at all: a "to taste"-style catalog line.
        assert_eq!(
            to_extra(&parsed("milk", None), Some(milk)),
            Extra::Catalog(RecipeIngredient::new("milk".into(), None))
        );
        // A unit the ingredient can't convert keeps the text, under the catalog name.
        assert_eq!(
            to_extra(&parsed("flour", Some("1 bag")), Some(flour)),
            Extra::Custom {
                name: "Flour".into(),
                quantity: Some("1 bag".into()),
            }
        );
        // No pick: custom, name trimmed and capitalized.
        assert_eq!(
            to_extra(&parsed(" candles ", Some("3")), None),
            Extra::Custom {
                name: "Candles".into(),
                quantity: Some("3".into()),
            }
        );
        assert_eq!(
            to_extra(&parsed("éclair", None), None),
            Extra::Custom {
                name: "Éclair".into(),
                quantity: None,
            }
        );
    }
}
