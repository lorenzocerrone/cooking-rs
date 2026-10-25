use leptos::prelude::*;
use leptos_router::components::A;
use std::time::Duration;

use web_sys::PointerEvent;

use crate::app::{href, notify, use_app_data, use_grocery, use_plan, use_today};
use crate::components::{IconClose, Modal};
use crate::extras::{parse_extra, suggest, to_extra};
use crate::format::{format_ingredient_amount, format_line_quantity, format_range, format_text};
use crate::models::{Extra, IngredientCatalog, MealPlan, Recipe};
use crate::shopping::{aggregate, ShoppingCategory, ShoppingLine};
use crate::storage::{GroceryRange, GroceryState};

/// Grocery list: everything the planned meals from today through the chosen
/// range (plus any extras) need, summed in shopping units and grouped by aisle. Ticks and "have it at home" omissions are
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
    let today = use_today();

    // The days covered: today through the range's end.
    let until = Memo::new(move |_| {
        let range = grocery.with(|g| g.range);
        plan.with(|p| range.until(p, today.get()))
    });

    // Live shopping list derived from the current plan.
    let shopping = Memo::new(move |_| {
        let (from, to) = (today.get(), until.get());
        plan.with(|p| recipes.with_value(|r| catalog.with_value(|c| aggregate(p, from, to, r, c))))
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
    let meals = move || plan.with(|p| p.entries_in(today.get(), until.get()).count());

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
                    let days = format_range(today.get(), until.get(), today.get());
                    format!(
                        "{} of {t} left to buy · {days} · {meals} · tap to tick, hold for more",
                        remaining()
                    )
                }
            }}
        </p>
        <div class="chips" role="group" aria-label="Days covered">
            {GroceryRange::PRESETS
                .into_iter()
                .map(|(range, label)| {
                    let active = move || grocery.with(|g| g.range == range);
                    view! {
                        <button
                            type="button"
                            class="chip"
                            class:chip--on=active
                            aria-pressed=move || active().to_string()
                            on:click=move |_| grocery.update(|g| g.range = range)
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>

        <Show
            when=move || total() != 0
            fallback=move || {
                view! {
                    <p class="muted empty">
                        "Add meals to "
                        <A href=href("/planner")>"the plan"</A>
                        " or type an extra below."
                    </p>
                }
            }
        >
            <Aisles categories=visible grocery=grocery recipes=recipes />
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
    recipes: StoredValue<Vec<Recipe>>,
) -> impl IntoView {
    // One sheet for every row, repointed before opening (hold a row to open it).
    let sheet_open = RwSignal::new(false);
    let sheet_line = RwSignal::new(None::<ShoppingLine>);

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
                                .map(|(checked, line)| {
                                    view! {
                                        <Row
                                            line=line
                                            checked=checked
                                            grocery=grocery
                                            sheet_line=sheet_line
                                            sheet_open=sheet_open
                                        />
                                    }
                                })
                                .collect_view()}
                        </section>
                    }
                })
                .collect_view()
        }}
        <RowSheet open=sheet_open line=sheet_line grocery=grocery recipes=recipes />
    }
}

/// How long a press must last to count as "hold" rather than a tap.
const HOLD_MS: u64 = 450;

/// One shopping line. A tap toggles the tick; holding the row (finger or mouse)
/// opens its sheet. Pointer events cover both, and a move of more than a few
/// pixels (a scroll) cancels the hold.
#[component]
fn Row(
    line: ShoppingLine,
    checked: bool,
    grocery: RwSignal<GroceryState>,
    sheet_line: RwSignal<Option<ShoppingLine>>,
    sheet_open: RwSignal<bool>,
) -> impl IntoView {
    let qty = format_line_quantity(&line);
    let name = line.name.clone();
    let line = StoredValue::new(line);
    let hold = StoredValue::new(None::<(TimeoutHandle, i32, i32)>);
    // Set when a hold fired, so the click that follows the release is ignored.
    let held = StoredValue::new(false);

    let cancel_hold = move || {
        if let Some((handle, _, _)) = hold.get_value() {
            handle.clear();
            hold.set_value(None);
        }
    };
    let on_down = move |ev: PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        cancel_hold();
        held.set_value(false);
        let open = move || {
            hold.set_value(None);
            held.set_value(true);
            sheet_line.set(Some(line.get_value()));
            sheet_open.set(true);
        };
        if let Ok(handle) = set_timeout_with_handle(open, Duration::from_millis(HOLD_MS)) {
            hold.set_value(Some((handle, ev.client_x(), ev.client_y())));
        }
    };
    let on_move = move |ev: PointerEvent| {
        if let Some((_, x, y)) = hold.get_value() {
            if (ev.client_x() - x).abs() > 8 || (ev.client_y() - y).abs() > 8 {
                cancel_hold();
            }
        }
    };
    let on_up = move |_| cancel_hold();
    let toggle = move |_| {
        if held.get_value() {
            held.set_value(false);
            return;
        }
        line.with_value(|l| grocery.update(|g| g.toggle_checked(l)));
    };

    view! {
        <button
            type="button"
            class="row"
            class:row--checked=checked
            aria-pressed=checked.to_string()
            on:click=toggle
            on:pointerdown=on_down
            on:pointermove=on_move
            on:pointerup=on_up
            on:pointercancel=on_up
            on:pointerleave=on_up
            on:contextmenu=|ev| ev.prevent_default()
        >
            <span class="row__box" aria-hidden="true"></span>
            <span class="row__name">{name}</span>
            <span class="row__qty mono">{qty}</span>
        </button>
    }
}

/// The sheet behind a held row: what it is for, and "I have it at home".
#[component]
fn RowSheet(
    open: RwSignal<bool>,
    line: RwSignal<Option<ShoppingLine>>,
    grocery: RwSignal<GroceryState>,
    recipes: StoredValue<Vec<Recipe>>,
) -> impl IntoView {
    let title = Signal::derive(move || {
        line.with(|l| l.as_ref().map(|l| l.name.clone()).unwrap_or_default())
    });
    let have_it = move |_| {
        if let Some(key) = line.with_untracked(|l| l.as_ref().map(|l| l.key.clone())) {
            grocery.update(|g| g.toggle_omitted(&key));
            notify("Left off the list");
        }
        open.set(false);
    };

    view! {
        <Modal open=open title=title>
            {move || {
                line.get()
                    .map(|l| {
                        let qty = format_line_quantity(&l);
                        let needed_for: Vec<String> = recipes.with_value(|rs| {
                            l.sources
                                .iter()
                                .filter_map(|id| rs.iter().find(|r| &r.id == id).map(|r| r.title.clone()))
                                .collect()
                        });
                        let source_text = if needed_for.is_empty() {
                            "Added as an extra".to_string()
                        } else {
                            format!("Needed for {}", needed_for.join(", "))
                        };
                        view! {
                            <div class="row-sheet">
                                <p class="mono muted">{qty}</p>
                                <p class="row-sheet__for">{source_text}</p>
                                <div class="meal-sheet__acts">
                                    <button type="button" class="btn" on:click=have_it>
                                        "I have it at home"
                                    </button>
                                    <button type="button" class="btn btn--quiet" on:click=move |_| open.set(false)>
                                        "Close"
                                    </button>
                                </div>
                            </div>
                        }
                    })
            }}
        </Modal>
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
