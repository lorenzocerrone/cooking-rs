use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::app::{href, use_app_data, use_grocery, use_plan};
use crate::components::{IconBook, IconCalendar, IconCart};
use crate::shopping::aggregate;

/// The three tabs: a bottom bar on phones, a top bar on wider screens (CSS). The
/// grocery tab shows how many items are still to buy. Hidden on the recipe page
/// so the recipe gets the whole screen.
#[component]
pub fn Nav() -> impl IntoView {
    let data = use_app_data();
    let plan = use_plan();
    let grocery = use_grocery();
    let remaining = Memo::new(move |_| {
        data.get()
            .map(|d| {
                let cats = aggregate(&plan.get(), &d.recipes, &d.catalog);
                grocery.with(|g| g.remaining(&cats))
            })
            .unwrap_or(0)
    });

    let location = use_location();
    let recipe_prefix = href("/recipe/");
    let on_recipe = move || location.pathname.get().starts_with(&recipe_prefix);

    view! {
        <nav class="nav" class:nav--hidden=on_recipe>
            <span class="nav__brand">
                "cooking-rs"
                <small>"weekly planner"</small>
            </span>
            <A href=href("/") exact=true attr:class="nav__tab">
                <IconBook />
                <span>"Recipes"</span>
            </A>
            <A href=href("/planner") attr:class="nav__tab">
                <IconCalendar />
                <span>"Week"</span>
            </A>
            <A href=href("/grocery") attr:class="nav__tab">
                <IconCart />
                <span>"Grocery"</span>
                <Show when=move || remaining.get() != 0>
                    <span class="nav__badge mono">{move || remaining.get()}</span>
                </Show>
            </A>
        </nav>
    }
}
