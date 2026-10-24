//! Progressive-web-app wiring: register the service worker and link the manifest.
//!
//! These references must be **absolute and base-path aware** so they resolve
//! correctly even on deep links under the GitHub Pages subpath (e.g.
//! `/cooking-rs/recipe/carbonara`), where document-relative URLs would break.
//! We therefore inject them from Rust using the compile-time [`BASE_PATH`] rather
//! than from static (relative) tags in `index.html`.

use leptos::prelude::window;

use crate::app::BASE_PATH;

/// Inject the manifest `<link>` and register the service worker. Best-effort:
/// any failure (e.g. service workers unavailable) is silently ignored.
pub fn setup() {
    let Some(document) = window().document() else {
        return;
    };

    // <link rel="manifest" href="{base}/manifest.webmanifest">
    if let (Ok(link), Some(head)) = (document.create_element("link"), document.head()) {
        let _ = link.set_attribute("rel", "manifest");
        let _ = link.set_attribute("href", &format!("{BASE_PATH}/manifest.webmanifest"));
        let _ = head.append_child(&link);
    }

    // A service worker sitting in front of a live-reloading dev server is a classic
    // source of stale, broken assets (e.g. images that "won't load" only on your
    // machine). Only register in production. On localhost we skip registration; the
    // worker also self-unregisters there (see `sw.js`) so any left over from an
    // earlier session is torn down on the next load.
    if is_local_dev() {
        return;
    }

    // navigator.serviceWorker.register("{base}/sw.js")
    let _ = window()
        .navigator()
        .service_worker()
        .register(&format!("{BASE_PATH}/sw.js"));
}

/// True when served from a local dev host (`trunk serve`).
fn is_local_dev() -> bool {
    window()
        .location()
        .hostname()
        .map(|h| h == "localhost" || h == "127.0.0.1")
        .unwrap_or(false)
}
