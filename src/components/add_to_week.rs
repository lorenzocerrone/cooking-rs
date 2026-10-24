use leptos::prelude::*;

use crate::app::{notify, use_plan};
use crate::components::{today_index, Modal};
use crate::models::DAY_NAMES;

/// "Add to week": a button that opens a sheet listing the seven days; tapping a
/// day appends the recipe to it. Used on the recipe page.
#[component]
pub fn AddToWeek(
    #[prop(into)] recipe_id: String,
    #[prop(into)] recipe_title: String,
) -> impl IntoView {
    let plan = use_plan();
    let open = RwSignal::new(false);
    let recipe_id = StoredValue::new(recipe_id);
    let today = today_index();
    let title = format!("Add “{recipe_title}” to…");

    view! {
        <button type="button" class="btn btn--primary" on:click=move |_| open.set(true)>
            "Add to week"
        </button>
        <Modal open=open title=title>
            <div class="day-pick">
                {DAY_NAMES
                    .iter()
                    .enumerate()
                    .map(|(day, name)| {
                        let count = move || plan.with(|p| p.days[day].meals.len());
                        let add = move |_| {
                            let id = recipe_id.get_value();
                            plan.update(|p| p.add_meal(day, id));
                            open.set(false);
                            notify(format!("Added to {name}"));
                        };
                        view! {
                            <button type="button" class="day-pick__row" on:click=add>
                                <span>
                                    {*name}
                                    {(day == today).then(|| view! { <span class="tag-today">"today"</span> })}
                                </span>
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
                    .collect_view()}
            </div>
        </Modal>
    }
}
