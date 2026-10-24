use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{use_grocery, BASE_PATH};
use crate::components::{
    IconClose, IconInfo, IconPlus, IconShuffle, Modal, RecipeOverview, RecipePicker,
};
use crate::format::{asset_url, format_line_quantity, format_text, format_timing};
use crate::models::{IngredientCatalog, MealPlan, Recipe, Slot, DAY_NAMES};
use crate::recipes::load_data;
use crate::shopping::{aggregate, ShoppingCategory};
use crate::storage::GroceryState;

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
        <p class="muted">"Saved on this device as you go — copy the page URL to share your plan."</p>

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

    // Grocery check-offs and view options: shared, persisted per device.
    let grocery = use_grocery();

    // Forget ticks for lines that disappeared or changed amount.
    Effect::new(move |_| {
        let lines: Vec<_> = shopping.get().into_iter().flat_map(|c| c.lines).collect();
        let mut g = grocery.get_untracked();
        if g.prune_checked(&lines) {
            grocery.set(g);
        }
    });

    // The list actually shown/exported, after the "omit to taste" toggle and the
    // user's "have at home" omissions.
    let visible = Memo::new(move |_| {
        let mut cats = shopping.get();
        grocery.with(|g| {
            for cat in cats.iter_mut() {
                cat.lines.retain(|line| {
                    let shown_to_taste = !g.hide_to_taste || !line.subtotals.is_empty();
                    shown_to_taste && !g.is_omitted(&line.key)
                });
            }
        });
        cats.retain(|cat| !cat.lines.is_empty());
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
                    prop:checked=move || grocery.with(|g| g.hide_to_taste)
                    on:change=move |ev| {
                        let v = event_target_checked(&ev);
                        grocery.update(|g| g.hide_to_taste = v);
                    }
                />
                " Omit \"to taste\" items"
            </label>
        </div>
        <ShoppingList categories=visible grocery=grocery catalog=catalog />
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
    grocery: RwSignal<GroceryState>,
    catalog: StoredValue<IngredientCatalog>,
) -> impl IntoView {
    // Names of the ingredients the user marked as "have at home", for restoring.
    let omitted = move || {
        grocery.with(|g| {
            g.omitted
                .iter()
                .map(|key| {
                    let name = catalog.with_value(|c| {
                        c.get(key)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|| key.clone())
                    });
                    (key.clone(), name)
                })
                .collect::<Vec<_>>()
        })
    };

    view! {
        {move || {
            let cats = categories.get();
            if cats.is_empty() {
                view! { <p class="muted">"No meals selected yet."</p> }.into_any()
            } else {
                cats.into_iter()
                    .map(|cat| {
                        view! {
                            <section>
                                <h3>{cat.name}</h3>
                                <ul class="shopping-items">
                                    {cat.lines
                                        .into_iter()
                                        .map(|line| {
                                            let qty = format_line_quantity(&line);
                                            let label = format!("{}: {}", line.name, qty);
                                            // Keyed by catalog key: stable across renames.
                                            let key = StoredValue::new(line.key.clone());
                                            let is_checked = move || {
                                                key.with_value(|k| grocery.with(|g| g.is_checked(k)))
                                            };
                                            let line = StoredValue::new(line);
                                            view! {
                                                <li class:checked=is_checked>
                                                    <label>
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=is_checked
                                                            on:change=move |_| {
                                                                line.with_value(|l| grocery.update(|g| g.toggle_checked(l)));
                                                            }
                                                        />
                                                        {label}
                                                    </label>
                                                    <button
                                                        type="button"
                                                        class="btn-link shopping-skip"
                                                        title="I have this at home — leave it off the list"
                                                        on:click=move |_| {
                                                            key.with_value(|k| grocery.update(|g| g.toggle_omitted(k)));
                                                        }
                                                    >
                                                        "have it"
                                                    </button>
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
        <Show when=move || !omitted().is_empty()>
            <p class="muted shopping-omitted">
                "At home (left off the list): "
                {move || {
                    omitted()
                        .into_iter()
                        .map(|(key, name)| {
                            view! {
                                <button
                                    type="button"
                                    class="btn-link"
                                    title="Put it back on the list"
                                    on:click=move |_| grocery.update(|g| g.toggle_omitted(&key))
                                >
                                    {name}
                                    " ×"
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </p>
        </Show>
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
