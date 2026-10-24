use std::time::Duration;

use leptos::prelude::*;

/// One-line confirmation ("Link copied") that fades after a moment. Set the
/// message through `crate::app::notify`.
#[component]
pub fn Toast() -> impl IntoView {
    let message = expect_context::<RwSignal<Option<String>>>();
    // Each message gets a generation so an older timer never hides a newer one.
    let generation = StoredValue::new(0u32);

    Effect::new(move |_| {
        if message.with(|m| m.is_some()) {
            let mine = generation.with_value(|g| g + 1);
            generation.set_value(mine);
            set_timeout(
                move || {
                    if generation.get_value() == mine {
                        message.set(None);
                    }
                },
                Duration::from_millis(1800),
            );
        }
    });

    view! {
        <Show when=move || message.with(|m| m.is_some())>
            <div class="toast" role="status">
                {move || message.get().unwrap_or_default()}
            </div>
        </Show>
    }
}
