use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, notify, use_app_data};
use crate::components::{IconBack, IconClose, IconPlus, Modal, Stepper};
use crate::editor::{unit_suggestions, Field, IngredientRow, Issue, RecipeForm};
use crate::extras::suggest;
use crate::filter::{Facets, VEGETARIAN_TAG};
use crate::models::{IngredientCatalog, Recipe};
use crate::pages::RecipeBody;
use crate::paste::form_from_text;
use crate::scaffold::render_json;
use crate::storage::{load_form, save_form};

/// Suggestions shown under an ingredient being typed.
const SUGGESTIONS: usize = 6;

/// Write a recipe (`/new`) with a form: name and details, ingredient rows picked
/// from the catalog (or added as new ingredients), and steps. What is still
/// missing is listed in plain words; a complete recipe is previewed as it will
/// look. The form stays on this device until it is sent.
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

/// Everything a part of the editor needs: the form and the dictionary.
#[derive(Clone, Copy)]
struct Editor {
    form: RwSignal<RecipeForm>,
    /// The ingredient dictionary: suggestions and unit hints.
    catalog: StoredValue<IngredientCatalog>,
    /// The checked recipe, or what is still missing.
    result: Memo<Result<Recipe, Vec<Issue>>>,
    /// The ingredient row being typed in (its suggestions are shown).
    focused: RwSignal<Option<usize>>,
}

impl Editor {
    fn row(self, i: usize) -> IngredientRow {
        self.form
            .with(|f| f.ingredients.get(i).cloned())
            .unwrap_or_default()
    }

    fn update_row(self, i: usize, change: impl FnOnce(&mut IngredientRow)) {
        self.form.update(|f| {
            if let Some(row) = f.ingredients.get_mut(i) {
                change(row);
            }
        });
    }
}

#[component]
fn RecipeEditor(recipes: Vec<Recipe>, catalog: IngredientCatalog) -> impl IntoView {
    let form = RwSignal::new(load_form());
    Effect::new(move |_| form.with(save_form));

    let catalog = StoredValue::new(catalog);
    let result = Memo::new(move |_| form.with(|f| catalog.with_value(|c| f.to_recipe(c))));
    let ed = Editor {
        form,
        catalog,
        result,
        focused: RwSignal::new(None),
    };
    let facets = StoredValue::new(Facets::of(&recipes));
    let recipes = StoredValue::new(recipes);
    let taken = move || {
        let id = result.with(|r| r.as_ref().ok().map(|r| r.id.clone()))?;
        recipes.with_value(|rs| rs.iter().find(|r| r.id == id).map(|r| r.title.clone()))
    };

    let start_over = RwSignal::new(false);
    let paste_open = RwSignal::new(false);
    let copy_json = move |_| {
        let Ok(recipe) = result.get_untracked() else {
            return;
        };
        let text = render_json(&recipe);
        let _ = window().navigator().clipboard().write_text(&text);
        notify("Recipe copied");
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
                    class="btn btn--quiet"
                    prop:disabled=move || form.with(RecipeForm::is_blank)
                    on:click=move |_| start_over.set(true)
                >
                    "Start over"
                </button>
            </div>

            <header class="page-head">
                <h1>"New recipe"</h1>
                <button type="button" class="btn" on:click=move |_| paste_open.set(true)>
                    "Paste a recipe"
                </button>
            </header>

            <About ed=ed facets=facets />
            <Ingredients ed=ed />
            <Steps ed=ed />

            <section class="form-card" aria-live="polite">
                {move || match result.get() {
                    Err(issues) => view! { <ToFinish issues=issues /> }.into_any(),
                    Ok(_) => {
                        view! {
                            <p class="eyebrow">"Ready"</p>
                            <p>"Everything is filled in. Check the preview below."</p>
                            {taken()
                                .map(|t| {
                                    view! {
                                        <p class="form-warn">
                                            {format!(
                                                "There is already a recipe called “{t}”. This one would replace it.",
                                            )}
                                        </p>
                                    }
                                })}
                            <div class="form-actions">
                                <button type="button" class="btn" on:click=copy_json>
                                    "Copy JSON"
                                </button>
                            </div>
                        }
                            .into_any()
                    }
                }}
            </section>

            {move || {
                result
                    .get()
                    .ok()
                    .map(|recipe| {
                        view! {
                            <p class="eyebrow">"Preview"</p>
                            <article class="recipe recipe--preview">
                                <RecipeBody recipe=recipe catalog=catalog.get_value() />
                            </article>
                        }
                    })
            }}
        </article>

        <PasteSheet ed=ed open=paste_open />

        <Modal open=start_over title="Start over?">
            <p class="muted">"This clears the whole form."</p>
            <div class="form-actions">
                <button type="button" class="btn" on:click=move |_| start_over.set(false)>
                    "Keep it"
                </button>
                <button
                    type="button"
                    class="btn btn--danger"
                    on:click=move |_| {
                        form.set(RecipeForm::default());
                        start_over.set(false);
                    }
                >
                    "Clear the form"
                </button>
            </div>
        </Modal>
    }
}

