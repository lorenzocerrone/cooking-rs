//! Pure recipe search/filtering. No Leptos types — the recipes page and the
//! picker build a [`RecipeQuery`] from the filter sheet and call [`filter`].
//!
//! Matching: author, cuisine and diet match **any** selected value (a recipe has
//! one of each); tags match **all** selected (they are attributes: "quick" and
//! "dinner" means quick dinners); a time cap keeps recipes at or under it; the
//! sections combine with AND. On the recipes page the query is carried in the
//! URL's query string ([`RecipeQuery::to_query_string`] / [`RecipeQuery::from_query_string`]).

use std::collections::BTreeSet;

use crate::format::total_time;
use crate::models::{IngredientCatalog, Recipe};

/// The tag that marks a recipe vegetarian. Validation keeps it in step with the
/// ingredients (no Meat or Fish aisle), so the Diet filter can read it.
pub const VEGETARIAN_TAG: &str = "vegetarian";

/// Aisles whose ingredients make a recipe not vegetarian.
pub const MEAT_AISLES: [&str; 2] = ["Meat", "Fish"];

/// The recipe's ingredients that come from a [`MEAT_AISLES`] aisle. The
/// scaffolder sets [`VEGETARIAN_TAG`] from this and validation checks it.
pub fn meat_ingredients<'a>(recipe: &'a Recipe, catalog: &IngredientCatalog) -> Vec<&'a str> {
    recipe
        .ingredients
        .iter()
        .filter(|i| {
            catalog
                .get(&i.ingredient)
                .is_some_and(|info| MEAT_AISLES.contains(&info.category.as_str()))
        })
        .map(|i| i.ingredient.as_str())
        .collect()
}

/// The time caps offered, in minutes of prep + cook.
pub const TIME_CAPS: [u32; 2] = [20, 40];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Diet {
    Vegetarian,
    /// Contains meat or fish.
    Meat,
}

impl Diet {
    pub const ALL: [Diet; 2] = [Diet::Vegetarian, Diet::Meat];

    pub fn label(self) -> &'static str {
        match self {
            Diet::Vegetarian => "vegetarian",
            Diet::Meat => "meat & fish",
        }
    }

    fn param(self) -> &'static str {
        match self {
            Diet::Vegetarian => "veg",
            Diet::Meat => "meat",
        }
    }

    fn of(recipe: &Recipe) -> Diet {
        if recipe.tags.iter().any(|t| t == VEGETARIAN_TAG) {
            Diet::Vegetarian
        } else {
            Diet::Meat
        }
    }
}

/// The active search + filter selections. Empty fields mean "no constraint".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecipeQuery {
    /// Free-text search over title + description (case-insensitive).
    pub search: String,
    pub diet: BTreeSet<Diet>,
    pub authors: BTreeSet<String>,
    pub cuisines: BTreeSet<String>,
    /// All of these tags are required.
    pub tags: BTreeSet<String>,
    /// Keep recipes whose total time is at most this many minutes.
    pub max_minutes: Option<u32>,
}

/// One selectable value in the filter sheet (or one active-filter chip).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterOption {
    Diet(Diet),
    Author(String),
    Cuisine(String),
    Tag(String),
    MaxMinutes(u32),
}

impl FilterOption {
    /// The chip label ("by Lorenzo", "≤20 min").
    pub fn label(&self) -> String {
        match self {
            FilterOption::Diet(d) => d.label().to_string(),
            FilterOption::Author(a) => format!("by {a}"),
            FilterOption::Cuisine(c) | FilterOption::Tag(c) => c.clone(),
            FilterOption::MaxMinutes(m) => format!("≤{m} min"),
        }
    }
}

