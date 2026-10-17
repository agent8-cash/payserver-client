//! Preferences settings tab.
//!
//! Deliberately has no controls. Theme, currency, timezone and date format
//! used to be offered here, but each lived in a local signal that nothing
//! read and nothing saved, so choosing a value changed nothing and was
//! forgotten on reload. A control belongs on this tab only together with
//! something that stores the choice and something that reads it back.

use leptos::prelude::*;

/// Preferences tab.
#[component]
pub fn PreferencesTab() -> impl IntoView {
    view! {
        <div class="settings-tab-preferences">
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Preferences"</h3>
                </div>
                <div class="ps-card-body">
                    <p class="form-help">
                        "There are no display preferences to change yet."
                    </p>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// Every control on this tab must persist and be read somewhere. Until
    /// that exists the tab must offer none: an editable control with no
    /// storage behind it silently discards the choice.
    #[test]
    fn tab_offers_no_unpersisted_controls() {
        let src = include_str!("preferences.rs");
        let view_src = src.split("#[cfg(test)]").next().unwrap();
        for tag in ["<select", "<input", "<textarea", "<button"] {
            assert!(
                !view_src.contains(tag),
                "preferences tab renders {tag}; wire it to storage (and a reader) first"
            );
        }
    }
}
