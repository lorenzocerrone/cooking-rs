use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::AddToPlanner;
use crate::filter::{available_cuisines, available_tags, filter, RecipeQuery};
use crate::format::{ingredient_line, total_time};
use crate::models::{IngredientCatalog, Recipe};
use crate::recipes::load_data;

/// Page listing every available recipe, with search and tag/cuisine filtering.
#[component]
pub fn RecipesPage() -> impl IntoView {
    // Async-first: embedded data resolves immediately, but we go through a
    // Resource + Suspense so a future fetch-based loader needs no UI changes.
    let data = Resource::new(|| (), |_| async { load_data().await });

    view! {
        <h1>"Recipes"</h1>
        <Suspense fallback=|| view! { <p class="muted">"Loading recipes…"</p> }>
            {move || {
                data.get()
                    .map(|d| view! { <RecipeBrowser recipes=d.recipes catalog=d.catalog /> })
            }}
        </Suspense>
    }
}

#[component]
fn RecipeBrowser(recipes: Vec<Recipe>, catalog: IngredientCatalog) -> impl IntoView {
    if recipes.is_empty() {
        return view! { <p class="muted">"No recipes found."</p> }.into_any();
    }

    let cuisines = available_cuisines(&recipes);
    let tags = available_tags(&recipes);

    let recipes = StoredValue::new(recipes);
    let catalog = StoredValue::new(catalog);

    let search = RwSignal::new(String::new());
    let cuisine = RwSignal::new(String::new());
    let tag = RwSignal::new(String::new());

    // Pure filtering lives in `crate::filter`; the page just builds the query.
    let filtered = move || {
        let query = RecipeQuery {
            search: search.get(),
            cuisine: cuisine.get(),
            tag: tag.get(),
        };
        recipes.with_value(|rs| filter(rs, &query).into_iter().cloned().collect::<Vec<_>>())
    };

    view! {
        <div class="recipe-filters">
            <input
                class="search-input"
                type="search"
                placeholder="Search recipes…"
                prop:value=move || search.get()
                on:input=move |ev| search.set(event_target_value(&ev))
            />
            <FilterSelect label="All cuisines" options=cuisines value=cuisine />
            <FilterSelect label="All tags" options=tags value=tag />
        </div>
        <div class="recipe-grid">
            {move || {
                let list = filtered();
                if list.is_empty() {
                    view! { <p class="muted">"No recipes match your filters."</p> }.into_any()
                } else {
                    list.into_iter()
                        .map(|r| view! { <RecipeCard recipe=r catalog=catalog /> })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>
    }
    .into_any()
}

#[component]
fn FilterSelect(
    label: &'static str,
    options: Vec<String>,
    value: RwSignal<String>,
) -> impl IntoView {
    view! {
        <select
            prop:value=move || value.get()
            on:change=move |ev| value.set(event_target_value(&ev))
        >
            <option value="">{label}</option>
            {options
                .into_iter()
                .map(|opt| {
                    let v = opt.clone();
                    view! { <option value=v>{opt}</option> }
                })
                .collect_view()}
        </select>
    }
}

#[component]
fn RecipeCard(recipe: Recipe, catalog: StoredValue<IngredientCatalog>) -> impl IntoView {
    let href = format!("/recipe/{}", recipe.id);
    let total = total_time(&recipe);
    let id = recipe.id.clone();
    let title = recipe.title.clone();

    view! {
        <article class="recipe-card">
            <h3><A href=href>{recipe.title}</A></h3>
            <p class="muted">
                {format!("Serves {}", recipe.servings)}
                {total.map(|m| format!(" · {m} min"))}
            </p>
            {recipe.description.map(|d| view! { <p class="recipe-desc">{d}</p> })}
            <div class="badges">
                {recipe.cuisine.map(|c| view! { <span class="badge badge-cuisine">{c}</span> })}
                {recipe
                    .tags
                    .into_iter()
                    .map(|t| view! { <span class="badge">{t}</span> })
                    .collect_view()}
            </div>
            <ul>
                {recipe
                    .ingredients
                    .into_iter()
                    .map(|item| {
                        let line = catalog.with_value(|c| ingredient_line(&item, c));
                        view! { <li>{line}</li> }
                    })
                    .collect_view()}
            </ul>
            <AddToPlanner recipe_id=id recipe_title=title />
        </article>
    }
}
