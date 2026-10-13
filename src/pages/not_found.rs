//! 404 Not Found page.

use leptos::prelude::*;
use ui_kit::components::{Button, ButtonVariant, Card};

/// 404 Not Found page.
#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div class="evm-page evm-not-found">
            <Card>
                <div class="evm-not-found-content">
                    <h1 class="evm-not-found-code">"404"</h1>
                    <h2 class="evm-not-found-title">"Page Not Found"</h2>
                    <p class="evm-not-found-message">
                        "The page you're looking for doesn't exist or has been moved."
                    </p>
                    <a href="/evm">
                        <Button variant=ButtonVariant::Primary>
                            "Go to Dashboard"
                        </Button>
                    </a>
                </div>
            </Card>
        </div>
    }
}
