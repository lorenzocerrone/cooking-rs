use leptos::prelude::*;

use crate::app::{notify, use_today};
use crate::components::{Modal, RecipeFilters};
use crate::date::Date;
use crate::filter::{filter, RecipeQuery};
use crate::format::{format_day_inline, format_total_time};
use crate::models::{MealPlan, Recipe};

/// Searchable recipe picker for a plan day. Tapping a recipe appends it
/// to `day`'s meals in the shared [`MealPlan`]. One instance serves the whole
/// planner: `day` is repointed before opening.
#[component]
pub fn RecipePicker(
    open: RwSignal<bool>,
    day: RwSignal<Date>,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
) -> impl IntoView {
    let query = RwSignal::new(RecipeQuery::default());
    let today = use_today();

    // Start every opening with a clean search.
    Effect::new(move |_| {
        if open.get() {
            query.set(RecipeQuery::default());
        }
    });

    let filtered = move || {
        let q = query.get();
        recipes.with_value(|rs| filter(rs, &q).into_iter().cloned().collect::<Vec<_>>())
    };
    let title =
        Signal::derive(move || format!("Add to {}", format_day_inline(day.get(), today.get())));

    view! {
        <Modal open=open title=title>
            <RecipeFilters query=query recipes=recipes inline=true />
            <div class="pick-list">
                {move || {
                    let list = filtered();
                    if list.is_empty() {
                        view! { <p class="muted">"No recipes match."</p> }.into_any()
                    } else {
                        list.into_iter()
                            .map(|r| {
                                let id = r.id.clone();
                                let meta = [
                                    format_total_time(&r),
                                    Some(match r.servings {
                                        1 => "1 serving".to_string(),
                                        n => format!("{n} servings"),
                                    }),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · ");
                                let pick = move |_| {
                                    let d = day.get_untracked();
                                    plan.update(|p| p.add_meal(d, id.clone()));
                                    open.set(false);
                                    notify(format!(
                                        "Added to {}",
                                        format_day_inline(d, today.get_untracked())
                                    ));
                                };
                                view! {
                                    <button type="button" class="pick-row" on:click=pick>
                                        <span class="pick-row__title">{r.title.clone()}</span>
                                        <span class="pick-row__meta mono">{meta}</span>
                                    </button>
                                }
                            })
                            .collect_view()
                            .into_any()
                    }
                }}
            </div>
        </Modal>
    }
}
