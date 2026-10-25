use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, notify, use_app_data, use_grocery, use_plan, use_today};
use crate::components::{rolling_dates, DayPick, IconPlus, Modal, RecipePicker, Stepper};
use crate::date::Date;
use crate::format::{format_day, format_day_inline, format_total_time};
use crate::models::{MealPlan, Recipe};
use crate::state::current_share_url;

/// The plan: the coming days from today (see [`rolling_dates`]). Today and days
/// with meals are cards (meal rows plus "Add a meal"); other days are one thin
/// row; a divider marks each new week. State is the shared meal plan
/// (persisted by `App`).
#[component]
pub fn PlannerPage() -> impl IntoView {
    let plan = use_plan();
    let data = use_app_data();

    view! {
        <Suspense fallback=|| view! { <p class="muted">"Loading recipes…"</p> }>
            {move || data.get().map(|d| view! { <Week recipes=d.recipes plan=plan /> })}
        </Suspense>
    }
}

#[component]
fn Week(recipes: Vec<Recipe>, plan: RwSignal<MealPlan>) -> impl IntoView {
    let recipes = StoredValue::new(recipes);
    let today = use_today();

    let is_empty = Memo::new(move |_| plan.with(|p| p.is_empty()));
    let dates = Memo::new(move |_| plan.with(|p| rolling_dates(today.get(), p)));

    // One picker and one meal sheet for the whole plan, repointed before opening.
    let picker_open = RwSignal::new(false);
    let picker_day = RwSignal::new(today.get_untracked());
    let sheet_open = RwSignal::new(false);
    let sheet_target = RwSignal::new((today.get_untracked(), 0usize));

    let share = move |_| {
        let Some(url) = plan.with_untracked(current_share_url) else {
            return;
        };
        let _ = window().navigator().clipboard().write_text(&url);
        notify("Link copied");
    };
    let clear_open = RwSignal::new(false);

    view! {
        <header class="page-head">
            <h1>"Plan"</h1>
            <button type="button" class="btn" on:click=share prop:disabled=move || is_empty.get()>
                "Share"
            </button>
        </header>

        <div class="week">
            <For each=move || dates.get() key=|date| *date let:date>
                {move || {
                    let t = today.get();
                    (date > t && date.weekday() == 0)
                        .then(|| view! { <p class="week__divider">"next week"</p> })
                }}
                <DayCard
                    date=date
                    today=today
                    recipes=recipes
                    plan=plan
                    picker_day=picker_day
                    picker_open=picker_open
                    sheet_target=sheet_target
                    sheet_open=sheet_open
                />
            </For>
        </div>

        <p class="week__foot">
            <button
                type="button"
                class="btn btn--quiet"
                on:click=move |_| clear_open.set(true)
                prop:disabled=move || is_empty.get()
            >
                "Clear plan"
            </button>
        </p>

        <RecipePicker open=picker_open day=picker_day recipes=recipes plan=plan />
        <MealSheet open=sheet_open target=sheet_target recipes=recipes plan=plan />
        <ClearSheet open=clear_open plan=plan />
    }
}

#[component]
fn DayCard(
    date: Date,
    today: RwSignal<Date>,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
    picker_day: RwSignal<Date>,
    picker_open: RwSignal<bool>,
    sheet_target: RwSignal<(Date, usize)>,
    sheet_open: RwSignal<bool>,
) -> impl IntoView {
    let is_today = move || date == today.get();
    let label = move || format_day(date, today.get());
    let meals = move || {
        plan.with(|p| {
            p.day(date)
                .iter()
                .enumerate()
                .map(|(idx, m)| {
                    let (title, default) = recipes.with_value(|rs| {
                        rs.iter()
                            .find(|r| r.id == m.recipe)
                            .map(|r| (r.title.clone(), r.servings))
                            .unwrap_or_else(|| (format!("Unknown recipe ({})", m.recipe), 1))
                    });
                    (idx, title, m.servings.unwrap_or(default))
                })
                .collect::<Vec<_>>()
        })
    };
    let has_meals = move || plan.with(|p| !p.day(date).is_empty());
    let pick = move |_| {
        picker_day.set(date);
        picker_open.set(true);
    };

    view! {
        <Show
            when=move || has_meals() || is_today()
            fallback=move || {
                view! {
                    <button type="button" class="day day--empty" on:click=pick>
                        <span>{label}</span>
                        <IconPlus />
                    </button>
                }
            }
        >
            <section class="day" class:day--today=is_today>
                <h2>{label}</h2>
                {move || {
                    meals()
                        .into_iter()
                        .map(|(idx, title, servings)| {
                            view! {
                                <button
                                    type="button"
                                    class="meal"
                                    on:click=move |_| {
                                        sheet_target.set((date, idx));
                                        sheet_open.set(true);
                                    }
                                >
                                    <span class="meal__title">{title}</span>
                                    <span class="pill mono" title="Servings">{servings}</span>
                                </button>
                            }
                        })
                        .collect_view()
                }}
                <button type="button" class="meal meal--add" on:click=pick>
                    <IconPlus />
                    "Add a meal"
                </button>
            </section>
        </Show>
    }
}

/// What the meal sheet shows: the meal's controls, or the days to move it to.
#[derive(Clone, Copy, PartialEq)]
enum SheetMode {
    Edit,
    Move,
}

