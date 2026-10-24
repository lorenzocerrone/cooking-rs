use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::{Nav, Toast};
use crate::models::MealPlan;
use crate::pages::{GroceryPage, PlannerPage, RecipeDetailPage, RecipesPage};
use crate::recipes::{load_data, AppData};
use crate::state::{clear_hash, read_hash};
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

/// The recipes and catalog, loaded once for the whole app.
pub type DataResource = Resource<AppData>;

/// Root component: the nav shell plus client-side routing.
#[component]
pub fn App() -> impl IntoView {
    // Shared, app-wide state, provided here so any page can read or update it:
    // - the meal plan (the planner edits it; recipe pages add to it), persisted
    //   locally and exported on demand as a `#plan=` share link;
    // - the grocery check-off state, persisted locally only;
    // - `own_plan`: `Some` while a shared plan is being previewed (the device's
    //   own plan is set aside and nothing is persisted until the user decides);
    // - a one-line toast message;
    // - the recipe data, loaded once.
    let startup = storage::resolve_initial(read_hash(), storage::load());
    let previewing = startup.own_plan.is_some();
    let plan = RwSignal::new(startup.state.plan);
    let grocery = RwSignal::new(startup.state.grocery);
    let own_plan = RwSignal::new(startup.own_plan);
    let toast = RwSignal::new(None::<String>);
    let data: DataResource = Resource::new(|| (), |_| async { load_data().await });
    provide_context(plan);
    provide_context(grocery);
    provide_context(own_plan);
    provide_context(toast);
    provide_context(data);

    // The share link has been read; keep the address bar clean from here on.
    if previewing {
        clear_hash();
    }

    // Persist on every change, except while previewing someone else's plan.
    Effect::new(move |_| {
        let state = PersistedState::new(plan.get(), grocery.get());
        if own_plan.with(|o| o.is_none()) {
            storage::save(&state);
        }
    });

    view! {
        <Router base=BASE_PATH>
            <Nav />
            <main>
                <Routes fallback=|| view! { <p class="muted">"Page not found."</p> }>
                    <Route path=path!("/") view=RecipesPage />
                    <Route path=path!("/recipe/:id") view=RecipeDetailPage />
                    <Route path=path!("/planner") view=PlannerPage />
                    <Route path=path!("/grocery") view=GroceryPage />
                </Routes>
            </main>
            <Toast />
        </Router>
    }
}

/// The shared meal plan.
pub fn use_plan() -> RwSignal<MealPlan> {
    expect_context::<RwSignal<MealPlan>>()
}

/// The shared grocery state.
pub fn use_grocery() -> RwSignal<GroceryState> {
    expect_context::<RwSignal<GroceryState>>()
}

/// The device's own plan while a shared plan is previewed (`None` otherwise).
pub fn use_own_plan() -> RwSignal<Option<MealPlan>> {
    expect_context::<RwSignal<Option<MealPlan>>>()
}

/// Show a short confirmation message ("Link copied").
pub fn notify(message: impl Into<String>) {
    expect_context::<RwSignal<Option<String>>>().set(Some(message.into()));
}

/// The app's recipe data (one load, shared by every page).
pub fn use_app_data() -> DataResource {
    expect_context::<DataResource>()
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