/// A one-line text field bound to a form field.
fn text_field(
    form: RwSignal<RecipeForm>,
    label: &'static str,
    placeholder: &'static str,
    list: Option<&'static str>,
    numeric: bool,
    get: fn(&RecipeForm) -> &String,
    set: fn(&mut RecipeForm, String),
) -> impl IntoView {
    view! {
        <label class="field">
            <span class="field__label">{label}</span>
            <input
                type="text"
                class="field__input"
                inputmode=numeric.then_some("numeric")
                placeholder=placeholder
                list=list
                autocomplete="off"
                prop:value=move || form.with(|f| get(f).clone())
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    form.update(|f| set(f, v));
                }
            />
        </label>
    }
}

/// Name, author, description, servings, times, cuisine and tags.
#[component]
fn About(ed: Editor, facets: StoredValue<Facets>) -> impl IntoView {
    let form = ed.form;

    // Tags: the ones other recipes use, plus any typed here.
    let new_tag = RwSignal::new(String::new());
    let add_tag = move || {
        let tag = new_tag.get_untracked().trim().to_lowercase();
        if !tag.is_empty() {
            form.update(|f| {
                if !f.tags.contains(&tag) {
                    f.tags.push(tag);
                }
            });
        }
        new_tag.set(String::new());
    };
    let tag_options = move || {
        let mut all = vec![VEGETARIAN_TAG.to_string()];
        all.extend(facets.with_value(|f| f.tags.clone()));
        form.with(|f| {
            for t in &f.tags {
                if !all.contains(t) {
                    all.push(t.clone());
                }
            }
        });
        all
    };

    view! {
        <section class="form-card">
            {text_field(form, "Name", "e.g. Lentil dal", None, false, |f| &f.title, |f, v| f.title = v)}
            {text_field(
                form,
                "By",
                "Who is the recipe from?",
                Some("authors"),
                false,
                |f| &f.author,
                |f, v| f.author = v,
            )}
            <label class="field">
                <span class="field__label">"Short description " <small>"(optional)"</small></span>
                <textarea
                    class="field__input"
                    rows="2"
                    placeholder="One line about the dish"
                    prop:value=move || form.with(|f| f.description.clone())
                    on:input=move |ev| {
                        let v = event_target_value(&ev);
                        form.update(|f| f.description = v);
                    }
                ></textarea>
            </label>
            <div class="field">
                <span class="field__label">"Serves"</span>
                <Stepper
                    value=Signal::derive(move || form.with(|f| f.servings))
                    label="servings"
                    on_change=Callback::new(move |v| form.update(|f| f.servings = v))
                />
            </div>
            <div class="field-row">
                {text_field(form, "Prep (min)", "15", None, true, |f| &f.prep, |f, v| f.prep = v)}
                {text_field(form, "Cook (min)", "30", None, true, |f| &f.cook, |f, v| f.cook = v)}
            </div>
            {text_field(
                form,
                "Cuisine (optional)",
                "e.g. Italian",
                Some("cuisines"),
                false,
                |f| &f.cuisine,
                |f, v| f.cuisine = v,
            )}
            <div class="field">
                <span class="field__label">"Tags " <small>"(optional)"</small></span>
                <p class="chips chips--wrap">
                    {move || {
                        tag_options()
                            .into_iter()
                            .map(|tag| {
                                let t = tag.clone();
                                let on = move || form.with(|f| f.tags.contains(&t));
                                let t2 = tag.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="chip"
                                        class:chip--on=on.clone()
                                        aria-pressed=move || on().to_string()
                                        on:click=move |_| {
                                            form.update(|f| {
                                                if let Some(at) = f.tags.iter().position(|x| *x == t2) {
                                                    f.tags.remove(at);
                                                } else {
                                                    f.tags.push(t2.clone());
                                                }
                                            })
                                        }
                                    >
                                        {tag}
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </p>
                <input
                    type="text"
                    class="field__input"
                    placeholder="Another tag, then Enter"
                    aria-label="New tag"
                    autocomplete="off"
                    prop:value=move || new_tag.get()
                    on:input=move |ev| new_tag.set(event_target_value(&ev))
                    on:change=move |_| add_tag()
                />
                <p class="field__hint">
                    "“vegetarian” is set for you when every ingredient is in the list; tap it yourself otherwise."
                </p>
            </div>
            <datalist id="authors">
                {facets
                    .with_value(|f| {
                        f.authors.iter().map(|a| view! { <option value=a.clone() /> }).collect_view()
                    })}
            </datalist>
            <datalist id="cuisines">
                {facets
                    .with_value(|f| {
                        f.cuisines.iter().map(|c| view! { <option value=c.clone() /> }).collect_view()
                    })}
            </datalist>
        </section>
    }
}

/// The ingredient rows and "Add ingredient".
#[component]
fn Ingredients(ed: Editor) -> impl IntoView {
    let count = Memo::new(move |_| ed.form.with(|f| f.ingredients.len()));
    view! {
        <section class="form-card">
            <p class="eyebrow">"Ingredients"</p>
            <ul class="ing-rows">
                <For each=move || 0..count.get() key=|i| *i let:i>
                    <IngredientRowView ed=ed i=i />
                </For>
            </ul>
            <button
                type="button"
                class="btn"
                on:click=move |_| ed.form.update(|f| f.ingredients.push(IngredientRow::default()))
            >
                <IconPlus />
                "Add ingredient"
            </button>
        </section>
    }
}

/// One ingredient: a name (any; the dictionary suggests), amount and unit,
/// note and optional.
#[component]
fn IngredientRowView(ed: Editor, i: usize) -> impl IntoView {
    let name = Memo::new(move |_| ed.row(i).name);
    let known = Memo::new(move |_| {
        let n = name.get();
        ed.catalog.with_value(|c| {
            c.lookup(&n)
                .map(|(_, x)| (x.name.clone(), x.category.clone()))
        })
    });
    let suggestions = Memo::new(move |_| {
        let n = name.get();
        let exact = known.with(|k| k.as_ref().map(|(name, _)| name.clone()));
        ed.catalog.with_value(|c| {
            suggest(c, &n, SUGGESTIONS)
                .into_iter()
                .filter(|s| Some(&s.name) != exact.as_ref())
                .collect::<Vec<_>>()
        })
    });
    let show_suggestions =
        move || ed.focused.get() == Some(i) && suggestions.with(|s| !s.is_empty());
    let row = move || ed.row(i);
    let issue = move || {
        ed.result.with(|r| {
            r.as_ref()
                .err()
                .and_then(|issues| issues.iter().find(|x| x.field == Field::Ingredient(i)))
                .map(|x| x.message.clone())
        })
    };
    let hint = move || ed.form.with(|f| ed.catalog.with_value(|c| f.hint(i, c)));
    let units_id = format!("units-{i}");
    let remove = move |_| {
        ed.form.update(|f| {
            if i < f.ingredients.len() {
                f.ingredients.remove(i);
            }
        })
    };

    view! {
        <li class="ing-row">
            <div class="ing-row__body">
                <div class="ing-row__name">
                    <input
                        type="text"
                        class="field__input"
                        placeholder="Ingredient, e.g. lentils"
                        aria-label="Ingredient"
                        autocomplete="off"
                        prop:value=move || name.get()
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            ed.update_row(i, |r| r.name = v);
                        }
                        on:focus=move |_| ed.focused.set(Some(i))
                        on:blur=move |_| {
                            if ed.focused.get_untracked() == Some(i) {
                                ed.focused.set(None);
                            }
                        }
                    />
                    {move || {
                        known
                            .get()
                            .map(|(_, aisle)| view! { <small class="eyebrow">{aisle}</small> })
                    }}
                </div>
                <Show when=show_suggestions>
                    <div class="suggest">
                        {move || {
                            suggestions
                                .get()
                                .into_iter()
                                .enumerate()
                                .map(|(n, s)| {
                                    let chosen = s.name.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="suggest__row"
                                            class:suggest__row--first=n == 0
                                            // Keep the focus in the field until the click lands.
                                            on:mousedown=|e| e.prevent_default()
                                            on:click=move |_| {
                                                let chosen = chosen.clone();
                                                ed.update_row(i, |r| r.name = chosen);
                                                ed.focused.set(None);
                                            }
                                        >
                                            <span>{s.name}</span>
                                            <span class="eyebrow">{s.category}</span>
                                        </button>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </Show>
                <div class="ing-row__qty">
                    <input
                        type="text"
                        inputmode="decimal"
                        class="field__input"
                        placeholder="to taste"
                        aria-label="Amount"
                        prop:value=move || row().amount
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            ed.update_row(i, |r| r.amount = v);
                        }
                    />
                    <input
                        type="text"
                        class="field__input"
                        placeholder="unit"
                        aria-label="Unit"
                        autocomplete="off"
                        list=units_id.clone()
                        prop:disabled=move || row().amount.trim().is_empty()
                        prop:value=move || row().unit
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            ed.update_row(i, |r| r.unit = v);
                        }
                    />
                    <datalist id=units_id>
                        {move || {
                            let n = name.get();
                            ed.catalog
                                .with_value(|c| unit_suggestions(&n, c))
                                .into_iter()
                                .map(|u| view! { <option value=u /> })
                                .collect_view()
                        }}
                    </datalist>
                </div>
                <input
                    type="text"
                    class="field__input field__input--small"
                    placeholder="Note, e.g. finely chopped"
                    aria-label="Note"
                    prop:value=move || row().note
                    on:input=move |ev| {
                        let v = event_target_value(&ev);
                        ed.update_row(i, |r| r.note = v);
                    }
                />
                <label class="toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || row().optional
                        on:change=move |ev| {
                            let on = event_target_checked(&ev);
                            ed.update_row(i, |r| r.optional = on);
                        }
                    />
                    "Optional"
                </label>
                {move || issue().map(|m| view! { <p class="form-warn">{m}</p> })}
                {move || hint().map(|m| view! { <p class="field__hint">{m}</p> })}
            </div>
            <button type="button" class="icon-btn" aria-label="Remove ingredient" on:click=remove>
                <IconClose />
            </button>
        </li>
    }
}

/// The numbered steps and "Add step".
#[component]
fn Steps(ed: Editor) -> impl IntoView {
    let form = ed.form;
    let count = Memo::new(move |_| form.with(|f| f.steps.len()));
    view! {
        <section class="form-card">
            <p class="eyebrow">"Steps"</p>
            <ol class="step-rows">
                <For each=move || 0..count.get() key=|i| *i let:i>
                    <li class="step-row">
                        <span class="step__n mono">{i + 1}</span>
                        <textarea
                            class="field__input"
                            rows="2"
                            placeholder=if i == 0 { "e.g. Soften the onion in olive oil." } else { "" }
                            aria-label=format!("Step {}", i + 1)
                            prop:value=move || {
                                form.with(|f| f.steps.get(i).cloned().unwrap_or_default())
                            }
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                form.update(|f| {
                                    if let Some(s) = f.steps.get_mut(i) {
                                        *s = v;
                                    }
                                });
                            }
                        ></textarea>
                        <button
                            type="button"
                            class="icon-btn"
                            aria-label="Remove step"
                            on:click=move |_| {
                                form.update(|f| {
                                    if i < f.steps.len() {
                                        f.steps.remove(i);
                                    }
                                })
                            }
                        >
                            <IconClose />
                        </button>
                    </li>
                </For>
            </ol>
            <button
                type="button"
                class="btn"
                on:click=move |_| form.update(|f| f.steps.push(String::new()))
            >
                <IconPlus />
                "Add step"
            </button>
        </section>
    }
}

/// What is still missing, as a checklist.
#[component]
fn ToFinish(issues: Vec<Issue>) -> impl IntoView {
    view! {
        <p class="eyebrow">"To finish"</p>
        <ul class="to-finish">
            {issues.into_iter().map(|x| view! { <li>{x.message}</li> }).collect_view()}
        </ul>
    }
}

/// "Paste a recipe": text copied from anywhere fills the form, to be checked.
#[component]
fn PasteSheet(ed: Editor, open: RwSignal<bool>) -> impl IntoView {
    let text = RwSignal::new(String::new());
    let fill = move |_| {
        let (mut pasted, summary) = ed
            .catalog
            .with_value(|c| form_from_text(&text.get_untracked(), c));
        ed.form.update(|f| {
            // Keep what the paste doesn't say (often the author).
            if pasted.author.is_empty() {
                pasted.author = f.author.clone();
            }
            *f = pasted;
        });
        open.set(false);
        text.set(String::new());
        notify(format!(
            "Filled in {} ingredient{} and {} step{}",
            summary.ingredients,
            if summary.ingredients == 1 { "" } else { "s" },
            summary.steps,
            if summary.steps == 1 { "" } else { "s" },
        ));
    };
    view! {
        <Modal open=open title="Paste a recipe">
            <div class="sheet-form">
                <p class="field__hint">
                    "Copy a recipe from a website, a message or a note and paste it here. "
                    "It fills in the form; check every row afterwards."
                </p>
                <textarea
                    class="field__input"
                    rows="10"
                    aria-label="Recipe text"
                    placeholder="Name\nServes 4\n\nIngredients\n300 g lentils\n1 onion, chopped\n\nMethod\nSoften the onion…"
                    prop:value=move || text.get()
                    on:input=move |ev| text.set(event_target_value(&ev))
                ></textarea>
                <Show when=move || !ed.form.with(RecipeForm::is_blank)>
                    <p class="form-warn">"This replaces what is in the form now."</p>
                </Show>
                <div class="form-actions">
                    <button
                        type="button"
                        class="btn btn--primary"
                        prop:disabled=move || text.with(|t| t.trim().is_empty())
                        on:click=fill
                    >
                        "Fill in the form"
                    </button>
                </div>
            </div>
        </Modal>
    }
}
