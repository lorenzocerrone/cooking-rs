use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::Nav;
use crate::models::MealPlan;
use crate::pages::{GroceryPage, PlannerPage, RecipeDetailPage, RecipesPage};
use crate::state::{read_hash, write_hash};
use crate::storage::{self, GroceryState, PersistedState};

/// Router base path. Empty for local dev (served at `/`); the deploy workflow sets
/// `APP_BASE_PATH=/cooking-rs` so routes/links resolve under the GitHub Pages subpath.
/// Must match Trunk's `--public-url`.
pub const BASE_PATH: &str = match option_env!("APP_BASE_PATH") {
    Some(p) => p,
    None => "",
};

/// Prefix an absolute in-app route with a deploy base path. `leptos_router`
/// applies `Router base` to route *matching* only: an `<A href="/planner">` is
/// emitted verbatim, which under `/cooking-rs/` would leave the app. Every
/// in-app link therefore goes through [`href`].
pub fn route_url(base: &str, path: &str) -> String {
    format!("{base}{path}")
}

/// [`route_url`] against the compile-time [`BASE_PATH`], for `<A href=...>`.
pub fn href(path: &str) -> String {
    route_url(BASE_PATH, path)
}

/// Root component: the nav shell plus client-side routing.
#[component]
pub fn App() -> impl IntoView {
    // Two pieces of shared, app-wide state, provided here so any page can read or
    // update them:
    // - the meal plan (the planner edits it; recipe pages add to it), which is
    //   mirrored into the URL hash for sharing and persisted locally;
    // - the grocery check-off state, persisted locally only.
    // A plan in the hash (a shared link) wins over the stored one on startup.
    let initial = storage::resolve_initial(read_hash(), storage::load());
    let plan = RwSignal::new(initial.plan);
    let grocery = RwSignal::new(initial.grocery);
    provide_context(plan);
    provide_context(grocery);

    // Persist on every change (including the initial merge above).
    Effect::new(move |_| {
        storage::save(&PersistedState::new(plan.get(), grocery.get()));
    });

    // Keep the URL hash in sync so the address bar is always a shareable snapshot.
    // Skip the initial run when the plan is empty so plain page loads don't get an
    // empty `#plan=` stamped on them.
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
                    <Route path=path!("/planner") view=PlannerPage />
                    <Route path=path!("/grocery") view=GroceryPage />
                </Routes>
            </main>
        </Router>
    }
}

/// Convenience for pages: the shared grocery state.
pub fn use_grocery() -> RwSignal<GroceryState> {
    expect_context::<RwSignal<GroceryState>>()
}

#[cfg(test)]
mod tests {
    use super::route_url;

    #[test]
    fn route_url_prefixes_base_path() {
        assert_eq!(route_url("", "/planner"), "/planner");
        assert_eq!(route_url("", "/"), "/");
        assert_eq!(route_url("/cooking-rs", "/planner"), "/cooking-rs/planner");
        assert_eq!(route_url("/cooking-rs", "/"), "/cooking-rs/");
        assert_eq!(
            route_url("/cooking-rs", "/recipe/lentil_dal"),
            "/cooking-rs/recipe/lentil_dal"
        );
    }
}
