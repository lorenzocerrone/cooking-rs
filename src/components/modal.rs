use leptos::ev;
use leptos::prelude::*;

/// A generic modal dialog. Visibility is driven by the `open` signal; it closes on
/// backdrop click, the close button, or the Escape key. Purely presentational — it
/// knows nothing about what it contains.
#[component]
pub fn Modal(open: RwSignal<bool>, children: ChildrenFn) -> impl IntoView {
    // Close on Escape (cleaned up when the component is dropped).
    let handle = window_event_listener(ev::keydown, move |e| {
        if e.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || handle.remove());

    view! {
        <Show when=move || open.get() fallback=|| ()>
            <div class="modal-overlay" on:click=move |_| open.set(false)>
                <div
                    class="modal-dialog"
                    role="dialog"
                    aria-modal="true"
                    on:click=|e| e.stop_propagation()
                >
                    <button
                        class="modal-close"
                        type="button"
                        aria-label="Close"
                        on:click=move |_| open.set(false)
                    >
                        "×"
                    </button>
                    {children()}
                </div>
            </div>
        </Show>
    }
}
