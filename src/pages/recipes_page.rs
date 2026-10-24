use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, BASE_PATH};
use crate::components::{AddToPlanner, Modal, RecipeOverview};
use crate::filter::{available_cuisines, available_tags, filter, RecipeQuery};
use crate::format::{asset_url, ingredient_summary, total_time};
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

    // Quick-view dialog state: which recipe id is shown and whether it's open. A
    // single Modal is rendered for the whole grid (same pattern as the planner).
    let modal_open = RwSignal::new(false);
    let selected = RwSignal::new(String::new());

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
                        .map(|r| {
                            view! {
                                <RecipeCard
                                    recipe=r
                                    catalog=catalog
                                    modal_open=modal_open
                                    selected=selected
                                />
                            }
                        })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>

        <Modal open=modal_open>
            {move || {
                let id = selected.get();
                recipes
                    .with_value(|rs| rs.iter().find(|r| r.id == id).cloned())
                    .map(|recipe| {
                        let cat = catalog.with_value(|c| c.clone());
                        let href = href(&format!("/recipe/{}", recipe.id));
                        view! {
                            <RecipeOverview recipe=recipe catalog=cat />
                            <A
                                href=href
                                attr:class="btn-link"
                                on:click=move |_| modal_open.set(false)
                            >
                                "View full recipe →"
                            </A>
                        }
                    })
            }}
        </Modal>
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
fn RecipeCard(
    recipe: Recipe,
    catalog: StoredValue<IngredientCatalog>,
    modal_open: RwSignal<bool>,
    selected: RwSignal<String>,
) -> impl IntoView {
    let total = total_time(&recipe);
    let id = recipe.id.clone();
    let title = recipe.title.clone();
    let alt = recipe.title.clone();
    let thumb = recipe.image.clone();

    // Open the quick-view dialog for this recipe (from the image or the title).
    let open_id = recipe.id.clone();
    let open = move || {
        selected.set(open_id.clone());
        modal_open.set(true);
    };

    view! {
        <article class="recipe-card">
            {
                let open = open.clone();
                match thumb {
                    Some(src) => view! {
                        <img
                            class="recipe-thumb"
                            src=asset_url(BASE_PATH, &src)
                            alt=alt
                            on:click=move |_| open()
                        />
                    }
                    .into_any(),
                    None => view! {
                        <div
                            class="recipe-thumb recipe-thumb--empty"
                            on:click=move |_| open()
                        ></div>
                    }
                    .into_any(),
                }
            }
            <div class="recipe-card__body">
                <h3>
                    <button
                        class="recipe-card__title-btn"
                        type="button"
                        on:click=move |_| open()
                    >
                        {recipe.title}
                    </button>
                </h3>
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
                <p class="recipe-ingredients">
                    {catalog.with_value(|c| ingredient_summary(&recipe.ingredients, c, 3))}
                </p>
                <AddToPlanner recipe_id=id recipe_title=title />
            </div>
        </article>
    }
}
