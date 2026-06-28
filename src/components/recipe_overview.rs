use leptos::prelude::*;

use crate::format::{format_kcal, format_macros, format_timing, ingredient_line};
use crate::models::{IngredientCatalog, Recipe};
use crate::nutrition::{per_serving, recipe_macros};

/// The shared summary body for a recipe: image, description, badges, per-serving
/// macros, and the resolved ingredient list. Reused by the recipe detail page and
/// the planner's overview modal so the two never drift apart.
#[component]
pub fn RecipeOverview(recipe: Recipe, catalog: IngredientCatalog) -> impl IntoView {
    let per_serving_macros =
        recipe_macros(&recipe, &catalog).map(|m| per_serving(&m, recipe.servings));

    view! {
        <div class="recipe-overview">
            {recipe
                .image
                .clone()
                .map(|src| view! { <img class="recipe-image" src=src alt=recipe.title.clone() /> })}

            <p class="muted">
                {format!("Serves {}", recipe.servings)}
                {format_timing(&recipe)}
            </p>

            {recipe.description.clone().map(|d| view! { <p class="recipe-desc">{d}</p> })}

            <div class="badges">
                {recipe
                    .cuisine
                    .clone()
                    .map(|c| view! { <span class="badge badge-cuisine">{c}</span> })}
                {recipe
                    .tags
                    .clone()
                    .into_iter()
                    .map(|t| view! { <span class="badge">{t}</span> })
                    .collect_view()}
            </div>

            {per_serving_macros.map(|m| {
                view! {
                    <p class="overview-macros">
                        <strong>{format_kcal(m.calories)}</strong>
                        " / serving · "
                        {format_macros(&m)}
                    </p>
                }
            })}

            <h4>"Ingredients"</h4>
            <ul>
                {recipe
                    .ingredients
                    .iter()
                    .map(|item| view! { <li>{ingredient_line(item, &catalog)}</li> })
                    .collect_view()}
            </ul>
        </div>
    }
}
