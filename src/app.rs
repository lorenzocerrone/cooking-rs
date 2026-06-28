use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::Nav;
use crate::models::MealPlan;
use crate::pages::{IngredientsPage, PlannerPage, RecipeDetailPage, RecipesPage};
use crate::state::{read_hash, write_hash};

/// Router base path. Empty for local dev (served at `/`); the deploy workflow sets
/// `APP_BASE_PATH=/cooking-rs` so routes/links resolve under the GitHub Pages subpath.
/// Must match Trunk's `--public-url`.
pub const BASE_PATH: &str = match option_env!("APP_BASE_PATH") {
    Some(p) => p,
    None => "",
};

/// Root component: the nav shell plus client-side routing.
#[component]
pub fn App() -> impl IntoView {
    // The meal plan is the one piece of shared, app-wide state. It lives here so
    // any page can read or update it (the planner edits it; recipe pages add to
    // it), and it is mirrored into the URL hash for sharing. Seed from the hash so
    // shared links restore regardless of which route is opened first.
    let plan = RwSignal::new(read_hash().unwrap_or_default());
    provide_context(plan);

    // Keep the URL hash in sync on every change. Skip the initial run when the
    // plan is empty so we don't stamp an empty `#plan=` onto plain page loads.
    Effect::new(move |prev: Option<()>| {
        let current = plan.get();
        if prev.is_none() && current == MealPlan::default() {
            return;
        }
        write_hash(&current);
    });

    view! {
        <Router base=BASE_PATH>
            <Nav />
            <main>
                <Routes fallback=|| view! { <p>"Page not found."</p> }>
                    <Route path=path!("/") view=RecipesPage />
                    <Route path=path!("/recipe/:id") view=RecipeDetailPage />
                    <Route path=path!("/ingredients") view=IngredientsPage />
                    <Route path=path!("/planner") view=PlannerPage />
                </Routes>
            </main>
        </Router>
    }
}