impl RecipeQuery {
    fn matches(&self, recipe: &Recipe) -> bool {
        let q = self.search.trim().to_lowercase();
        let text_ok = q.is_empty()
            || recipe.title.to_lowercase().contains(&q)
            || recipe
                .description
                .as_deref()
                .is_some_and(|d| d.to_lowercase().contains(&q));
        let diet_ok = self.diet.is_empty() || self.diet.contains(&Diet::of(recipe));
        let author_ok = self.authors.is_empty()
            || recipe
                .author
                .as_ref()
                .is_some_and(|a| self.authors.contains(a));
        let cuisine_ok = self.cuisines.is_empty()
            || recipe
                .cuisine
                .as_ref()
                .is_some_and(|c| self.cuisines.contains(c));
        let tags_ok = self.tags.iter().all(|t| recipe.tags.contains(t));
        let time_ok = self
            .max_minutes
            .is_none_or(|cap| total_time(recipe).is_some_and(|m| m <= cap));
        text_ok && diet_ok && author_ok && cuisine_ok && tags_ok && time_ok
    }

    pub fn is_on(&self, option: &FilterOption) -> bool {
        match option {
            FilterOption::Diet(d) => self.diet.contains(d),
            FilterOption::Author(a) => self.authors.contains(a),
            FilterOption::Cuisine(c) => self.cuisines.contains(c),
            FilterOption::Tag(t) => self.tags.contains(t),
            FilterOption::MaxMinutes(m) => self.max_minutes == Some(*m),
        }
    }

    /// Select or deselect `option` (a time cap replaces any other cap).
    pub fn toggle(&mut self, option: &FilterOption) {
        fn flip<T: Ord + Clone>(set: &mut BTreeSet<T>, v: &T) {
            if !set.remove(v) {
                set.insert(v.clone());
            }
        }
        match option {
            FilterOption::Diet(d) => flip(&mut self.diet, d),
            FilterOption::Author(a) => flip(&mut self.authors, a),
            FilterOption::Cuisine(c) => flip(&mut self.cuisines, c),
            FilterOption::Tag(t) => flip(&mut self.tags, t),
            FilterOption::MaxMinutes(m) => {
                self.max_minutes = (self.max_minutes != Some(*m)).then_some(*m)
            }
        }
    }

    /// The selected options, in sheet order (the active-filter chips).
    pub fn active(&self) -> Vec<FilterOption> {
        let mut out: Vec<FilterOption> = self.diet.iter().map(|d| FilterOption::Diet(*d)).collect();
        out.extend(self.authors.iter().cloned().map(FilterOption::Author));
        out.extend(self.cuisines.iter().cloned().map(FilterOption::Cuisine));
        out.extend(self.max_minutes.map(FilterOption::MaxMinutes));
        out.extend(self.tags.iter().cloned().map(FilterOption::Tag));
        out
    }

    /// Number of selected options (the "Filters · N" badge).
    pub fn active_count(&self) -> usize {
        self.diet.len()
            + self.authors.len()
            + self.cuisines.len()
            + self.tags.len()
            + usize::from(self.max_minutes.is_some())
    }

    /// Drop every filter, keeping the search text.
    pub fn clear_filters(&mut self) {
        *self = RecipeQuery {
            search: std::mem::take(&mut self.search),
            ..Default::default()
        };
    }

