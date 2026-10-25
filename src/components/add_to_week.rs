use leptos::prelude::*;

use crate::app::{notify, use_plan, use_today};
use crate::components::{rolling_dates, Modal};
use crate::format::{format_day, format_day_inline};

/// "Add to week": a button that opens a sheet listing the coming days (the same
/// rolling dates as the plan page); tapping one appends the recipe to it. Used
/// on the recipe page.
#[component]
pub fn AddToWeek(
    #[prop(into)] recipe_id: String,
    #[prop(into)] recipe_title: String,
) -> impl IntoView {
    let plan = use_plan();
    let today = use_today();
    let open = RwSignal::new(false);
    let recipe_id = StoredValue::new(recipe_id);
    let title = format!("Add “{recipe_title}” to…");

    view! {
        <button type="button" class="btn btn--primary" on:click=move |_| open.set(true)>
            "Add to week"
        </button>
        <Modal open=open title=title>
            <div class="day-pick">
                {move || {
                    let t = today.get();
                    plan.with_untracked(|p| rolling_dates(t, p))
                        .into_iter()
                        .map(|date| {
                            let label = format_day(date, t);
                            let count = move || plan.with(|p| p.day(date).len());
                            let toast = format_day_inline(date, t);
                            let add = move |_| {
                                let id = recipe_id.get_value();
                                plan.update(|p| p.add_meal(date, id));
                                open.set(false);
                                notify(format!("Added to {toast}"));
                            };
                            view! {
                                <button type="button" class="day-pick__row" on:click=add>
                                    <span>{label}</span>
                                    <span class="mono muted">
                                        {move || match count() {
                                            0 => "empty".to_string(),
                                            1 => "1 meal".to_string(),
                                            n => format!("{n} meals"),
                                        }}
                                    </span>
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </Modal>
    }
}
