use leptos::prelude::*;

use crate::app::BASE_PATH;
use crate::format::{asset_url, format_ingredient_amount, format_timing};
use crate::models::{IngredientCatalog, Recipe};

/// The shared summary body for a recipe: image, description, badges, and the
/// resolved ingredient list. Reused by the recipe detail page and the planner's
/// overview modal so the two never drift apart.
#[component]
pub fn RecipeOverview(recipe: Recipe, catalog: IngredientCatalog) -> impl IntoView {
    view! {
        <div class="recipe-overview">
            {recipe
                .image
                .clone()
                .map(|src| {
                    view! {
                        <img
                            class="recipe-image"
                            src=asset_url(BASE_PATH, &src)
                            alt=recipe.title.clone()
                        />
                    }
                })}

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

            <h4>"Ingredients"</h4>
            <table class="ingredient-table">
                <thead>
                    <tr>
                        <th>"Ingredient"</th>
                        <th>"Amount"</th>
                    </tr>
                </thead>
                <tbody>
                    {recipe
                        .ingredients
                        .iter()
                        .map(|item| {
                            let name = catalog
                                .get(&item.ingredient)
                                .map(|i| i.name.clone())
                                .unwrap_or_else(|| format!("unknown: {}", item.ingredient));
                            let note = item.note.clone();
                            let amount = format_ingredient_amount(item, &catalog);
                            let optional = item.optional;
                            view! {
                                <tr>
                                    <td>
                                        {name}
                                        {note.map(|n| view! { <span class="muted">{format!(", {n}")}</span> })}
                                    </td>
                                    <td>
                                        {amount}
                                        {optional.then(|| view! { <span class="muted">" (optional)"</span> })}
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
}
