use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::use_location;
use leptos_router::path;

use crate::components::{today, Nav, Toast};
use crate::date::Date;
use crate::models::MealPlan;
use crate::pages::{GroceryPage, PlannerPage, RecipeDetailPage, RecipesPage};
use crate::recipes::{load_data, AppData};
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

/// The recipes and catalog, loaded once for the whole app.
pub type DataResource = Resource<AppData>;

/// Root component: the nav shell plus client-side routing.
#[component]
pub fn App() -> impl IntoView {
    // Shared, app-wide state, provided here so any page can read or update it:
    // - the meal plan (the planner edits it; recipe pages add to it), kept in
    //   the address bar as `#plan=` (see `UrlSync`) and remembered locally as
    //   the last plan opened;
    // - the grocery check-off state, persisted locally only;
    // - today's date, which every date-based view starts from;
    // - a one-line toast message;
    // - the recipe data, loaded once.
    let today = RwSignal::new(today());
    let startup = storage::resolve_initial(
        read_hash(today.get_untracked()),
        storage::load(today.get_untracked()),
    );
    let plan = RwSignal::new(startup.plan);
    let grocery = RwSignal::new(startup.grocery);
    let toast = RwSignal::new(None::<String>);
    let data: DataResource = Resource::new(|| (), |_| async { load_data().await });
    provide_context(plan);
    provide_context(grocery);
    provide_context(today);
    provide_context(toast);
    provide_context(data);

    // An installed app can stay open across midnight: roll over when it comes
    // back to the foreground, letting the day that passed drop off the plan.
    let _ = window_event_listener(leptos::ev::visibilitychange, move |_| {
        let now = crate::components::today();
        if now != today.get_untracked() {
            today.set(now);
            plan.update(|p| p.drop_before(now));
        }
    });

    // Remember the last plan and the grocery state on every change.
    Effect::new(move |_| {
        storage::save(&PersistedState::new(plan.get(), grocery.get()));
    });

    view! {
        <Router base=BASE_PATH>
            <UrlSync plan=plan />
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

/// Keeps the plan in the address bar: rewrites `#plan=` after every plan change
/// and after every navigation (router links drop the fragment).
#[component]
fn UrlSync(plan: RwSignal<MealPlan>) -> impl IntoView {
    let location = use_location();
    Effect::new(move |_| {
        location.pathname.track();
        location.search.track();
        plan.track();
        // The router updates its location signals before it pushes the new
        // URL; write once the push has happened.
        set_timeout(
            move || plan.with_untracked(write_hash),
            std::time::Duration::ZERO,
        );
    });
}

/// The shared meal plan.
pub fn use_plan() -> RwSignal<MealPlan> {
    expect_context::<RwSignal<MealPlan>>()
}

/// The shared grocery state.
pub fn use_grocery() -> RwSignal<GroceryState> {
    expect_context::<RwSignal<GroceryState>>()
}

/// Today's date (rolls over when the app returns to the foreground).
pub fn use_today() -> RwSignal<Date> {
    expect_context::<RwSignal<Date>>()
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
