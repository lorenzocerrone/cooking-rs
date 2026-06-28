use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::{AddToPlanner, RecipeOverview};
use crate::models::{IngredientCatalog, Recipe};
use crate::recipes::load_data;

/// Detail view for a single recipe (`/recipe/:id`): the shared overview plus the
/// numbered steps (which only the full page shows).
#[component]
pub fn RecipeDetailPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || {
        params
            .read()
            .get("id")
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let data = Resource::new(|| (), |_| async { load_data().await });

    view! {
        <Suspense fallback=|| view! { <p class="muted">"Loading recipe…"</p> }>
            {move || {
                data.get()
                    .map(|d| {
                        let wanted = id();
                        match d.recipes.into_iter().find(|r| r.id == wanted) {
                            Some(recipe) => {
                                view! { <RecipeDetail recipe=recipe catalog=d.catalog /> }
                                    .into_any()
                            }
                            None => {
                                view! {
                                    <h1>"Recipe not found"</h1>
                                    <p class="muted">"No recipe matches this link."</p>
                                }
                                    .into_any()
                            }
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn RecipeDetail(recipe: Recipe, catalog: IngredientCatalog) -> impl IntoView {
    let title = recipe.title.clone();
    let id = recipe.id.clone();
    let steps = recipe.steps.clone();

    view! {
        <article class="recipe-detail">
            <h1>{title.clone()}</h1>
            <AddToPlanner recipe_id=id recipe_title=title />
            <RecipeOverview recipe=recipe catalog=catalog />
            <h2>"Steps"</h2>
            <ol class="recipe-steps">
                {steps.into_iter().map(|step| view! { <li>{step}</li> }).collect_view()}
            </ol>
        </article>
    }
}