    /// The query as a URL query string ("?diet=veg&by=Claude,Lorenzo"), or ""
    /// when nothing is set. Multiple values are comma-separated.
    pub fn to_query_string(&self) -> String {
        let join = |values: Vec<&str>| {
            values
                .into_iter()
                .map(percent_encode)
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut parts = Vec::new();
        if !self.search.trim().is_empty() {
            parts.push(format!("q={}", percent_encode(self.search.trim())));
        }
        if !self.diet.is_empty() {
            parts.push(format!(
                "diet={}",
                join(self.diet.iter().map(|d| d.param()).collect())
            ));
        }
        for (key, set) in [
            ("by", &self.authors),
            ("cuisine", &self.cuisines),
            ("tag", &self.tags),
        ] {
            if !set.is_empty() {
                parts.push(format!(
                    "{key}={}",
                    join(set.iter().map(String::as_str).collect())
                ));
            }
        }
        if let Some(m) = self.max_minutes {
            parts.push(format!("time={m}"));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }

    /// Parse [`to_query_string`](Self::to_query_string)'s output (leading `?`
    /// optional). Unknown keys and unreadable values are ignored.
    pub fn from_query_string(raw: &str) -> RecipeQuery {
        let mut q = RecipeQuery::default();
        for pair in raw.trim_start_matches('?').split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let values = || {
                value
                    .split(',')
                    .filter_map(percent_decode)
                    .filter(|v| !v.is_empty())
            };
            match key {
                "q" => q.search = percent_decode(value).unwrap_or_default(),
                "diet" => {
                    q.diet = values()
                        .filter_map(|v| Diet::ALL.into_iter().find(|d| d.param() == v))
                        .collect()
                }
                "by" => q.authors = values().collect(),
                "cuisine" => q.cuisines = values().collect(),
                "tag" => q.tags = values().collect(),
                "time" => q.max_minutes = value.parse().ok().filter(|m| *m > 0),
                _ => {}
            }
        }
        q
    }
}

/// The recipes matching `query`, preserving input order.
pub fn filter<'a>(recipes: &'a [Recipe], query: &RecipeQuery) -> Vec<&'a Recipe> {
    recipes.iter().filter(|r| query.matches(r)).collect()
}

/// The count shown next to an option: the recipes with that value that pass the
/// rest of the query. For the any-of sections and the time cap, the option
/// stands in for its section's selection; a tag is added to the selected tags.
pub fn option_count(recipes: &[Recipe], query: &RecipeQuery, option: &FilterOption) -> usize {
    let mut q = query.clone();
    match option {
        FilterOption::Diet(d) => q.diet = BTreeSet::from([*d]),
        FilterOption::Author(a) => q.authors = BTreeSet::from([a.clone()]),
        FilterOption::Cuisine(c) => q.cuisines = BTreeSet::from([c.clone()]),
        FilterOption::Tag(t) => {
            q.tags.insert(t.clone());
        }
        FilterOption::MaxMinutes(m) => q.max_minutes = Some(*m),
    }
    recipes.iter().filter(|r| q.matches(r)).count()
}

/// The values present in the recipe set, for the filter sheet's sections.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facets {
    pub authors: Vec<String>,
    pub cuisines: Vec<String>,
    /// Every tag except [`VEGETARIAN_TAG`] (that one is the Diet section).
    pub tags: Vec<String>,
}

impl Facets {
    pub fn of(recipes: &[Recipe]) -> Self {
        Facets {
            authors: sorted_unique(recipes.iter().filter_map(|r| r.author.clone())),
            cuisines: sorted_unique(recipes.iter().filter_map(|r| r.cuisine.clone())),
            tags: sorted_unique(
                recipes
                    .iter()
                    .flat_map(|r| r.tags.iter().cloned())
                    .filter(|t| t != VEGETARIAN_TAG),
            ),
        }
    }
}

fn sorted_unique(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = items.collect();
    v.sort();
    v.dedup();
    v
}

