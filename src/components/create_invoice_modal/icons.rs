//! Inline SVG icons used by the create-invoice modal.

use leptos::prelude::*;

#[component]
pub(super) fn IconClose() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
    }
}

#[component]
pub(super) fn IconChevron(expanded: ReadSignal<bool>) -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            style=move || if expanded.get() { "transform: rotate(180deg); transition: transform 0.15s" } else { "transition: transform 0.15s" }
        >
            <polyline points="6 9 12 15 18 9"></polyline>
        </svg>
    }
}
