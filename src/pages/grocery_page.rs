use leptos::prelude::*;

use crate::app::use_grocery;
use crate::format::{format_ingredient_amount, format_line_quantity, format_text};
use crate::models::{IngredientCatalog, MealPlan, Recipe, RecipeIngredient};
use crate::recipes::load_data;
use crate::shopping::{aggregate, ShoppingCategory};
use crate::storage::GroceryState;

/// Grocery list: everything the week's meals (plus any extras) need, summed in
/// shopping units and grouped by aisle. Ticks and "have at home" omissions are
/// persisted per device (see `crate::storage`).
#[component]
pub fn GroceryPage() -> impl IntoView {
    let plan = expect_context::<RwSignal<MealPlan>>();
    let data = Resource::new(|| (), |_| async { load_data().await });

    view! {
        <h1>"Grocery list"</h1>
        <p class="muted">
            "Built from your "
            <leptos_router::components::A href="/planner">"meal plan"</leptos_router::components::A>
            ". Tick things off as you shop; ticks reset when a plan change alters the amount."
        </p>

        <Suspense fallback=|| view! { <p class="muted">"Loading recipes…"</p> }>
            {move || {
                data.get()
                    .map(|d| {
                        view! { <GroceryList recipes=d.recipes catalog=d.catalog plan=plan /> }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn GroceryList(
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

    view! {
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

        <ExtrasEditor plan=plan catalog=catalog />
    }
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

/// Add staples and snacks that aren't part of a recipe (milk, bread, coffee…).
/// They are stored in the plan, so they travel with the share link too.
#[component]
fn ExtrasEditor(
    plan: RwSignal<MealPlan>,
    catalog: StoredValue<IngredientCatalog>,
) -> impl IntoView {
    // Catalog entries sorted by display name, for the picker.
    let options = StoredValue::new(catalog.with_value(|c| {
        let mut v: Vec<(String, String)> =
            c.0.iter()
                .map(|(k, i)| (k.clone(), i.name.clone()))
                .collect();
        v.sort_by(|a, b| a.1.cmp(&b.1));
        v
    }));

    let key = RwSignal::new(String::new());
    let amount = RwSignal::new(String::new());
    let unit = RwSignal::new(String::new());

    let can_add = move || !key.get().is_empty();
    let add = move |_| {
        let k = key.get_untracked();
        if k.is_empty() {
            return;
        }
        let parsed = amount
            .get_untracked()
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|a| *a > 0.0);
        let u = unit.get_untracked().trim().to_string();
        plan.update(|p| {
            p.extras.push(RecipeIngredient {
                ingredient: k,
                amount: parsed,
                unit: (!u.is_empty() && parsed.is_some()).then_some(u),
                note: None,
                optional: false,
            })
        });
        amount.set(String::new());
        unit.set(String::new());
    };

    view! {
        <section class="extras">
            <h3>"Extras"</h3>
            <p class="muted">"Staples and snacks to buy this week that aren't in a recipe."</p>
            <ul class="extras-list">
                {move || {
                    plan.with(|p| {
                        p.extras
                            .iter()
                            .enumerate()
                            .map(|(i, item)| {
                                let (name, qty) = catalog.with_value(|c| {
                                    (
                                        c.get(&item.ingredient)
                                            .map(|x| x.name.clone())
                                            .unwrap_or_else(|| item.ingredient.clone()),
                                        format_ingredient_amount(item, c),
                                    )
                                });
                                view! {
                                    <li>
                                        {format!("{name}: {qty}")}
                                        <button
                                            type="button"
                                            class="btn-link"
                                            title="Remove"
                                            aria-label="Remove"
                                            on:click=move |_| {
                                                plan.update(|p| {
                                                    if i < p.extras.len() {
                                                        p.extras.remove(i);
                                                    }
                                                })
                                            }
                                        >
                                            "×"
                                        </button>
                                    </li>
                                }
                            })
                            .collect_view()
                    })
                }}
            </ul>
            <div class="extras-form">
                <select
                    prop:value=move || key.get()
                    on:change=move |ev| key.set(event_target_value(&ev))
                >
                    <option value="">"Choose an ingredient…"</option>
                    {options
                        .get_value()
                        .into_iter()
                        .map(|(k, name)| view! { <option value=k>{name}</option> })
                        .collect_view()}
                </select>
                <input
                    type="number"
                    min="0"
                    step="any"
                    placeholder="amount"
                    prop:value=move || amount.get()
                    on:input=move |ev| amount.set(event_target_value(&ev))
                />
                <input
                    type="text"
                    placeholder="unit (optional)"
                    prop:value=move || unit.get()
                    on:input=move |ev| unit.set(event_target_value(&ev))
                />
                <button
                    type="button"
                    class="btn-add"
                    prop:disabled=move || !can_add()
                    on:click=add
                >
                    "Add"
                </button>
            </div>
        </section>
    }
}
