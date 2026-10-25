use leptos::prelude::*;

use crate::components::{IconClose, Modal};
use crate::filter::{filter, option_count, Diet, Facets, FilterOption, RecipeQuery, TIME_CAPS};
use crate::models::Recipe;

/// Search box, a "Filters · N" button and the active filters as removable
/// chips. The button opens the filter sheet (a bottom sheet on phones); with
/// `inline` the sections expand in place instead, for use inside another
/// sheet (the recipe picker). Drives a shared [`RecipeQuery`] so the recipes
/// page and the picker behave the same.
#[component]
pub fn RecipeFilters(
    query: RwSignal<RecipeQuery>,
    recipes: StoredValue<Vec<Recipe>>,
    #[prop(optional)] inline: bool,
) -> impl IntoView {
    let facets = StoredValue::new(recipes.with_value(|rs| Facets::of(rs)));
    let panel_open = RwSignal::new(false);
    let active_count = move || query.with(RecipeQuery::active_count);
    let matching = move || recipes.with_value(|rs| query.with(|q| filter(rs, q).len()));

    let active_chip = move |option: FilterOption| {
        let label = option.label();
        view! {
            <button
                type="button"
                class="chip chip--on chip--x"
                title="Remove this filter"
                on:click=move |_| query.update(|q| q.toggle(&option))
            >
                {label}
                <IconClose />
            </button>
        }
    };

    let panel = move || {
        if inline {
            view! {
                <Show when=move || panel_open.get()>
                    <div class="filter-panel">
                        <FilterSections query=query recipes=recipes facets=facets />
                    </div>
                </Show>
            }
            .into_any()
        } else {
            view! {
                <Modal open=panel_open title="Filters">
                    <FilterSections query=query recipes=recipes facets=facets />
                    <div class="filter-foot">
                        <button
                            type="button"
                            class="btn btn--quiet"
                            prop:disabled=move || active_count() == 0
                            on:click=move |_| query.update(RecipeQuery::clear_filters)
                        >
                            "Clear"
                        </button>
                        <button
                            type="button"
                            class="btn btn--primary"
                            on:click=move |_| panel_open.set(false)
                        >
                            {move || match matching() {
                                1 => "Show 1 recipe".to_string(),
                                n => format!("Show {n} recipes"),
                            }}
                        </button>
                    </div>
                </Modal>
            }
            .into_any()
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
                    class="chip chip--filters"
                    class:chip--on=move || { active_count() > 0 }
                    aria-expanded=move || panel_open.get().to_string()
                    on:click=move |_| panel_open.update(|o| *o = !*o)
                >
                    {move || match active_count() {
                        0 => "Filters".to_string(),
                        n => format!("Filters · {n}"),
                    }}
                </button>
                {move || query.with(RecipeQuery::active).into_iter().map(active_chip).collect_view()}
            </div>
            {panel}
        </div>
    }
}

/// The filter sheet's sections: Diet, Author (with two or more authors),
/// Cuisine, Time and Tags. Each option shows how many recipes have it among
/// those the rest of the query lets through (see [`option_count`]); options
/// with none are dimmed.
#[component]
fn FilterSections(
    query: RwSignal<RecipeQuery>,
    recipes: StoredValue<Vec<Recipe>>,
    facets: StoredValue<Facets>,
) -> impl IntoView {
    let option_chip = move |option: FilterOption| {
        let on = {
            let option = option.clone();
            move || query.with(|q| q.is_on(&option))
        };
        let count = {
            let option = option.clone();
            Memo::new(move |_| {
                recipes.with_value(|rs| query.with(|q| option_count(rs, q, &option)))
            })
        };
        let label = match &option {
            // The section heading already says "Author".
            FilterOption::Author(a) => a.clone(),
            other => other.label(),
        };
        let on_dim = on.clone();
        view! {
            <button
                type="button"
                class="chip"
                class:chip--on=on
                class:chip--dim=move || count.get() == 0 && !on_dim()
                on:click=move |_| query.update(|q| q.toggle(&option))
            >
                {label}
                <span class="chip__count">{move || count.get()}</span>
            </button>
        }
    };
    let section = move |title: &'static str, hint: &'static str, options: Vec<FilterOption>| {
        (!options.is_empty()).then(|| {
            view! {
                <section class="filter-section">
                    <h3 class="filter-section__title">
                        {title}
                        <span class="filter-section__hint">{hint}</span>
                    </h3>
                    <div class="chips chips--wrap">
                        {options.into_iter().map(option_chip).collect_view()}
                    </div>
                </section>
            }
        })
    };

    let f = facets.get_value();
    let authors = if f.authors.len() > 1 {
        f.authors
    } else {
        Vec::new()
    };
    view! {
        <div class="filter-sections">
            {section("Diet", "", Diet::ALL.into_iter().map(FilterOption::Diet).collect())}
            {section("Author", "any of", authors.into_iter().map(FilterOption::Author).collect())}
            {section("Cuisine", "any of", f.cuisines.into_iter().map(FilterOption::Cuisine).collect())}
            {section("Time", "prep + cook", TIME_CAPS.into_iter().map(FilterOption::MaxMinutes).collect())}
            {section("Tags", "all of", f.tags.into_iter().map(FilterOption::Tag).collect())}
        </div>
    }
}
