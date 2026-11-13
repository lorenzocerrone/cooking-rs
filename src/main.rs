use cooking_rs::app::{redirected_path, App, BASE_PATH};
use cooking_rs::pwa;

fn main() {
    console_error_panic_hook::set_once();
    restore_redirected_path();
    pwa::setup();
    leptos::mount::mount_to_body(App);
}

/// Back from the site-wide 404 fallback (see [`redirected_path`]): put the
/// deep link back in the address bar before the router reads it.
fn restore_redirected_path() {
    let window = leptos::prelude::window();
    let location = window.location();
    let Some(path) = location.search().ok().and_then(|s| redirected_path(&s)) else {
        return;
    };
    let hash = location.hash().unwrap_or_default();
    if let Ok(history) = window.history() {
        let url = format!("{BASE_PATH}{path}{hash}");
        let _ =
            history.replace_state_with_url(&leptos::wasm_bindgen::JsValue::NULL, "", Some(&url));
    }
}
