use leptos::prelude::*;

/// A "− value +" control for whole numbers (servings). The value is owned by the
/// caller; `on_change` receives the new value on every tap.
#[component]
pub fn Stepper(
    #[prop(into)] value: Signal<u32>,
    #[prop(default = 1)] min: u32,
    #[prop(default = 99)] max: u32,
    /// Caption under the number ("servings").
    #[prop(into)]
    label: String,
    on_change: Callback<u32>,
) -> impl IntoView {
    let dec = move |_| {
        let v = value.get_untracked();
        if v > min {
            on_change.run(v - 1);
        }
    };
    let inc = move |_| {
        let v = value.get_untracked();
        if v < max {
            on_change.run(v + 1);
        }
    };
    let aria = label.clone();
    view! {
        <div class="stepper" role="group" aria-label=aria>
            <button type="button" aria-label="Fewer" on:click=dec>"−"</button>
            <span class="stepper__value">
                <b>{move || value.get()}</b>
                <span>{label}</span>
            </span>
            <button type="button" aria-label="More" on:click=inc>"+"</button>
        </div>
    }
}
