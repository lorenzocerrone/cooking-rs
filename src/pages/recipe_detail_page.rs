use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::app::{href, use_app_data};
use crate::components::{AddToWeek, IconBack, Stepper};
use crate::format::{format_ingredient_amount_scaled, format_timing};
use crate::models::{IngredientCatalog, Recipe};

/// A single recipe (`/recipe/:id`), laid out for cooking from the phone: a
/// servings stepper that rescales the ingredients, and steps you can tap off.
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
    let data = use_app_data();

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
                                    <A href=href("/") attr:class="back">
                                        <IconBack />
                                        "Recipes"
                                    </A>
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
    let base_servings = recipe.servings.max(1);
    let servings = RwSignal::new(base_servings);
    let factor = move || servings.get() as f64 / base_servings as f64;

    // Per-step "done" ticks: a cooking aid, deliberately not persisted.
    let done = RwSignal::new(vec![false; recipe.steps.len()]);

    let eyebrow = {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = &recipe.cuisine {
            parts.push(c.clone());
        }
        if !recipe.tags.is_empty() {
            parts.push(recipe.tags.join(", "));
        }
        parts.join(" · ")
    };
    let timing = format_timing(&recipe);

    let ingredients = StoredValue::new(recipe.ingredients.clone());
    let catalog = StoredValue::new(catalog);

    view! {
        <article class="recipe">
            <div class="recipe__bar">
                <A href=href("/") attr:class="back">
                    <IconBack />
                    "Recipes"
                </A>
                <AddToWeek recipe_id=recipe.id.clone() recipe_title=recipe.title.clone() />
            </div>

            <header class="recipe__head">
                {(!eyebrow.is_empty()).then(|| view! { <p class="eyebrow">{eyebrow}</p> })}
                <h1>{recipe.title.clone()}</h1>
                {recipe.description.clone().map(|d| view! { <p class="recipe__desc">{d}</p> })}
            </header>

            <div class="recipe__scale">
                <Stepper
                    value=servings
                    label="servings"
                    on_change=Callback::new(move |v| servings.set(v))
                />
                {(!timing.is_empty()).then(|| view! { <span class="mono muted">{timing}</span> })}
            </div>

            <ul class="ingredients">
                {move || {
                    let f = factor();
                    ingredients.with_value(|items| {
                        items
                            .iter()
                            .map(|item| {
                                let (name, amount) = catalog.with_value(|c| {
                                    (
                                        c.get(&item.ingredient)
                                            .map(|i| i.name.clone())
                                            .unwrap_or_else(|| item.ingredient.clone()),
                                        format_ingredient_amount_scaled(item, c, f),
                                    )
                                });
                                let note = item.note.clone();
                                let optional = item.optional;
                                view! {
                                    <li>
                                        <span>
                                            {name}
                                            {note.map(|n| view! { <small class="muted">{format!(", {n}")}</small> })}
                                            {optional.then(|| view! { <small class="muted">" (optional)"</small> })}
                                        </span>
                                        <span class="mono">{amount}</span>
                                    </li>
                                }
                            })
                            .collect_view()
                    })
                }}
            </ul>

            <p class="eyebrow">"Method"</p>
            <ol class="steps">
                {recipe
                    .steps
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, step)| {
                        let is_done = move || done.with(|d| d.get(i).copied().unwrap_or(false));
                        let toggle = move |_| {
                            done.update(|d| {
                                if let Some(v) = d.get_mut(i) {
                                    *v = !*v;
                                }
                            })
                        };
                        view! {
                            <li class:done=is_done>
                                <button
                                    type="button"
                                    class="step"
                                    aria-pressed=move || is_done().to_string()
                                    on:click=toggle
                                >
                                    <span class="step__n mono">
                                        {move || if is_done() { "✓".to_string() } else { (i + 1).to_string() }}
                                    </span>
                                    <span>{step}</span>
                                </button>
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
        </article>
    }
}