/// Sheet for one planned meal: servings, order within the day, move to another
/// day, link to the recipe, remove.
#[component]
fn MealSheet(
    open: RwSignal<bool>,
    target: RwSignal<(Date, usize)>,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
) -> impl IntoView {
    let today = use_today();
    let mode = RwSignal::new(SheetMode::Edit);
    // Every opening starts on the meal's controls.
    Effect::new(move |_| {
        if open.get() {
            mode.set(SheetMode::Edit);
        }
    });

    // The recipe behind the targeted meal (None once it has been removed).
    let recipe = Memo::new(move |_| {
        let (day, idx) = target.get();
        let id = plan.with(|p| p.meal(day, idx).map(|m| m.recipe.clone()))?;
        recipes.with_value(|rs| rs.iter().find(|r| r.id == id).cloned())
    });
    let title = Signal::derive(move || {
        let name = recipe.with(|r| r.as_ref().map(|r| r.title.clone()).unwrap_or_default());
        match mode.get() {
            SheetMode::Edit => name,
            SheetMode::Move => format!("Move “{name}” to…"),
        }
    });
    let day = Signal::derive(move || target.get().0);
    let servings = Signal::derive(move || {
        let (day, idx) = target.get();
        let default = recipe.with(|r| r.as_ref().map(|r| r.servings).unwrap_or(1));
        plan.with(|p| p.meal(day, idx).and_then(|m| m.servings))
            .unwrap_or(default)
    });
    let set_servings = Callback::new(move |v: u32| {
        let (day, idx) = target.get_untracked();
        let default = recipe.with_untracked(|r| r.as_ref().map(|r| r.servings).unwrap_or(1));
        // Store None when it matches the recipe default (short share links).
        let value = (v != default).then_some(v);
        plan.update(|p| p.set_servings(day, idx, value));
    });
    let day_len = move || plan.with(|p| p.day(day.get()).len());
    // Swap with the neighbour `step` away and keep following the meal.
    let shift = move |step: isize| {
        let (day, idx) = target.get_untracked();
        let Some(to) = idx.checked_add_signed(step) else {
            return;
        };
        let mut swapped = false;
        plan.update(|p| swapped = p.swap_meals(day, idx, to));
        if swapped {
            target.set((day, to));
        }
    };
    let move_to = Callback::new(move |to: Date| {
        let (day, idx) = target.get_untracked();
        let mut moved = None;
        plan.update(|p| moved = p.move_meal(day, idx, to));
        if let Some(new_idx) = moved {
            target.set((to, new_idx));
            open.set(false);
            notify(format!(
                "Moved to {}",
                format_day_inline(to, today.get_untracked())
            ));
        }
    });
    let remove = move |_| {
        let (day, idx) = target.get_untracked();
        plan.update(|p| p.remove_meal(day, idx));
        open.set(false);
    };

    view! {
        <Modal open=open title=title>
            <Show
                when=move || mode.get() == SheetMode::Edit
                fallback=move || view! { <DayPick on_pick=move_to exclude=day /> }
            >
                {move || {
                    recipe
                        .get()
                        .map(|r| {
                            let link = href(&format!("/recipe/{}", r.id));
                            let meta = format_total_time(&r).unwrap_or_default();
                            view! {
                                <div class="meal-sheet">
                                    <div class="recipe__scale">
                                        <Stepper
                                            value=servings
                                            label="servings"
                                            on_change=set_servings
                                        />
                                        <span class="mono muted">{meta}</span>
                                    </div>
                                    <Show when=move || { day_len() > 1 }>
                                        <div class="meal-sheet__order">
                                            <button
                                                type="button"
                                                class="btn"
                                                on:click=move |_| shift(-1)
                                                prop:disabled=move || target.get().1 == 0
                                            >
                                                "↑ Earlier"
                                            </button>
                                            <button
                                                type="button"
                                                class="btn"
                                                on:click=move |_| shift(1)
                                                prop:disabled=move || {
                                                    target.get().1 + 1 >= day_len()
                                                }
                                            >
                                                "↓ Later"
                                            </button>
                                        </div>
                                    </Show>
                                    <div class="meal-sheet__acts">
                                        <button
                                            type="button"
                                            class="btn"
                                            on:click=move |_| mode.set(SheetMode::Move)
                                        >
                                            "Move to…"
                                        </button>
                                        <A
                                            href=link
                                            attr:class="btn"
                                            on:click=move |_| open.set(false)
                                        >
                                            "View recipe"
                                        </A>
                                        <button
                                            type="button"
                                            class="btn btn--danger"
                                            on:click=remove
                                        >
                                            "Remove"
                                        </button>
                                    </div>
                                </div>
                            }
                        })
                }}
            </Show>
        </Modal>
    }
}

/// "Clear plan" sheet: drop the meals only (extras stay) or everything. Either
/// way the grocery ticks go; at-home items always stay.
#[component]
fn ClearSheet(open: RwSignal<bool>, plan: RwSignal<MealPlan>) -> impl IntoView {
    let grocery = use_grocery();
    let no_meals = move || plan.with(|p| p.meal_count() == 0);
    let clear = move |everything: bool| {
        plan.update(|p| {
            if everything {
                *p = MealPlan::default();
            } else {
                p.clear_meals();
            }
        });
        grocery.update(|g| g.clear_checked());
        open.set(false);
        notify("Plan cleared");
    };

    view! {
        <Modal open=open title="Clear the plan?">
            <div class="clear-sheet">
                <button
                    type="button"
                    class="btn"
                    on:click=move |_| clear(false)
                    prop:disabled=no_meals
                >
                    "Meals only"
                </button>
                <p class="muted">"Keeps the extras and the items you have at home."</p>
                <button type="button" class="btn btn--danger" on:click=move |_| clear(true)>
                    "Everything"
                </button>
                <p class="muted">"Meals and extras. Items you have at home stay hidden."</p>
            </div>
        </Modal>
    }
}
