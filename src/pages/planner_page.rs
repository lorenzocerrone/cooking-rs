use std::collections::HashSet;

use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::BASE_PATH;
use crate::components::{
    IconClose, IconInfo, IconPlus, IconShuffle, Modal, RecipeOverview, RecipePicker,
};
use crate::format::{asset_url, format_item_quantity, format_text, format_timing};
use crate::models::{IngredientCatalog, MealPlan, Recipe, Slot, DAY_NAMES};
use crate::recipes::load_data;
use crate::shopping::{aggregate, ShoppingCategory};

/// Weekly meal planner: a responsive grid of day-cards whose state is the shared
/// meal plan (mirrored into the URL hash by `App`), plus a category-grouped
/// shopping list.
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

    // True when the week has no meals and no extras.
    let is_empty = Memo::new(move |_| plan.with(|p| p.is_empty()));

    // Recipe-overview modal state.
    let modal_open = RwSignal::new(false);
    let selected = RwSignal::new(String::new());

    // Shared recipe-picker modal state (repointed before opening).
    let picker_open = RwSignal::new(false);
    let picker_target = RwSignal::new((0usize, Slot::Breakfast));

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

    let fill_random = move |_| fill_random_plan(plan, recipes);
    let clear_week = move |_| {
        if window()
            .confirm_with_message("Clear the whole week?")
            .unwrap_or(false)
        {
            plan.set(MealPlan::default());
        }
    };

    view! {
        <div class="planner-toolbar">
            <button type="button" class="btn-add btn-icon" on:click=fill_random>
                <IconShuffle />
                "Fill randomly"
            </button>
            <button
                type="button"
                class="btn-link"
                on:click=clear_week
                prop:disabled=move || is_empty.get()
            >
                "Clear week"
            </button>
        </div>

        <Show when=move || is_empty.get()>
            <div class="planner-empty">
                <p>"Your week is empty."</p>
                <p class="muted">
                    "Add a recipe to any meal below, or let us fill the week for you."
                </p>
                <button type="button" class="btn-primary btn-icon" on:click=fill_random>
                    <IconShuffle />
                    "Fill randomly"
                </button>
            </div>
        </Show>

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
                                            <SlotCell
                                                day=day
                                                meal_slot=slot
                                                recipes=recipes
                                                plan=plan
                                                selected=selected
                                                modal_open=modal_open
                                                picker_target=picker_target
                                                picker_open=picker_open
                                            />
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })
                .collect_view()}
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

        <RecipePicker
            open=picker_open
            target=picker_target
            recipes=recipes
            plan=plan
        />
    }
}

/// Fill every slot of the week with a random recipe. Seeded from the high-res
/// timer so it varies per click, without pulling in an RNG dependency.
fn fill_random_plan(plan: RwSignal<MealPlan>, recipes: StoredValue<Vec<Recipe>>) {
    let ids: Vec<String> = recipes.with_value(|rs| rs.iter().map(|r| r.id.clone()).collect());
    if ids.is_empty() {
        return;
    }

    // xorshift64, seeded from `performance.now()` (non-zero state required).
    let seed = window()
        .performance()
        .map(|p| p.now())
        .unwrap_or(1.0)
        .to_bits();
    let mut state = seed | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    plan.update(|p| {
        for day in 0..7 {
            for slot in Slot::ALL {
                let idx = (next() % ids.len() as u64) as usize;
                p.set_recipe(day, slot, Some(ids[idx].clone()));
            }
        }
    });
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
fn SlotCell(
    day: usize,
    meal_slot: Slot,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
    selected: RwSignal<String>,
    modal_open: RwSignal<bool>,
    picker_target: RwSignal<(usize, Slot)>,
    picker_open: RwSignal<bool>,
) -> impl IntoView {
    // The chosen recipe for this slot, if any (reactive).
    let chosen = move || {
        plan.with(|p| p.recipe_id(day, meal_slot))
            .and_then(|id| recipes.with_value(|rs| rs.iter().find(|r| r.id == id).cloned()))
    };

    view! {
        <div class="meal-cell">
            {move || match chosen() {
                None => {
                    view! {
                        <button
                            class="slot-add btn-icon"
                            type="button"
                            on:click=move |_| {
                                picker_target.set((day, meal_slot));
                                picker_open.set(true);
                            }
                        >
                            <IconPlus />
                            "Add"
                        </button>
                    }
                        .into_any()
                }
                Some(recipe) => {
                    let recipe_default = recipe.servings;
                    let timing = format_timing(&recipe);
                    let meta = format!("Serves {}{}", recipe_default, timing);
                    let servings_value = move || {
                        plan.with(|p| p.servings(day, meal_slot))
                            .unwrap_or(recipe_default)
                            .to_string()
                    };
                    let thumb = match recipe.image.clone() {
                        Some(src) => {
                            view! {
                                <img
                                    class="meal-chip__thumb"
                                    src=asset_url(BASE_PATH, &src)
                                    alt=recipe.title.clone()
                                />
                            }
                                .into_any()
                        }
                        None => {
                            view! {
                                <div class="meal-chip__thumb recipe-thumb--empty"></div>
                            }
                                .into_any()
                        }
                    };
                    view! {
                        <div class="meal-chip">
                            {thumb}
                            <div class="meal-chip__body">
                                <span class="meal-chip__title" title=recipe.title.clone()>
                                    {recipe.title.clone()}
                                </span>
                                <span class="meal-chip__meta muted">{meta}</span>
                            </div>
                            <input
                                class="servings-input"
                                type="number"
                                min="1"
                                title="Servings"
                                prop:value=servings_value
                                on:change=move |ev| {
                                    let parsed = event_target_value(&ev)
                                        .parse::<u32>()
                                        .ok()
                                        .filter(|n| *n > 0);
                                    // Store None when it matches the recipe default (short URLs).
                                    let servings = if parsed == Some(recipe_default) {
                                        None
                                    } else {
                                        parsed
                                    };
                                    plan.update(|p| p.set_servings(day, meal_slot, servings));
                                }
                            />
                            <button
                                class="info-btn"
                                type="button"
                                title="Recipe overview"
                                aria-label="Recipe overview"
                                on:click=move |_| {
                                    if let Some(id) = plan.with(|p| p.recipe_id(day, meal_slot)) {
                                        selected.set(id);
                                        modal_open.set(true);
                                    }
                                }
                            >
                                <IconInfo />
                            </button>
                            <button
                                class="remove-btn"
                                type="button"
                                title="Remove"
                                aria-label="Remove"
                                on:click=move |_| {
                                    plan.update(|p| p.set_recipe(day, meal_slot, None));
                                }
                            >
                                <IconClose />
                            </button>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}
