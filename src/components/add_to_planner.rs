use leptos::prelude::*;

use crate::components::Modal;
use crate::models::{MealPlan, Slot, DAY_NAMES};

/// A self-contained "add this recipe to the planner" control: a button that opens
/// a day/meal picker modal and writes into the shared [`MealPlan`]. Reused on the
/// recipe cards and the recipe detail page.
#[component]
pub fn AddToPlanner(
    #[prop(into)] recipe_id: String,
    #[prop(into)] recipe_title: String,
) -> impl IntoView {
    let plan = expect_context::<RwSignal<MealPlan>>();

    let open = RwSignal::new(false);
    let day = RwSignal::new(0usize);
    let slot_idx = RwSignal::new(0usize);
    let confirmation = RwSignal::new(None::<String>);

    view! {
        <div class="add-to-planner">
            <button type="button" class="btn-add" on:click=move |_| open.set(true)>
                "＋ Add to planner"
            </button>
            {move || {
                confirmation.get().map(|m| view! { <span class="confirm-msg">{m}</span> })
            }}

            <Modal open=open>
                {
                    let id = recipe_id.clone();
                    let title = recipe_title.clone();
                    view! {
                        <div class="planner-picker">
                            <h4>{format!("Add “{title}” to:")}</h4>

                            <label>
                                "Day"
                                <select on:change=move |ev| {
                                    day.set(event_target_value(&ev).parse().unwrap_or(0));
                                }>
                                    {DAY_NAMES
                                        .iter()
                                        .enumerate()
                                        .map(|(i, name)| {
                                            view! { <option value=i.to_string()>{*name}</option> }
                                        })
                                        .collect_view()}
                                </select>
                            </label>

                            <label>
                                "Meal"
                                <select on:change=move |ev| {
                                    slot_idx.set(event_target_value(&ev).parse().unwrap_or(0));
                                }>
                                    {Slot::ALL
                                        .iter()
                                        .enumerate()
                                        .map(|(i, slot)| {
                                            view! {
                                                <option value=i.to_string()>{slot.label()}</option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                            </label>

                            <button
                                type="button"
                                class="btn-primary"
                                on:click=move |_| {
                                    let d = day.get_untracked();
                                    let slot = Slot::ALL[slot_idx.get_untracked()];
                                    plan.update(|p| p.set_recipe(d, slot, Some(id.clone())));
                                    confirmation
                                        .set(
                                            Some(
                                                format!("✓ Added to {} · {}", DAY_NAMES[d], slot.label()),
                                            ),
                                        );
                                    open.set(false);
                                }
                            >
                                "Add to plan"
                            </button>
                        </div>
                    }
                }
            </Modal>
        </div>
    }
}
