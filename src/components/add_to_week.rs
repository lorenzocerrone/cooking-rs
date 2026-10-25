use leptos::prelude::*;

use crate::app::{notify, use_plan, use_today};
use crate::components::{DayPick, Modal};
use crate::date::Date;
use crate::format::format_day_inline;

/// "Add to week": a button that opens a sheet listing the coming days (see
/// [`DayPick`]); tapping one appends the recipe to it. Used on the recipe page.
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
    let add = Callback::new(move |date: Date| {
        let id = recipe_id.get_value();
        plan.update(|p| p.add_meal(date, id));
        open.set(false);
        notify(format!(
            "Added to {}",
            format_day_inline(date, today.get_untracked())
        ));
    });

    view! {
        <button type="button" class="btn btn--primary" on:click=move |_| open.set(true)>
            "Add to week"
        </button>
        <Modal open=open title=title>
            <DayPick on_pick=add />
        </Modal>
    }
}
