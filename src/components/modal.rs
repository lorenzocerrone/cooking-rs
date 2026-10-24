use leptos::ev;
use leptos::prelude::*;

/// A generic dialog: a bottom sheet on narrow screens, a centered dialog on wide
/// ones (CSS decides). Visibility is driven by the `open` signal; it closes on
/// backdrop click, the close button, or the Escape key. Purely presentational.
#[component]
pub fn Modal(
    open: RwSignal<bool>,
    /// Optional heading shown at the top of the sheet.
    #[prop(optional, into)]
    title: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
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
                    <div class="modal-head">
                        <Show when=move || !title.get().is_empty()>
                            <h2 class="modal-title">{move || title.get()}</h2>
                        </Show>
                        <button
                            class="modal-close"
                            type="button"
                            aria-label="Close"
                            on:click=move |_| open.set(false)
                        >
                            "×"
                        </button>
                    </div>
                    {children()}
                </div>
            </div>
        </Show>
    }
}
