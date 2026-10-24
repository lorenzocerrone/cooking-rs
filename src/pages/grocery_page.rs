use leptos::prelude::*;
use leptos_router::components::A;
use web_sys::TouchEvent;

use crate::app::{href, notify, use_app_data, use_grocery, use_plan};
use crate::components::IconClose;
use crate::extras::{parse_extra, suggest, to_extra};
use crate::format::{format_ingredient_amount, format_line_quantity, format_text};
use crate::models::{Extra, IngredientCatalog, MealPlan, Recipe};
use crate::shopping::{aggregate, ShoppingCategory, ShoppingLine};
use crate::storage::GroceryState;

/// Grocery list: everything the week's meals (plus any extras) need, summed in
/// shopping units and grouped by aisle. Ticks and "have it at home" omissions are
/// persisted per device (see `crate::storage`).
#[component]
pub fn GroceryPage() -> impl IntoView {
    let plan = use_plan();
    let data = use_app_data();

    view! {
        <Suspense fallback=|| view! { <p class="muted">"Loading recipes…"</p> }>
            {move || {
                data.get()
                    .map(|d| view! { <GroceryList recipes=d.recipes catalog=d.catalog plan=plan /> })
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
    let grocery = use_grocery();

    // Live shopping list derived from the current plan.
    let shopping = Memo::new(move |_| {
        recipes.with_value(|r| catalog.with_value(|c| aggregate(&plan.get(), r, c)))
    });

    // Forget ticks for lines that disappeared or changed amount.
    Effect::new(move |_| {
        let lines: Vec<_> = shopping.get().into_iter().flat_map(|c| c.lines).collect();
        let mut g = grocery.get_untracked();
        if g.prune_checked(&lines) {
            grocery.set(g);
        }
    });

    // What is shown and exported: after omissions and the "to taste" toggle.
    let visible = Memo::new(move |_| grocery.with(|g| g.visible(&shopping.get())));
    let total = move || visible.with(|v| v.iter().map(|c| c.lines.len()).sum::<usize>());
    let remaining = move || grocery.with(|g| g.remaining(&shopping.get()));
    let meals = move || plan.with(|p| p.meal_count());

    let copy_list = move |_| {
        let text = format_text(&visible.get());
        let _ = window().navigator().clipboard().write_text(&text);
        notify("List copied");
    };

    view! {
        <header class="page-head">
            <h1>"Grocery"</h1>
            <button type="button" class="btn" on:click=copy_list prop:disabled=move || total() == 0>
                "Copy list"
            </button>
        </header>
        <p class="muted summary">
            {move || match (total(), meals()) {
                (0, 0) => "Nothing to buy yet.".to_string(),
                (t, m) => {
                    let meals = match m {
                        0 => "no meals".to_string(),
                        1 => "1 meal".to_string(),
                        n => format!("{n} meals"),
                    };
                    format!("{} of {t} left to buy · {meals} this week", remaining())
                }
            }}
        </p>

        <Show
            when=move || total() != 0
            fallback=move || {
                view! {
                    <p class="muted empty">
                        "Add meals to "
                        <A href=href("/planner")>"the week"</A>
                        " or type an extra below."
                    </p>
                }
            }
        >
            <Aisles categories=visible grocery=grocery />
        </Show>

        <AtHome grocery=grocery catalog=catalog />
        <ExtrasEditor plan=plan catalog=catalog />

        <label class="toggle">
            <input
                type="checkbox"
                prop:checked=move || grocery.with(|g| g.hide_to_taste)
                on:change=move |ev| {
                    let v = event_target_checked(&ev);
                    grocery.update(|g| g.hide_to_taste = v);
                }
            />
            " Hide “to taste” items"
        </label>
    }
}

#[component]
fn Aisles(
    categories: Memo<Vec<ShoppingCategory>>,
    grocery: RwSignal<GroceryState>,
) -> impl IntoView {
    view! {
        {move || {
            categories
                .get()
                .into_iter()
                .map(|cat| {
                    // Unticked first, ticked sink to the bottom; name order within each.
                    let mut lines: Vec<(bool, ShoppingLine)> = grocery.with(|g| {
                        cat.lines
                            .into_iter()
                            .map(|l| (g.is_checked(&l.key), l))
                            .collect()
                    });
                    lines.sort_by_key(|(checked, _)| *checked);
                    let left = lines.iter().filter(|(c, _)| !c).count();
                    view! {
                        <section class="aisle">
                            <h2>
                                <span>{cat.name}</span>
                                <span class="mono">{left}</span>
                            </h2>
                            {lines
                                .into_iter()
                                .map(|(checked, line)| view! { <Row line=line checked=checked grocery=grocery /> })
                                .collect_view()}
                        </section>
                    }
                })
                .collect_view()
        }}
    }
}

/// One shopping line. Tap toggles the tick. Swiping left (touch) reveals "Have
/// it"; on pointer devices the same button shows on hover.
#[component]
fn Row(line: ShoppingLine, checked: bool, grocery: RwSignal<GroceryState>) -> impl IntoView {
    let qty = format_line_quantity(&line);
    let name = line.name.clone();
    let key = StoredValue::new(line.key.clone());
    let line = StoredValue::new(line);
    let revealed = RwSignal::new(false);
    let touch_x = StoredValue::new(None::<i32>);

    let toggle = move |_| {
        if revealed.get_untracked() {
            revealed.set(false);
        } else {
            line.with_value(|l| grocery.update(|g| g.toggle_checked(l)));
        }
    };
    let have_it = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        key.with_value(|k| grocery.update(|g| g.toggle_omitted(k)));
    };
    let on_start = move |ev: TouchEvent| {
        touch_x.set_value(ev.touches().get(0).map(|t| t.client_x()));
    };
    let on_end = move |ev: TouchEvent| {
        let (Some(start), Some(end)) = (
            touch_x.get_value(),
            ev.changed_touches().get(0).map(|t| t.client_x()),
        ) else {
            return;
        };
        touch_x.set_value(None);
        let dx = end - start;
        if dx < -50 {
            revealed.set(true);
        } else if dx > 30 {
            revealed.set(false);
        }
    };

    view! {
        <div class="row" class:row--checked=checked class:row--revealed=move || revealed.get()>
            <button
                type="button"
                class="row__main"
                aria-pressed=checked.to_string()
                on:click=toggle
                on:touchstart=on_start
                on:touchend=on_end
            >
                <span class="row__box" aria-hidden="true"></span>
                <span class="row__name">{name}</span>
                <span class="row__qty mono">{qty}</span>
            </button>
            <button
                type="button"
                class="row__have"
                title="I have this at home: leave it off the list"
                on:click=have_it
            >
                "Have it"
            </button>
        </div>
    }
}

/// Ingredients marked "have it": chips that put them back on the list.
#[component]
fn AtHome(
    grocery: RwSignal<GroceryState>,
    catalog: StoredValue<IngredientCatalog>,
) -> impl IntoView {
    let omitted = move || {
        grocery.with(|g| {
            g.omitted
                .iter()
                .map(|key| {
                    let name = catalog.with_value(|c| {
                        c.get(key)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|| key.trim_start_matches("custom:").to_string())
                    });
                    (key.clone(), name)
                })
                .collect::<Vec<_>>()
        })
    };
    view! {
        <Show when=move || !omitted().is_empty()>
            <p class="chips chips--wrap at-home">
                <span class="eyebrow">"At home"</span>
                {move || {
                    omitted()
                        .into_iter()
                        .map(|(key, name)| {
                            view! {
                                <button
                                    type="button"
                                    class="chip chip--x"
                                    title="Put it back on the list"
                                    on:click=move |_| grocery.update(|g| g.toggle_omitted(&key))
                                >
                                    {name}
                                    <IconClose />
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </p>
        </Show>
    }
}

/// Staples and snacks outside any recipe, typed as one line ("milk 1 l").
/// Catalog matches are suggested while typing and keep their aisle; anything
/// else lands in "Other". Extras live in the plan, so they travel with a share link.
#[component]
fn ExtrasEditor(
    plan: RwSignal<MealPlan>,
    catalog: StoredValue<IngredientCatalog>,
) -> impl IntoView {
    let input = RwSignal::new(String::new());
    let parsed = Memo::new(move |_| parse_extra(&input.get()));
    let suggestions = Memo::new(move |_| catalog.with_value(|c| suggest(c, &parsed.get().name, 5)));

    let add = move |key: Option<String>| {
        let p = parsed.get_untracked();
        if p.name.trim().is_empty() && key.is_none() {
            return;
        }
        let extra = catalog.with_value(|c| {
            let pick = key.as_deref().and_then(|k| c.get(k).map(|info| (k, info)));
            to_extra(&p, pick)
        });
        plan.update(|pl| pl.extras.push(extra));
        input.set(String::new());
    };
    let on_key = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Enter" {
            ev.prevent_default();
            let first = suggestions.with_untracked(|s| s.first().map(|s| s.key.clone()));
            add(first);
        }
    };

    let current = move || {
        plan.with(|p| {
            p.extras
                .iter()
                .enumerate()
                .map(|(i, extra)| {
                    let label = match extra {
                        Extra::Catalog(item) => catalog.with_value(|c| {
                            let name = c
                                .get(&item.ingredient)
                                .map(|x| x.name.clone())
                                .unwrap_or_else(|| item.ingredient.clone());
                            match item.amount {
                                Some(_) => format!("{name} {}", format_ingredient_amount(item, c)),
                                None => name,
                            }
                        }),
                        Extra::Custom { name, quantity } => match quantity {
                            Some(q) => format!("{name} {q}"),
                            None => name.clone(),
                        },
                    };
                    (i, label)
                })
                .collect::<Vec<_>>()
        })
    };

    view! {
        <section class="extras">
            <p class="eyebrow">"Extras"</p>
            <div class="extras__input">
                <input
                    id="extra-input"
                    class="search"
                    type="text"
                    autocomplete="off"
                    placeholder="Add an item, e.g. milk 1 l"
                    aria-label="Add an extra item"
                    prop:value=move || input.get()
                    on:input=move |ev| input.set(event_target_value(&ev))
                    on:keydown=on_key
                />
                <Show when=move || !parsed.with(|p| p.name.trim().is_empty())>
                    <div class="suggest">
                        {move || {
                            suggestions
                                .get()
                                .into_iter()
                                .enumerate()
                                .map(|(i, s)| {
                                    let key = s.key.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="suggest__row"
                                            class:suggest__row--first=i == 0
                                            on:click=move |_| add(Some(key.clone()))
                                        >
                                            <span>{s.name}</span>
                                            <span class="eyebrow">{s.category}</span>
                                        </button>
                                    }
                                })
                                .collect_view()
                        }}
                        <button type="button" class="suggest__row" on:click=move |_| add(None)>
                            <span>{move || format!("Add “{}”", parsed.with(|p| p.name.trim().to_string()))}</span>
                            <span class="eyebrow">"Other"</span>
                        </button>
                    </div>
                </Show>
            </div>
            <Show when=move || plan.with(|p| !p.extras.is_empty())>
                <p class="chips chips--wrap">
                    {move || {
                        current()
                            .into_iter()
                            .map(|(i, label)| {
                                view! {
                                    <button
                                        type="button"
                                        class="chip chip--x"
                                        title="Remove"
                                        on:click=move |_| {
                                            plan.update(|p| {
                                                if i < p.extras.len() {
                                                    p.extras.remove(i);
                                                }
                                            })
                                        }
                                    >
                                        {label}
                                        <IconClose />
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </p>
            </Show>
        </section>
    }
}
