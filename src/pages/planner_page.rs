use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{href, notify, use_app_data, use_own_plan, use_plan, use_today};
use crate::components::{rolling_dates, IconPlus, Modal, RecipePicker, Stepper};
use crate::date::Date;
use crate::format::{format_day, format_total_time};
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
    let own_plan = use_own_plan();
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
    let keep_shared = move |_| {
        own_plan.set(None);
        notify("Plan kept");
    };
    let back_to_mine = move |_| {
        if let Some(own) = own_plan.get_untracked() {
            plan.set(own);
        }
        own_plan.set(None);
    };
    let clear_plan = move |_| {
        if window()
            .confirm_with_message("Clear the whole plan?")
            .unwrap_or(false)
        {
            plan.set(MealPlan::default());
        }
    };

    view! {
        <header class="page-head">
            <h1>"Plan"</h1>
            <button type="button" class="btn" on:click=share prop:disabled=move || is_empty.get()>
                "Share"
            </button>
        </header>

        <Show when=move || own_plan.with(|o| o.is_some())>
            <div class="banner" role="status">
                <span>"Viewing a shared plan."</span>
                <span class="banner__acts">
                    <button type="button" class="btn btn--primary" on:click=keep_shared>"Keep it"</button>
                    <button type="button" class="btn" on:click=back_to_mine>"Back to mine"</button>
                </span>
            </div>
        </Show>

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
                on:click=clear_plan
                prop:disabled=move || is_empty.get()
            >
                "Clear plan"
            </button>
        </p>

        <RecipePicker open=picker_open day=picker_day recipes=recipes plan=plan />
        <MealSheet open=sheet_open target=sheet_target recipes=recipes plan=plan />
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

/// Sheet for one planned meal: servings, link to the recipe, remove.
#[component]
fn MealSheet(
    open: RwSignal<bool>,
    target: RwSignal<(Date, usize)>,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
) -> impl IntoView {
    // The recipe behind the targeted meal (None once it has been removed).
    let recipe = Memo::new(move |_| {
        let (day, idx) = target.get();
        let id = plan.with(|p| p.meal(day, idx).map(|m| m.recipe.clone()))?;
        recipes.with_value(|rs| rs.iter().find(|r| r.id == id).cloned())
    });
    let title = Signal::derive(move || {
        recipe.with(|r| r.as_ref().map(|r| r.title.clone()).unwrap_or_default())
    });
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
    let remove = move |_| {
        let (day, idx) = target.get_untracked();
        plan.update(|p| p.remove_meal(day, idx));
        open.set(false);
    };

    view! {
        <Modal open=open title=title>
            {move || {
                recipe
                    .get()
                    .map(|r| {
                        let link = href(&format!("/recipe/{}", r.id));
                        let meta = format_total_time(&r).unwrap_or_default();
                        view! {
                            <div class="meal-sheet">
                                <div class="recipe__scale">
                                    <Stepper value=servings label="servings" on_change=set_servings />
                                    <span class="mono muted">{meta}</span>
                                </div>
                                <div class="meal-sheet__acts">
                                    <A href=link attr:class="btn" on:click=move |_| open.set(false)>
                                        "View recipe"
                                    </A>
                                    <button type="button" class="btn btn--danger" on:click=remove>
                                        "Remove"
                                    </button>
                                </div>
                            </div>
                        }
                    })
            }}
        </Modal>
    }
}
