use leptos::prelude::*;
use leptos_router::components::A;
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::app::{href, notify, use_app_data, use_own_plan, use_plan};
use crate::components::{today_index, IconPlus, Modal, RecipePicker, Stepper};
use crate::format::format_total_time;
use crate::models::{MealPlan, Recipe, DAY_NAMES};
use crate::state::current_share_url;

/// The week: seven day cards, each a list of meals plus an "Add a meal" row.
/// State is the shared meal plan (persisted by `App`).
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
    let today = today_index();

    let is_empty = Memo::new(move |_| plan.with(|p| p.is_empty()));

    // One picker and one meal sheet for the whole week, repointed before opening.
    let picker_open = RwSignal::new(false);
    let picker_day = RwSignal::new(0usize);
    let sheet_open = RwSignal::new(false);
    let sheet_target = RwSignal::new((0usize, 0usize));

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
    let clear_week = move |_| {
        if window()
            .confirm_with_message("Clear the whole week?")
            .unwrap_or(false)
        {
            plan.set(MealPlan::default());
        }
    };

    view! {
        <header class="page-head page-head--wide">
            <h1>"This week"</h1>
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
            {(0..7)
                .map(|day| {
                    view! {
                        <DayCard
                            day=day
                            is_today=day == today
                            recipes=recipes
                            plan=plan
                            picker_day=picker_day
                            picker_open=picker_open
                            sheet_target=sheet_target
                            sheet_open=sheet_open
                        />
                    }
                })
                .collect_view()}
        </div>

        <p class="week__foot">
            <button
                type="button"
                class="btn btn--quiet"
                on:click=clear_week
                prop:disabled=move || is_empty.get()
            >
                "Clear week"
            </button>
        </p>

        <RecipePicker open=picker_open day=picker_day recipes=recipes plan=plan />
        <MealSheet open=sheet_open target=sheet_target recipes=recipes plan=plan />
    }
}

#[component]
fn DayCard(
    day: usize,
    is_today: bool,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
    picker_day: RwSignal<usize>,
    picker_open: RwSignal<bool>,
    sheet_target: RwSignal<(usize, usize)>,
    sheet_open: RwSignal<bool>,
) -> impl IntoView {
    // On a phone the week is a long list: open it at today.
    let card = NodeRef::<leptos::html::Section>::new();
    if is_today && day > 0 {
        Effect::new(move |_| {
            let narrow = window()
                .inner_width()
                .ok()
                .and_then(|w| w.as_f64())
                .is_some_and(|w| w < 720.0);
            if let (true, Some(el)) = (narrow, card.get()) {
                let opts = ScrollIntoViewOptions::new();
                opts.set_block(ScrollLogicalPosition::Start);
                el.scroll_into_view_with_scroll_into_view_options(&opts);
            }
        });
    }

    let meals = move || {
        plan.with(|p| {
            p.days[day]
                .meals
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

    view! {
        <section class="day" class:day--today=is_today node_ref=card>
            <h2>
                {DAY_NAMES[day]}
                {is_today.then(|| view! { <span class="tag-today">"today"</span> })}
            </h2>
            {move || {
                meals()
                    .into_iter()
                    .map(|(idx, title, servings)| {
                        view! {
                            <button
                                type="button"
                                class="meal"
                                on:click=move |_| {
                                    sheet_target.set((day, idx));
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
            <button
                type="button"
                class="meal meal--add"
                on:click=move |_| {
                    picker_day.set(day);
                    picker_open.set(true);
                }
            >
                <IconPlus />
                "Add a meal"
            </button>
        </section>
    }
}

/// Sheet for one planned meal: servings, link to the recipe, remove.
#[component]
fn MealSheet(
    open: RwSignal<bool>,
    target: RwSignal<(usize, usize)>,
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
