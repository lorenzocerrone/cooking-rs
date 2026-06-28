use leptos::prelude::*;

use crate::format::{format_kcal, format_macros};
use crate::models::Macros;

/// A compact macro summary card (title, calories, P/C/F, optional subtitle).
/// Used for the planner's week-total summary. Purely presentational.
#[component]
pub fn MacroSummary(
    #[prop(into)] title: String,
    macros: Macros,
    #[prop(optional, into)] subtitle: Option<String>,
) -> impl IntoView {
    view! {
        <div class="macro-summary">
            <span class="macro-summary-title">{title}</span>
            <span class="macro-summary-kcal">{format_kcal(macros.calories)}</span>
            <span class="macro-summary-macros muted">{format_macros(&macros)}</span>
            {subtitle.map(|s| view! { <span class="macro-summary-sub muted">{s}</span> })}
        </div>
    }
}
