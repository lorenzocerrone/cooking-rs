use leptos::prelude::*;

use crate::app::BASE_PATH;
use crate::components::Modal;
use crate::filter::{available_cuisines, available_tags, filter, RecipeQuery};
use crate::format::{asset_url, format_timing};
use crate::models::{MealPlan, Recipe, Slot, DAY_NAMES};

/// A searchable/filterable recipe picker shown in a modal. Selecting a recipe
/// writes it into the shared [`MealPlan`] for the current `target` (day + slot).
/// A single instance is shared by every slot on the planner (the `target` signal
/// is repointed before opening).
#[component]
pub fn RecipePicker(
    open: RwSignal<bool>,
    target: RwSignal<(usize, Slot)>,
    recipes: StoredValue<Vec<Recipe>>,
    plan: RwSignal<MealPlan>,
) -> impl IntoView {
    let cuisines = StoredValue::new(recipes.with_value(|rs| available_cuisines(rs)));
    let tags = StoredValue::new(recipes.with_value(|rs| available_tags(rs)));

    let search = RwSignal::new(String::new());
    let cuisine = RwSignal::new(String::new());
    let tag = RwSignal::new(String::new());

    // Reset the filters whenever the picker is (re)opened, so a previous search
    // doesn't carry over to the next slot.
    Effect::new(move |_| {
        if open.get() {
            search.set(String::new());
            cuisine.set(String::new());
            tag.set(String::new());
        }
    });

    let filtered = move || {
        let query = RecipeQuery {
            search: search.get(),
            cuisine: cuisine.get(),
            tag: tag.get(),
        };
        recipes.with_value(|rs| filter(rs, &query).into_iter().cloned().collect::<Vec<_>>())
    };

    view! {
        <Modal open=open>
            <div class="recipe-picker">
                <h4>
                    {move || {
                        let (day, slot) = target.get();
                        format!("Choose a recipe — {} · {}", DAY_NAMES[day], slot.label())
                    }}
                </h4>

                <div class="recipe-filters">
                    <input
                        class="search-input"
                        type="search"
                        placeholder="Search recipes…"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                    <PickerSelect label="All cuisines" options=cuisines.get_value() value=cuisine />
                    <PickerSelect label="All tags" options=tags.get_value() value=tag />
                </div>

                <div class="recipe-picker-list">
                    {move || {
                        let list = filtered();
                        if list.is_empty() {
                            view! { <p class="muted">"No recipes match your filters."</p> }
                                .into_any()
                        } else {
                            list.into_iter()
                                .map(|r| {
                                    let id = r.id.clone();
                                    let timing = format_timing(&r);
                                    let alt = r.title.clone();
                                    let pick = move |_| {
                                        let (day, slot) = target.get_untracked();
                                        plan.update(|p| p.set_recipe(day, slot, Some(id.clone())));
                                        open.set(false);
                                    };
                                    view! {
                                        <button
                                            type="button"
                                            class="recipe-picker-row"
                                            on:click=pick
                                        >
                                            {match r.image.clone() {
                                                Some(src) => {
                                                    view! {
                                                        <img
                                                            class="recipe-picker-row__thumb"
                                                            src=asset_url(BASE_PATH, &src)
                                                            alt=alt
                                                        />
                                                    }
                                                        .into_any()
                                                }
                                                None => {
                                                    view! {
                                                        <div class="recipe-picker-row__thumb recipe-thumb--empty"></div>
                                                    }
                                                        .into_any()
                                                }
                                            }}
                                            <div class="recipe-picker-row__body">
                                                <span class="recipe-picker-row__title">
                                                    {r.title.clone()}
                                                </span>
                                                <span class="muted">
                                                    {format!("Serves {}{}", r.servings, timing)}
                                                </span>
                                                <div class="badges">
                                                    {r
                                                        .cuisine
                                                        .clone()
                                                        .map(|c| {
                                                            view! { <span class="badge badge-cuisine">{c}</span> }
                                                        })}
                                                    {r
                                                        .tags
                                                        .clone()
                                                        .into_iter()
                                                        .map(|t| view! { <span class="badge">{t}</span> })
                                                        .collect_view()}
                                                </div>
                                            </div>
                                        </button>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }
                    }}
                </div>
            </div>
        </Modal>
    }
}

#[component]
fn PickerSelect(
    label: &'static str,
    options: Vec<String>,
    value: RwSignal<String>,
) -> impl IntoView {
    view! {
        <select
            prop:value=move || value.get()
            on:change=move |ev| value.set(event_target_value(&ev))
        >
            <option value="">{label}</option>
            {options
                .into_iter()
                .map(|opt| {
                    let v = opt.clone();
                    view! { <option value=v>{opt}</option> }
                })
                .collect_view()}
        </select>
    }
}
