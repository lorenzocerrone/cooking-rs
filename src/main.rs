mod app;
mod components;
mod filter;
mod format;
mod models;
mod nutrition;
mod pages;
mod prefs;
mod pwa;
mod recipes;
mod shopping;
mod state;

use app::App;

fn main() {
    console_error_panic_hook::set_once();
    pwa::setup();
    leptos::mount::mount_to_body(App);
}
