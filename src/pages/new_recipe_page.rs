use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, notify, use_app_data};
use crate::components::{IconBack, IconClose, IconPlus, Modal, Stepper};
use crate::editor::{
    default_unit, unit_choices, Field, IngredientRow, Issue, Measure, NewIngredient, RecipeForm,
};
use crate::extras::suggest;
use crate::filter::{Facets, VEGETARIAN_TAG};
use crate::models::{IngredientCatalog, Recipe};
use crate::pages::RecipeBody;
use crate::scaffold::{render_json, slug};
use crate::shopping::CATEGORY_ORDER;
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

/// Everything a part of the editor needs: the form and the catalogs.
#[derive(Clone, Copy)]
struct Editor {
    form: RwSignal<RecipeForm>,
    /// The catalog plus this form's new ingredients.
    catalog: Memo<IngredientCatalog>,
    /// The checked recipe, or what is still missing.
    result: Memo<Result<Recipe, Vec<Issue>>>,
    /// Which row the "new ingredient" sheet is for.
    new_for: RwSignal<Option<usize>>,
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

    /// Set row `i` to the catalog ingredient `key`, starting from its usual unit.
    fn pick(self, i: usize, key: String) {
        let info = self.catalog.with_untracked(|c| c.get(&key).cloned());
        if let Some(info) = info {
            self.update_row(i, |row| {
                row.name = info.name.clone();
                row.unit = default_unit(&info);
                row.key = Some(key);
            });
        }
    }
}

