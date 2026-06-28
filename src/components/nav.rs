use leptos::prelude::*;
use leptos_router::components::A;

/// Top navigation bar linking the two pages.
#[component]
pub fn Nav() -> impl IntoView {
    view! {
        <nav>
            <A href="/">"Recipes"</A>
            <A href="/ingredients">"Ingredients"</A>
            <A href="/planner">"Meal Planner"</A>
        </nav>
    }
}
