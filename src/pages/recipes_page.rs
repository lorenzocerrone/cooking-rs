use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, use_app_data};
use crate::components::RecipeFilters;
use crate::filter::{filter, RecipeQuery};
use crate::format::format_total_time;
use crate::models::Recipe;
use crate::state::{read_search, write_search};

/// Every recipe as a compact list with search and the filter sheet. A row opens
/// the full recipe page. The query lives in the address bar's query string, so
/// it survives opening a recipe and coming back, and a filtered list can be sent.
#[component]
pub fn RecipesPage() -> impl IntoView {
    let data = use_app_data();

    view! {
        <Suspense fallback=|| view! { <p class="muted">"Loading recipes…"</p> }>
            {move || data.get().map(|d| view! { <RecipeList recipes=d.recipes /> })}
        </Suspense>
    }
}

#[component]
fn RecipeList(recipes: Vec<Recipe>) -> impl IntoView {
    let recipes = StoredValue::new(recipes);
    let query = RwSignal::new(RecipeQuery::from_query_string(&read_search()));
    Effect::new(move |_| query.with(|q| write_search(&q.to_query_string())));

    // Pure filtering lives in `crate::filter`; the page just holds the query.
    let filtered = Memo::new(move |_| {
        let q = query.get();
        recipes.with_value(|rs| filter(rs, &q).into_iter().cloned().collect::<Vec<_>>())
    });

    view! {
        <header class="page-head">
            <h1>"Recipes"</h1>
            <span class="mono muted">{move || filtered.with(|l| l.len())}</span>
        </header>
        <RecipeFilters query=query recipes=recipes />
        <ul class="recipe-list">
            {move || {
                let list = filtered.get();
                if list.is_empty() {
                    view! { <li class="muted empty">"No recipes match."</li> }.into_any()
                } else {
                    list.into_iter().map(|r| view! { <RecipeRow recipe=r /> }).collect_view().into_any()
                }
            }}
        </ul>
    }
}

#[component]
fn RecipeRow(recipe: Recipe) -> impl IntoView {
    let time = format_total_time(&recipe);
    let mut meta: Vec<String> = Vec::new();
    if let Some(c) = &recipe.cuisine {
        meta.push(c.clone());
    }
    meta.push(match recipe.servings {
        1 => "1 serving".to_string(),
        n => format!("{n} servings"),
    });
    let tags = recipe.tags.join(", ");

    view! {
        <li>
            <A href=href(&format!("/recipe/{}", recipe.id)) attr:class="recipe-row">
                <span class="recipe-row__name">{recipe.title}</span>
                <span class="recipe-row__time mono">{time}</span>
                <span class="recipe-row__meta mono">
                    {meta.join(" · ")}
                    {(!tags.is_empty()).then(|| view! { <i>{format!(" · {tags}")}</i> })}
                </span>
            </A>
        </li>
    }
}