#[component]
fn RecipeEditor(recipes: Vec<Recipe>, catalog: IngredientCatalog) -> impl IntoView {
    let form = RwSignal::new(load_form());
    Effect::new(move |_| form.with(save_form));

    let base = StoredValue::new(catalog);
    let full = Memo::new(move |_| form.with(|f| base.with_value(|b| f.catalog_with(b))));
    let result = Memo::new(move |_| form.with(|f| base.with_value(|b| f.to_recipe(b))));
    let ed = Editor {
        form,
        catalog: full,
        result,
        new_for: RwSignal::new(None),
    };
    let facets = StoredValue::new(Facets::of(&recipes));
    let recipes = StoredValue::new(recipes);
    let taken = move || {
        let id = result.with(|r| r.as_ref().ok().map(|r| r.id.clone()))?;
        recipes.with_value(|rs| rs.iter().find(|r| r.id == id).map(|r| r.title.clone()))
    };

    let start_over = RwSignal::new(false);
    let copy_json = move |_| {
        let Ok(recipe) = result.get_untracked() else {
            return;
        };
        let mut text = render_json(&recipe);
        let new_lines: Vec<String> = form.with_untracked(|f| {
            f.used_new_ingredients()
                .into_iter()
                .map(NewIngredient::catalog_line)
                .collect()
        });
        if !new_lines.is_empty() {
            text.push_str("\nNew in ingredients.json:\n");
            text.push_str(&new_lines.join("\n"));
            text.push('\n');
        }
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
                                <RecipeBody recipe=recipe catalog=full.get() />
                            </article>
                        }
                    })
            }}
        </article>

        <NewIngredientSheet ed=ed />

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
        if !tag.is_empty() && tag != VEGETARIAN_TAG {
            form.update(|f| {
                if !f.tags.contains(&tag) {
                    f.tags.push(tag);
                }
            });
        }
        new_tag.set(String::new());
    };
    let tag_options = move || {
        let mut all = facets.with_value(|f| f.tags.clone());
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
                    "“vegetarian” is added for you when there is no meat or fish."
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

#[component]
fn IngredientRowView(ed: Editor, i: usize) -> impl IntoView {
    let picked = Memo::new(move |_| {
        ed.form
            .with(|f| f.ingredients.get(i).and_then(|r| r.key.clone()))
    });
    let remove = move |_| {
        ed.form.update(|f| {
            if i < f.ingredients.len() {
                f.ingredients.remove(i);
            }
        })
    };

    view! {
        <li class="ing-row">
            <Show
                when=move || picked.with(Option::is_some)
                fallback=move || view! { <IngredientSearch ed=ed i=i /> }
            >
                <PickedIngredient ed=ed i=i picked=picked />
            </Show>
            <button type="button" class="icon-btn" aria-label="Remove ingredient" on:click=remove>
                <IconClose />
            </button>
        </li>
    }
}

/// A row with no ingredient yet: type, then pick a suggestion or add a new one.
#[component]
fn IngredientSearch(ed: Editor, i: usize) -> impl IntoView {
    let name = Memo::new(move |_| ed.row(i).name);
    let suggestions = move || {
        let typed = name.get();
        ed.catalog.with(|c| suggest(c, &typed, SUGGESTIONS))
    };
    view! {
        <div class="ing-row__body">
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
            />
            <Show when=move || !name.with(|n| n.trim().is_empty())>
                <div class="suggest">
                    {move || {
                        suggestions()
                            .into_iter()
                            .enumerate()
                            .map(|(n, s)| {
                                let key = s.key.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="suggest__row"
                                        class:suggest__row--first=n == 0
                                        on:click=move |_| ed.pick(i, key.clone())
                                    >
                                        <span>{s.name}</span>
                                        <span class="eyebrow">{s.category}</span>
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                    <button
                        type="button"
                        class="suggest__row"
                        on:click=move |_| ed.new_for.set(Some(i))
                    >
                        <span>{move || format!("New ingredient “{}”…", name.get().trim())}</span>
                        <span class="eyebrow">"Add"</span>
                    </button>
                </div>
            </Show>
        </div>
    }
}

/// A row with its ingredient chosen: amount and unit, note, optional.
#[component]
fn PickedIngredient(ed: Editor, i: usize, picked: Memo<Option<String>>) -> impl IntoView {
    let info = move || {
        picked
            .get()
            .and_then(|k| ed.catalog.with(|c| c.get(&k).cloned()))
    };
    let row = move || ed.row(i);
    let issue = move || {
        ed.result.with(|r| {
            r.as_ref()
                .err()
                .and_then(|issues| issues.iter().find(|x| x.field == Field::Ingredient(i)))
                .map(|x| x.message.clone())
        })
    };
    view! {
        <div class="ing-row__body">
            <button
                type="button"
                class="ing-row__name"
                title="Change the ingredient"
                on:click=move |_| ed.update_row(i, |r| r.key = None)
            >
                {move || info().map(|x| x.name).unwrap_or_default()}
                <small class="muted">
                    {move || info().map(|x| format!(" · {}", x.category)).unwrap_or_default()}
                </small>
            </button>
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
                <select
                    class="field__input"
                    aria-label="Unit"
                    prop:disabled=move || row().amount.trim().is_empty()
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        ed.update_row(i, |r| r.unit = v);
                    }
                >
                    {move || {
                        let current = row().unit;
                        info()
                            .map(|x| unit_choices(&x))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|c| {
                                let selected = c.value == current;
                                view! {
                                    <option value=c.value selected=selected>
                                        {c.label}
                                    </option>
                                }
                            })
                            .collect_view()
                    }}
                </select>
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
        </div>
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

/// The sheet for an ingredient that isn't in the catalog yet: its name, aisle
/// and how it is measured.
#[component]
fn NewIngredientSheet(ed: Editor) -> impl IntoView {
    let open = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let aisle = RwSignal::new(String::new());
    let measure = RwSignal::new(Measure::Weight);

    // Opening for a row fills the name in from what was typed there.
    Effect::new(move |_| {
        if let Some(i) = ed.new_for.get() {
            // Untracked: picking the ingredient changes the row, which must not
            // reopen the sheet.
            let typed = ed.form.with_untracked(|f| {
                f.ingredients
                    .get(i)
                    .map(|r| r.name.trim().to_string())
                    .unwrap_or_default()
            });
            name.set(typed);
            aisle.set(String::new());
            measure.set(Measure::Weight);
            open.set(true);
        }
    });
    Effect::new(move |_| {
        if !open.get() {
            ed.new_for.set(None);
        }
    });

    // Typing a name the catalog already has just uses that ingredient.
    let existing = move || {
        let key = slug(&name.get());
        ed.catalog.with(|c| c.get(&key).map(|x| x.name.clone()))
    };
    let add = move |_| {
        let Some(i) = ed.new_for.get_untracked() else {
            return;
        };
        let n = NewIngredient::new(
            &name.get_untracked(),
            &aisle.get_untracked(),
            measure.get_untracked(),
        );
        let key = n.key.clone();
        ed.new_for.set(None);
        if ed.catalog.with_untracked(|c| c.get(&key).is_none()) {
            ed.form.update(|f| f.new_ingredients.push(n));
        }
        ed.pick(i, key);
        open.set(false);
    };
    let ready =
        move || !slug(&name.get()).is_empty() && (existing().is_some() || !aisle.get().is_empty());

    view! {
        <Modal open=open title="New ingredient">
            <div class="sheet-form">
                <label class="field">
                    <span class="field__label">"Name"</span>
                    <input
                        type="text"
                        class="field__input"
                        autocomplete="off"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </label>
                {move || {
                    existing()
                        .map(|n| {
                            view! {
                                <p class="field__hint">
                                    {format!("Already in the list as “{n}”: that one will be used.")}
                                </p>
                            }
                        })
                }}
                <Show when=move || existing().is_none()>
                    <div class="field">
                        <span class="field__label">"Aisle"</span>
                        <p class="chips chips--wrap">
                            {CATEGORY_ORDER
                                .iter()
                                .map(|c| {
                                    let c = c.to_string();
                                    let (c2, c3) = (c.clone(), c.clone());
                                    view! {
                                        <button
                                            type="button"
                                            class="chip"
                                            class:chip--on=move || aisle.with(|a| *a == c2)
                                            on:click=move |_| aisle.set(c3.clone())
                                        >
                                            {c}
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </p>
                    </div>
                    <div class="field">
                        <span class="field__label">"Bought and measured"</span>
                        <p class="chips chips--wrap">
                            {Measure::ALL
                                .into_iter()
                                .map(|m| {
                                    view! {
                                        <button
                                            type="button"
                                            class="chip"
                                            class:chip--on=move || measure.get() == m
                                            on:click=move |_| measure.set(m)
                                        >
                                            {m.label()}
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </p>
                        <p class="field__hint">
                            {move || match measure.get() {
                                Measure::Weight => "In grams, like flour or cheese.",
                                Measure::Volume => "In millilitres, like milk or stock.",
                                Measure::Count => "By the piece, like eggs or lemons.",
                            }}
                        </p>
                    </div>
                </Show>
                <div class="form-actions">
                    <button
                        type="button"
                        class="btn btn--primary"
                        prop:disabled=move || !ready()
                        on:click=add
                    >
                        "Use this ingredient"
                    </button>
                </div>
            </div>
        </Modal>
    }
}
