use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, notify, use_app_data};
use crate::components::IconBack;
use crate::models::{IngredientCatalog, Recipe};
use crate::pages::RecipeBody;
use crate::scaffold::{headline, parse_draft, render_json, DRAFT_EXAMPLE};
use crate::storage::{load_draft, save_draft};

/// Write a recipe as a plain-text draft (`/new`): the scaffolder checks it on
/// every keystroke against the catalog, listing problems by line or previewing
/// the recipe as it will look. The draft stays on this device until sent.
#[component]
pub fn NewRecipePage() -> impl IntoView {
    let data = use_app_data();

    view! {
        <Suspense fallback=|| view! { <p class="muted">"Loading…"</p> }>
            {move || {
                data.get()
                    .map(|d| view! { <RecipeEditor recipes=d.recipes catalog=d.catalog /> })
            }}
        </Suspense>
    }
}

#[component]
fn RecipeEditor(recipes: Vec<Recipe>, catalog: IngredientCatalog) -> impl IntoView {
    let draft = RwSignal::new(load_draft());
    Effect::new(move |_| draft.with(|t| save_draft(t)));

    let catalog = StoredValue::new(catalog);
    let parsed = Memo::new(move |_| {
        let text = draft.get();
        (!text.trim().is_empty()).then(|| catalog.with_value(|c| parse_draft(&text, c)))
    });
    let recipe = move || parsed.with(|p| p.clone().and_then(Result::ok).map(|d| d.recipe));
    // A draft whose id is taken would replace that recipe.
    let taken = move || {
        let id = recipe()?.id;
        recipes.iter().find(|r| r.id == id).map(|r| r.title.clone())
    };

    let copy_json = move |_| {
        if let Some(r) = recipe() {
            let _ = window()
                .navigator()
                .clipboard()
                .write_text(&render_json(&r));
            notify("Recipe JSON copied");
        }
    };

    view! {
        <article class="editor">
            <div class="recipe__bar">
                <A href=href("/") attr:class="back">
                    <IconBack />
                    "Recipes"
                </A>
                <button
                    type="button"
                    class="btn"
                    disabled=move || recipe().is_none()
                    on:click=copy_json
                >
                    "Copy JSON"
                </button>
            </div>

            <header class="page-head">
                <h1>"New recipe"</h1>
            </header>

            <textarea
                class="draft mono"
                rows="16"
                placeholder=DRAFT_EXAMPLE
                aria-label="Recipe draft"
                autocapitalize="sentences"
                spellcheck="false"
                prop:value=move || draft.get()
                on:input=move |ev| draft.set(event_target_value(&ev))
            ></textarea>

            <details class="draft-help">
                <summary class="eyebrow">"How to write a draft"</summary>
                <ul>
                    <li>"First line: the title. Then "<code>"author:"</code>" and, if you like, "
                        <code>"servings: cuisine: tags: prep: cook: description:"</code>"."</li>
                    <li>"A blank line, then one ingredient per line: "
                        <code>"300 g lentils"</code>", "<code>"3 cloves garlic"</code>", "
                        <code>"2 onion"</code>" (a bare number counts pieces)."</li>
                    <li>"After a comma is a note ("<code>"1 onion, chopped"</code>"); "
                        <code>"(optional)"</code>" at the end marks it optional; a line "
                        "without an amount is “to taste”."</li>
                    <li>"Units: "<code>"g kg ml l tsp tbsp cup pcs"</code>"."</li>
                    <li>"A line "<code>"Steps"</code>", then one step per line."</li>
                </ul>
            </details>

            {move || {
                parsed
                    .get()
                    .map(|result| match result {
                        Err(problems) => {
                            let count = problems.len();
                            view! {
                                <section class="draft-check draft-check--bad" aria-live="polite">
                                    <p class="eyebrow">
                                        {if count == 1 {
                                            "1 problem".to_string()
                                        } else {
                                            format!("{count} problems")
                                        }}
                                    </p>
                                    <ul>
                                        {problems
                                            .into_iter()
                                            .map(|p| {
                                                view! {
                                                    <li>
                                                        <span class="mono muted">{format!("Line {}", p.line)}</span>
                                                        <span>{headline(&p.message).to_string()}</span>
                                                    </li>
                                                }
                                            })
                                            .collect_view()}
                                    </ul>
                                </section>
                            }
                                .into_any()
                        }
                        Ok(d) => {
                            let notes: Vec<String> = taken()
                                .map(|t| format!("Replaces the existing recipe “{t}” (same id)."))
                                .into_iter()
                                .chain(d.warnings)
                                .collect();
                            view! {
                                <section class="draft-check" aria-live="polite">
                                    <p class="eyebrow">"Looks good · preview"</p>
                                    {(!notes.is_empty())
                                        .then(|| {
                                            view! {
                                                <ul>
                                                    {notes
                                                        .into_iter()
                                                        .map(|n| view! { <li>{n}</li> })
                                                        .collect_view()}
                                                </ul>
                                            }
                                        })}
                                </section>
                                <article class="recipe recipe--preview">
                                    <RecipeBody
                                        recipe=d.recipe
                                        catalog=catalog.get_value()
                                    />
                                </article>
                            }
                                .into_any()
                        }
                    })
            }}
        </article>
    }
}
