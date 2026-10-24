use leptos::prelude::*;
use leptos_router::components::A;

/// Top navigation bar linking the app's pages.
#[component]
pub fn Nav() -> impl IntoView {
    view! {
        <nav>
            <A href="/">"Recipes"</A>
            <A href="/planner">"Meal Planner"</A>
        </nav>
    }
}
