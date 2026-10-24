mod app;
mod components;
mod extras;
mod filter;
mod format;
mod models;
mod pages;
mod pwa;
mod quantity;
mod recipes;
mod shopping;
mod state;
mod storage;
#[cfg(test)]
mod validate;

use app::App;

fn main() {
    console_error_panic_hook::set_once();
    pwa::setup();
    leptos::mount::mount_to_body(App);
}
