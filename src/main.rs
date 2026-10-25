use cooking_rs::{app::App, pwa};

fn main() {
    console_error_panic_hook::set_once();
    pwa::setup();
    leptos::mount::mount_to_body(App);
}
