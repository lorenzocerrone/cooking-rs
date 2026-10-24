use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::href;

/// Top navigation bar linking the app's pages.
#[component]
pub fn Nav() -> impl IntoView {
    view! {
        <nav>
            <A href=href("/") exact=true>"Recipes"</A>
            <A href=href("/planner")>"Meal Planner"</A>
            <A href=href("/grocery")>"Grocery List"</A>
        </nav>
    }
}