/// Percent-encode everything but RFC 3986 unreserved characters.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Undo [`percent_encode`] (also reading `+` as a space). `None` if malformed.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(
        id: &str,
        title: &str,
        cuisine: Option<&str>,
        tags: &[&str],
        desc: Option<&str>,
    ) -> Recipe {
        Recipe {
            id: id.into(),
            title: title.into(),
            servings: 1,
            ingredients: vec![],
            steps: vec![],
            description: desc.map(|s| s.into()),
            cuisine: cuisine.map(|s| s.into()),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            prep_time: None,
            cook_time: None,
            image: None,
            author: None,
        }
    }

    fn sample() -> Vec<Recipe> {
        let mut carbonara = recipe(
            "carbonara",
            "Spaghetti Carbonara",
            Some("Italian"),
            &["pasta", "quick", "dinner"],
            Some("Roman classic"),
        );
        carbonara.author = Some("Claude".into());
        carbonara.prep_time = Some(10);
        carbonara.cook_time = Some(15);
        let mut salad = recipe(
            "salad",
            "Greek Salad",
            Some("Greek"),
            &["salad", "quick", "vegetarian"],
            Some("Crunchy and fresh"),
        );
        salad.author = Some("Lorenzo".into());
        salad.prep_time = Some(15);
        let mut oats = recipe(
            "oats",
            "Overnight Oats",
            Some("American"),
            &["breakfast", "vegetarian"],
            None,
        );
        oats.author = Some("Lorenzo".into());
        let mut dal = recipe(
            "dal",
            "Lentil Dal",
            Some("Indian"),
            &["dinner", "vegetarian"],
            None,
        );
        dal.author = Some("Anna".into());
        dal.prep_time = Some(10);
        dal.cook_time = Some(30);
        vec![carbonara, salad, oats, dal]
    }

    fn ids(recipes: &[Recipe], q: &RecipeQuery) -> Vec<String> {
        filter(recipes, q).iter().map(|r| r.id.clone()).collect()
    }

    fn query(options: &[FilterOption]) -> RecipeQuery {
        let mut q = RecipeQuery::default();
        for o in options {
            q.toggle(o);
        }
        q
    }

    #[test]
    fn empty_query_returns_all() {
        let r = sample();
        assert_eq!(filter(&r, &RecipeQuery::default()).len(), 4);
    }

    #[test]
    fn search_matches_title_and_description() {
        let r = sample();
        let by_title = RecipeQuery {
            search: "greek".into(),
            ..Default::default()
        };
        assert_eq!(ids(&r, &by_title), ["salad"]);
        let by_desc = RecipeQuery {
            search: " roman ".into(),
            ..Default::default()
        };
        assert_eq!(ids(&r, &by_desc), ["carbonara"]);
    }

    #[test]
    fn authors_cuisines_and_diet_match_any_selected_value() {
        let r = sample();
        let q = query(&[
            FilterOption::Author("Lorenzo".into()),
            FilterOption::Author("Anna".into()),
        ]);
        assert_eq!(ids(&r, &q), ["salad", "oats", "dal"]);
        let q = query(&[
            FilterOption::Cuisine("Italian".into()),
            FilterOption::Cuisine("Indian".into()),
        ]);
        assert_eq!(ids(&r, &q), ["carbonara", "dal"]);
        assert_eq!(
            ids(&r, &query(&[FilterOption::Diet(Diet::Meat)])),
            ["carbonara"]
        );
        let both = query(&[
            FilterOption::Diet(Diet::Meat),
            FilterOption::Diet(Diet::Vegetarian),
        ]);
        assert_eq!(filter(&r, &both).len(), 4);
    }

    #[test]
    fn tags_require_all_selected_and_sections_combine() {
        let r = sample();
        let quick_dinner = query(&[
            FilterOption::Tag("quick".into()),
            FilterOption::Tag("dinner".into()),
        ]);
        assert_eq!(ids(&r, &quick_dinner), ["carbonara"]);
        let veg_by_lorenzo_quick = query(&[
            FilterOption::Diet(Diet::Vegetarian),
            FilterOption::Author("Lorenzo".into()),
            FilterOption::Tag("quick".into()),
        ]);
        assert_eq!(ids(&r, &veg_by_lorenzo_quick), ["salad"]);
        let with_search = RecipeQuery {
            search: "oats".into(),
            ..veg_by_lorenzo_quick
        };
        assert!(ids(&r, &with_search).is_empty());
    }

    #[test]
    fn time_cap_keeps_timed_recipes_at_or_under_it() {
        let r = sample();
        // Oats have no time set and so never pass a cap.
        assert_eq!(ids(&r, &query(&[FilterOption::MaxMinutes(20)])), ["salad"]);
        assert_eq!(
            ids(&r, &query(&[FilterOption::MaxMinutes(40)])),
            ["carbonara", "salad", "dal"]
        );
        // A second cap replaces the first; toggling it again clears it.
        let mut q = query(&[FilterOption::MaxMinutes(20), FilterOption::MaxMinutes(40)]);
        assert_eq!(q.max_minutes, Some(40));
        q.toggle(&FilterOption::MaxMinutes(40));
        assert_eq!(q.max_minutes, None);
    }

    #[test]
    fn option_counts_are_per_value_given_the_other_sections() {
        let r = sample();
        let veg = query(&[FilterOption::Diet(Diet::Vegetarian)]);
        assert_eq!(
            option_count(&r, &veg, &FilterOption::Tag("dinner".into())),
            1
        );
        assert_eq!(
            option_count(&r, &veg, &FilterOption::Cuisine("Italian".into())),
            0
        );
        // A selected option counts its own recipes, and so do its neighbours.
        assert_eq!(
            option_count(&r, &veg, &FilterOption::Diet(Diet::Vegetarian)),
            3
        );
        assert_eq!(option_count(&r, &veg, &FilterOption::Diet(Diet::Meat)), 1);
        let lorenzo = query(&[FilterOption::Author("Lorenzo".into())]);
        assert_eq!(
            option_count(&r, &lorenzo, &FilterOption::Author("Anna".into())),
            1
        );
        assert_eq!(
            option_count(&r, &lorenzo, &FilterOption::Author("Lorenzo".into())),
            2
        );
        // A time cap stands in for the selected one.
        let capped = query(&[FilterOption::MaxMinutes(20)]);
        assert_eq!(option_count(&r, &capped, &FilterOption::MaxMinutes(40)), 3);
    }

    #[test]
    fn active_count_chips_and_clear_keep_the_search() {
        let mut q = query(&[
            FilterOption::Tag("quick".into()),
            FilterOption::MaxMinutes(20),
            FilterOption::Diet(Diet::Vegetarian),
        ]);
        q.search = "salad".into();
        assert_eq!(q.active_count(), 3);
        let labels: Vec<String> = q.active().iter().map(FilterOption::label).collect();
        assert_eq!(labels, ["vegetarian", "≤20 min", "quick"]);
        q.clear_filters();
        assert_eq!(q.active_count(), 0);
        assert_eq!(q.search, "salad");
    }

    #[test]
    fn query_string_round_trips_and_ignores_garbage() {
        let mut q = query(&[
            FilterOption::Diet(Diet::Vegetarian),
            FilterOption::Author("Anna Maria".into()),
            FilterOption::Author("Lorenzo".into()),
            FilterOption::Cuisine("Middle Eastern".into()),
            FilterOption::Tag("make-ahead".into()),
            FilterOption::MaxMinutes(20),
        ]);
        q.search = "dal & rice".into();
        let qs = q.to_query_string();
        assert_eq!(
            qs,
            "?q=dal%20%26%20rice&diet=veg&by=Anna%20Maria,Lorenzo\
             &cuisine=Middle%20Eastern&tag=make-ahead&time=20"
        );
        assert_eq!(RecipeQuery::from_query_string(&qs), q);
        assert_eq!(RecipeQuery::default().to_query_string(), "");

        let junk = RecipeQuery::from_query_string("?diet=fish&time=soon&x=1&q=%zz&by=&tag");
        assert_eq!(junk, RecipeQuery::default());
        assert_eq!(
            RecipeQuery::from_query_string("q=lentil+soup").search,
            "lentil soup"
        );
    }

    #[test]
    fn facets_are_sorted_unique_and_leave_out_the_diet_tag() {
        let f = Facets::of(&sample());
        assert_eq!(f.authors, ["Anna", "Claude", "Lorenzo"]);
        assert_eq!(f.cuisines, ["American", "Greek", "Indian", "Italian"]);
        assert_eq!(f.tags, ["breakfast", "dinner", "pasta", "quick", "salad"]);
    }
}
