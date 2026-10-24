use leptos::prelude::*;

use crate::app::BASE_PATH;
use crate::format::{asset_url, format_amount, format_kcal, format_macros, format_timing, round};
use crate::models::{IngredientCatalog, Recipe};
use crate::nutrition::{ingredient_macros, per_serving, recipe_macros};

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
            <table class="ingredient-table">
                <thead>
                    <tr>
                        <th>"Ingredient"</th>
                        <th>"Amount"</th>
                        <th>"Calories"</th>
                        <th>"Protein"</th>
                        <th>"Carbs"</th>
                        <th>"Fat"</th>
                    </tr>
                </thead>
                <tbody>
                    {recipe
                        .ingredients
                        .iter()
                        .map(|item| {
                            let info = catalog.get(&item.ingredient);
                            let name = info
                                .map(|i| i.name.clone())
                                .unwrap_or_else(|| format!("unknown: {}", item.ingredient));
                            let amount = match info {
                                Some(i) => format_amount(item.amount, &i.unit),
                                None => format_amount(item.amount, ""),
                            };
                            let m = ingredient_macros(item, &catalog);
                            view! {
                                <tr>
                                    <td>{name}</td>
                                    <td>{amount}</td>
                                    <td>{cell(m.as_ref().map(|m| m.calories))}</td>
                                    <td>{cell(m.as_ref().map(|m| m.protein))}</td>
                                    <td>{cell(m.as_ref().map(|m| m.carbs))}</td>
                                    <td>{cell(m.as_ref().map(|m| m.fat))}</td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
}

/// Render a contributed-macro cell as a rounded number, or "—" when unavailable
/// ("to taste" amounts, or ingredients lacking catalog macro data).
fn cell(value: Option<f64>) -> String {
    match value {
        Some(v) => round(v).to_string(),
        None => "—".to_string(),
    }
}
