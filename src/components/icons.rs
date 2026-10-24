//! Small inline SVG icons. Inline (rather than glyphs/emoji) so they render
//! identically regardless of the user's system fonts, and inherit color via
//! `currentColor` so they match the surrounding text/button.

use leptos::prelude::*;

macro_rules! icon {
    ($(#[$doc:meta])* $name:ident, $body:expr) => {
        $(#[$doc])*
        #[component]
        pub fn $name() -> impl IntoView {
            view! {
                <svg
                    class="icon"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                    inner_html=$body
                ></svg>
            }
        }
    };
}

icon!(
    /// Close / remove "✕".
    IconClose,
    r#"<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>"#
);
icon!(
    /// Plus "＋".
    IconPlus,
    r#"<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>"#
);
icon!(
    /// Back chevron "‹".
    IconBack,
    r#"<polyline points="15 5 8 12 15 19"/>"#
);
icon!(
    /// Recipes tab: a page with lines.
    IconBook,
    r#"<path d="M5 4h10l4 4v12H5z"/><path d="M9 11h6M9 15h6"/>"#
);
icon!(
    /// Week tab: a calendar.
    IconCalendar,
    r#"<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>"#
);
icon!(
    /// Grocery tab: a shopping cart.
    IconCart,
    r#"<path d="M3 4h2l2 12h11l2-8H6"/><circle cx="9" cy="20" r="1.5"/><circle cx="17" cy="20" r="1.5"/>"#
);
