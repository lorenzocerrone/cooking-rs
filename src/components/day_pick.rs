use leptos::prelude::*;

use crate::app::{use_plan, use_today};
use crate::components::rolling_dates;
use crate::date::Date;
use crate::format::format_day;

/// The coming days (the same rolling dates as the plan page), one row each with
/// its meal count; tapping a row calls `on_pick`. Used inside sheets: "Add to
/// week" and moving a meal (which leaves out the meal's own day via `exclude`).
#[component]
pub fn DayPick(
    on_pick: Callback<Date>,
    /// A day not to offer.
    #[prop(optional, into)]
    exclude: Option<Signal<Date>>,
) -> impl IntoView {
    let plan = use_plan();
    let today = use_today();

    view! {
        <div class="day-pick">
            {move || {
                let t = today.get();
                let skip = exclude.map(|e| e.get());
                plan.with_untracked(|p| rolling_dates(t, p))
                    .into_iter()
                    .filter(|date| Some(*date) != skip)
                    .map(|date| {
                        let label = format_day(date, t);
                        let count = move || plan.with(|p| p.day(date).len());
                        view! {
                            <button
                                type="button"
                                class="day-pick__row"
                                on:click=move |_| on_pick.run(date)
                            >
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
    }
}
