//! Pure recipe search/filtering. No Leptos types — the recipes page builds a
//! [`RecipeQuery`] from its signals and calls [`filter`]; the result drives the view.

use crate::models::Recipe;

/// The active search + filter selections. Empty fields mean "no constraint".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecipeQuery {
    /// Free-text search over title + description (case-insensitive).
    pub search: String,
    /// Exact cuisine to require, or empty for any.
    pub cuisine: String,
    /// Exact tag to require, or empty for any.
    pub tag: String,
}

impl RecipeQuery {
    fn matches(&self, recipe: &Recipe) -> bool {
        let q = self.search.to_lowercase();
        let text_ok = q.is_empty()
            || recipe.title.to_lowercase().contains(&q)
            || recipe
                .description
                .as_deref()
                .is_some_and(|d| d.to_lowercase().contains(&q));
        let cuisine_ok =
            self.cuisine.is_empty() || recipe.cuisine.as_deref() == Some(self.cuisine.as_str());
        let tag_ok = self.tag.is_empty() || recipe.tags.iter().any(|t| t == &self.tag);
        text_ok && cuisine_ok && tag_ok
    }
}

/// The recipes matching `query`, preserving input order.
pub fn filter<'a>(recipes: &'a [Recipe], query: &RecipeQuery) -> Vec<&'a Recipe> {
    recipes.iter().filter(|r| query.matches(r)).collect()
}

/// Sorted, de-duplicated cuisines present in the recipe set.
pub fn available_cuisines(recipes: &[Recipe]) -> Vec<String> {
    sorted_unique(recipes.iter().filter_map(|r| r.cuisine.clone()))
}

/// Sorted, de-duplicated tags present in the recipe set.
pub fn available_tags(recipes: &[Recipe]) -> Vec<String> {
    sorted_unique(recipes.iter().flat_map(|r| r.tags.iter().cloned()))
}

fn sorted_unique(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = items.collect();
    v.sort();
    v.dedup();
    v
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
            macros: None,
        }
    }

    fn sample() -> Vec<Recipe> {
        vec![
            recipe(
                "carbonara",
                "Spaghetti Carbonara",
                Some("Italian"),
                &["pasta", "quick"],
                Some("Roman classic"),
            ),
            recipe(
                "salad",
                "Greek Salad",
                Some("Greek"),
                &["salad", "quick"],
                Some("Crunchy and fresh"),
            ),
            recipe(
                "oats",
                "Overnight Oats",
                Some("American"),
                &["breakfast"],
                None,
            ),
        ]
    }

    #[test]
    fn empty_query_returns_all() {
        let r = sample();
        assert_eq!(filter(&r, &RecipeQuery::default()).len(), 3);
    }

    #[test]
    fn search_matches_title_and_description() {
        let r = sample();
        let by_title = RecipeQuery {
            search: "greek".into(),
            ..Default::default()
        };
        assert_eq!(filter(&r, &by_title).len(), 1);
        let by_desc = RecipeQuery {
            search: "roman".into(),
            ..Default::default()
        };
        assert_eq!(filter(&r, &by_desc)[0].id, "carbonara");
    }

    #[test]
    fn cuisine_and_tag_filters_combine() {
        let r = sample();
        let q = RecipeQuery {
            cuisine: "Italian".into(),
            tag: "quick".into(),
            ..Default::default()
        };
        let out = filter(&r, &q);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "carbonara");

        let q2 = RecipeQuery {
            tag: "quick".into(),
            ..Default::default()
        };
        assert_eq!(filter(&r, &q2).len(), 2);
    }

    #[test]
    fn available_lists_are_sorted_unique() {
        let r = sample();
        assert_eq!(available_cuisines(&r), vec!["American", "Greek", "Italian"]);
        assert_eq!(
            available_tags(&r),
            vec!["breakfast", "pasta", "quick", "salad"]
        );
    }
}
