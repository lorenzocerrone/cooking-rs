use leptos::prelude::*;

use crate::filter::RecipeQuery;

/// Search box plus one row of filter chips (tags, then cuisines). One tag and
/// one cuisine can be active at a time; "All" clears both. Drives a shared
/// [`RecipeQuery`] so the recipes page and the picker behave the same.
#[component]
pub fn RecipeFilters(
    query: RwSignal<RecipeQuery>,
    tags: Vec<String>,
    cuisines: Vec<String>,
) -> impl IntoView {
    let all_active = move || query.with(|q| q.tag.is_empty() && q.cuisine.is_empty());
    let tag_chip = |tag: String| {
        let active = {
            let tag = tag.clone();
            move || query.with(|q| q.tag == tag)
        };
        let toggle = {
            let tag = tag.clone();
            move |_| {
                query.update(|q| {
                    q.tag = if q.tag == tag {
                        String::new()
                    } else {
                        tag.clone()
                    };
                })
            }
        };
        view! {
            <button type="button" class="chip" class:chip--on=active on:click=toggle>
                {tag}
            </button>
        }
    };
    let cuisine_chip = |cuisine: String| {
        let active = {
            let c = cuisine.clone();
            move || query.with(|q| q.cuisine == c)
        };
        let toggle = {
            let c = cuisine.clone();
            move |_| {
                query.update(|q| {
                    q.cuisine = if q.cuisine == c {
                        String::new()
                    } else {
                        c.clone()
                    };
                })
            }
        };
        view! {
            <button
                type="button"
                class="chip chip--cuisine"
                class:chip--on=active
                on:click=toggle
            >
                {cuisine}
            </button>
        }
    };

    view! {
        <div class="filters">
            <input
                class="search"
                type="search"
                placeholder="Search recipes"
                aria-label="Search recipes"
                prop:value=move || query.with(|q| q.search.clone())
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    query.update(|q| q.search = v);
                }
            />
            <div class="chips">
                <button
                    type="button"
                    class="chip"
                    class:chip--on=all_active
                    on:click=move |_| {
                        query.update(|q| {
                            q.tag.clear();
                            q.cuisine.clear();
                        })
                    }
                >
                    "All"
                </button>
                {tags.into_iter().map(tag_chip).collect_view()}
                {cuisines.into_iter().map(cuisine_chip).collect_view()}
            </div>
        </div>
    }
}
