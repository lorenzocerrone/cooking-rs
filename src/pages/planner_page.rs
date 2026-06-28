use std::collections::HashSet;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::{MacroSummary, Modal, RecipeOverview};
use crate::format::{format_item_quantity, format_kcal, format_macros, format_text};
use crate::models::{IngredientCatalog, MealPlan, Recipe, Slot, DAY_NAMES};
use crate::nutrition::{day_macros, week_macros};
use crate::recipes::load_data;
use crate::shopping::{aggregate, ShoppingCategory};

/// Weekly meal planner: a responsive grid of day-cards whose state is the shared
/// meal plan (mirrored into the URL hash by `App`), plus per-day/week macros and a
/// category-grouped shopping list.
#[component]
pub fn PlannerPage() -> impl IntoView {
    // Shared, app-wide meal plan (provided by `App`, synced to the URL hash there).
    let plan = expect_context::<RwSignal<MealPlan>>();

    let data = Resource::new(|| (), |_| async { load_data().await });

    view! {
        <h1>"Weekly Meal Planner"</h1>
        <p class="muted">"Your plan is saved in the page URL — copy it to share."</p>

        <Suspense fallback=|| view! { <p class="muted">"Loading recipes…"</p> }>
            {move || {
                data.get()
                    .map(|d| {
                        view! { <PlannerGrid recipes=d.recipes catalog=d.catalog plan=plan /> }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn PlannerGrid(
    recipes: Vec<Recipe>,
    catalog: IngredientCatalog,
    plan: RwSignal<MealPlan>,
) -> impl IntoView {
    let recipes = StoredValue::new(recipes);
    let catalog = StoredValue::new(catalog);

    // Live, reactive shopping list derived from the current plan.
    let shopping = Memo::new(move |_| {
        recipes.with_value(|r| catalog.with_value(|c| aggregate(&plan.get(), r, c)))
    });

    // Week-total macros (pure aggregation in `crate::nutrition`).
    let week = Memo::new(move |_| {
        recipes.with_value(|r| catalog.with_value(|c| week_macros(&plan.get(), r, c)))
    });

    // Recipe-overview modal state.
    let modal_open = RwSignal::new(false);
    let selected = RwSignal::new(String::new());

    // Shopping-list view options (session-only, not encoded in the URL).
    let hide_to_taste = RwSignal::new(false);
    let checked = RwSignal::new(HashSet::<String>::new());

    // The list actually shown/exported, after applying the "omit to taste" toggle.
    let visible = Memo::new(move |_| {
        let mut cats = shopping.get();
        if hide_to_taste.get() {
            for cat in cats.iter_mut() {
                cat.items.retain(|item| item.total.is_some());
            }
            cats.retain(|cat| !cat.items.is_empty());
        }
        cats
    });

    let copy_list = move |_| {
        let text = format_text(&visible.get());
        let _ = window().navigator().clipboard().write_text(&text);
    };

    view! {
        <div class="planner-grid">
            {(0..7)
                .map(|day| {
                    view! {
                        <div class="day-card">
                            <header>{DAY_NAMES[day]}</header>
                            {Slot::ALL
                                .iter()
                                .map(|&slot| {
                                    view! {
                                        <div class="slot-row">
                                            <span class="slot-label">{slot.label()}</span>
                                            <MealSelect
                                                day=day
                                                meal_slot=slot
                                                recipes=recipes
                                                plan=plan
                                                selected=selected
                                                modal_open=modal_open
                                            />
                                        </div>
                                    }
                                })
                                .collect_view()}
                            <footer class="day-card-cal">
                                {move || {
                                    let m = recipes.with_value(|r| {
                                        catalog.with_value(|c| {
                                            plan.with(|p| day_macros(&p.days[day], r, c))
                                        })
                                    });
                                    view! {
                                        <span class="day-kcal">{format_kcal(m.calories)}</span>
                                        <span class="day-pcf muted">{format_macros(&m)}</span>
                                    }
                                }}
                            </footer>
                        </div>
                    }
                })
                .collect_view()}
        </div>

        <div class="week-summary">
            {move || {
                let w = week.get();
                let avg = format!("avg {} / day", format_kcal(w.calories / 7.0));
                view! { <MacroSummary title="Week total" macros=w subtitle=avg /> }
            }}
        </div>

        <h2>"Shopping list"</h2>
        <div class="shopping-controls">
            <button type="button" class="btn-primary" on:click=copy_list>"Copy to clipboard"</button>
            <label>
                <input
                    type="checkbox"
                    prop:checked=move || hide_to_taste.get()
                    on:change=move |ev| hide_to_taste.set(event_target_checked(&ev))
                />
                " Omit \"to taste\" items"
            </label>
        </div>
        <ShoppingList categories=visible checked=checked />
        <textarea
            class="shopping-list"
            prop:value=move || format_text(&visible.get())
            readonly
            placeholder="Your shopping list will appear here."
        ></textarea>

        <Modal open=modal_open>
            {move || {
                let id = selected.get();
                recipes
                    .with_value(|rs| rs.iter().find(|r| r.id == id).cloned())
                    .map(|recipe| {
                        let cat = catalog.with_value(|c| c.clone());
                        let href = format!("/recipe/{}", recipe.id);
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
}

#[component]
fn ShoppingList(
    categories: Memo<Vec<ShoppingCategory>>,
    checked: RwSignal<HashSet<String>>,
) -> impl IntoView {
    view! {
        {move || {
            let cats = categories.get();
            if cats.is_empty() {
                view! { <p class="muted">"No meals selected yet."</p> }.into_any()
            } else {
                cats.into_iter()
                    .map(|cat| {
                        let cat_name = cat.name.clone();
                        view! {
                            <section>
                                <h3>{cat.name}</h3>
                                <ul class="shopping-items">
                                    {cat.items
                                        .into_iter()
                                        .map(|item| {
                                            let qty = format_item_quantity(&item);
                                            let key = format!("{}\u{0}{}", cat_name, item.name);
                                            let key_class = key.clone();
                                            let key_input = key.clone();
                                            let is_checked_class = move || {
                                                checked.with(|c| c.contains(&key_class))
                                            };
                                            let is_checked_input = move || {
                                                checked.with(|c| c.contains(&key_input))
                                            };
                                            view! {
                                                <li class:checked=is_checked_class>
                                                    <label>
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=is_checked_input
                                                            on:change=move |_| {
                                                                checked
                                                                    .update(|c| {
                                                                        if !c.remove(&key) {
                                                                            c.insert(key.clone());
                                                                        }
                                                                    });
                                                            }
                                                        />
                                                        {format!("{}: {}", item.name, qty)}
                                                    </label>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            </section>
                        }
                    })
                    .collect_view()
                    .into_any()
            }
        }}
    }
}

#[component]
fn MealSelect(
    day: usize,
    meal_slot: Slot,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
    selected: RwSignal<String>,
    modal_open: RwSignal<bool>,
) -> impl IntoView {
    let current = move || {
        plan.with(|p| p.recipe_id(day, meal_slot))
            .unwrap_or_default()
    };
    let has_recipe = move || plan.with(|p| p.recipe_id(day, meal_slot)).is_some();

    // The recipe's own default servings, for the chosen recipe (if any).
    let default_servings = move || {
        plan.with(|p| p.recipe_id(day, meal_slot)).and_then(|id| {
            recipes.with_value(|rs| rs.iter().find(|r| r.id == id).map(|r| r.servings))
        })
    };
    let servings_value = move || {
        plan.with(|p| p.servings(day, meal_slot))
            .or_else(default_servings)
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    view! {
        <div class="meal-cell">
            <select
                prop:value=current
                on:change=move |ev| {
                    let value = event_target_value(&ev);
                    let value = if value.is_empty() { None } else { Some(value) };
                    plan.update(|p| p.set_recipe(day, meal_slot, value));
                }
            >
                <option value="">"—"</option>
                {recipes
                    .with_value(|rs| {
                        rs.iter()
                            .map(|r| {
                                let (id, title) = (r.id.clone(), r.title.clone());
                                view! { <option value=id>{title}</option> }
                            })
                            .collect_view()
                    })}
            </select>
            <Show when=has_recipe>
                <input
                    class="servings-input"
                    type="number"
                    min="1"
                    title="Servings"
                    prop:value=servings_value
                    on:change=move |ev| {
                        let parsed = event_target_value(&ev).parse::<u32>().ok().filter(|n| *n > 0);
                        // Store None when it matches the recipe default (keeps URLs short).
                        let servings = if parsed == default_servings() { None } else { parsed };
                        plan.update(|p| p.set_servings(day, meal_slot, servings));
                    }
                />
                <button
                    class="info-btn"
                    type="button"
                    title="Recipe overview"
                    on:click=move |_| {
                        if let Some(id) = plan.with(|p| p.recipe_id(day, meal_slot)) {
                            selected.set(id);
                            modal_open.set(true);
                        }
                    }
                >
                    "ⓘ"
                </button>
            </Show>
        </div>
    }
}
