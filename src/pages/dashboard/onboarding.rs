use super::icons::{IconArrowRight, IconCheck};
use crate::api::ApiClient;
use crate::app::{StoreContext, StoresStatus};
use leptos::prelude::*;
use leptos_router::components::A;

/// The three steps a merchant needs before an invoice can be paid: a store, a
/// wallet, and the invoice itself. Gone once all three are done - a merchant
/// who has already set up does not need reminding forever.
///
/// A store with no wallet cannot receive payments - the server refuses to
/// create an invoice for one - so this is not just a tour, it is the path to
/// clearing that gate.
///
/// A fourth, optional row offers a contact email. It never gates the three
/// required steps: see `checklist_shown` and `email_row_shown`.
#[component]
pub(super) fn OnboardingChecklist() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let store_ctx = use_context::<StoreContext>().expect("StoreContext must be provided");
    let stores = store_ctx.stores;
    let stores_status = store_ctx.stores_status;

    // Per-device: a merchant who chose "Not now" should not see the row again
    // in this browser. The server holds no record of it, and none is needed.
    let skipped = RwSignal::new(email_row_skipped());

    // Wallets are account-level and invoice counts aren't in `StoreContext`,
    // so this fetches its own copy rather than widening a context every
    // dashboard visitor pays for. Waits for `stores_status` so a checklist
    // that briefly has zero of everything doesn't flash "all done" before the
    // real store count arrives.
    let progress = LocalResource::new(move || {
        let client = api.get();
        let status = stores_status.get();
        let has_store = !stores.get().is_empty();
        async move {
            if !matches!(status, StoresStatus::Loaded) {
                return None;
            }
            let has_wallet = client
                .list_wallets()
                .await
                .map(|w| !w.is_empty())
                .unwrap_or(false);
            let has_invoice = client
                .get_dashboard_stats()
                .await
                .map(|s| s.total_invoices > 0)
                .unwrap_or(false);
            // A failed lookup reads as "no email row": an optional nudge must
            // not appear on bad data. `has_email` defaults to true so a failed
            // profile fetch hides the row rather than offering a set-email
            // flow for an address that may already exist.
            let has_email = client
                .get_me()
                .await
                .map(|u| u.email.is_some())
                .unwrap_or(true);
            let email_configured = client
                .get_email_status()
                .await
                .map(|s| s.configured)
                .unwrap_or(false);
            Some(Progress {
                has_store,
                has_wallet,
                has_invoice,
                has_email,
                email_configured,
            })
        }
    });

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let skipped_now = skipped.get();
                progress.get().and_then(|w| (*w).clone()).map(|p| {
                    let email_row = email_row_shown(p.email_configured, p.has_email, skipped_now);
                    let on_skip = Callback::new(move |_| {
                        remember_email_row_skipped();
                        skipped.set(true);
                    });
                    view! {
                        <ChecklistBox
                            has_store=p.has_store
                            has_wallet=p.has_wallet
                            has_invoice=p.has_invoice
                            email_row=email_row
                            on_skip=on_skip
                        />
                    }
                })
            }}
        </Suspense>
    }
}

/// What the progress fetch learned about the account.
#[derive(Clone)]
struct Progress {
    has_store: bool,
    has_wallet: bool,
    has_invoice: bool,
    has_email: bool,
    email_configured: bool,
}

/// Whether the checklist box renders. The three required steps decide whether
/// work is left; the optional email row only keeps the box up while it is
/// offered, and never counts toward completion.
fn checklist_shown(has_store: bool, has_wallet: bool, has_invoice: bool, email_row: bool) -> bool {
    !(has_store && has_wallet && has_invoice) || email_row
}

/// Whether the optional email row is offered. Only where the server can send
/// mail (otherwise the row would promise a verification email that never
/// arrives), only while the account has no address, and only until this
/// device has skipped it.
fn email_row_shown(configured: bool, has_email: bool, skipped: bool) -> bool {
    configured && !has_email && !skipped
}

const EMAIL_ROW_SKIPPED_KEY: &str = "ps-onboarding-email-skipped";

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Storage can be absent or blocked (private windows, cleared site data). Then
/// the row is simply shown again, which is the safe direction to fail.
fn email_row_skipped() -> bool {
    local_storage()
        .and_then(|s| s.get_item(EMAIL_ROW_SKIPPED_KEY).ok().flatten())
        .is_some()
}

fn remember_email_row_skipped() {
    if let Some(s) = local_storage() {
        let _ = s.set_item(EMAIL_ROW_SKIPPED_KEY, "1");
    }
}

/// The checklist box itself. Takes plain values so it can be rendered without
/// the app's contexts.
#[component]
fn ChecklistBox(
    has_store: bool,
    has_wallet: bool,
    has_invoice: bool,
    email_row: bool,
    on_skip: Callback<()>,
) -> impl IntoView {
    if !checklist_shown(has_store, has_wallet, has_invoice, email_row) {
        return ().into_any();
    }
    view! {
        <div class="onboarding-checklist">
            <h3 class="onboarding-checklist-title">
                "Get set up to accept payments"
            </h3>
            <ul class="onboarding-checklist-items">
                <OnboardingStep
                    done=has_store
                    label="Create a store"
                    href="/evm/stores"
                />
                <OnboardingStep
                    done=has_wallet
                    label="Add a wallet"
                    href="/evm/wallets"
                />
                <OnboardingStep
                    done=has_invoice
                    label="Create your first invoice"
                    href="/evm/invoices"
                />
                {email_row.then(|| view! { <OptionalEmailStep on_skip=on_skip /> })}
            </ul>
        </div>
    }
    .into_any()
}

/// One row of the onboarding checklist: a status mark, a label, and - while
/// not yet done - a link to where the step happens.
#[component]
fn OnboardingStep(done: bool, label: &'static str, href: &'static str) -> impl IntoView {
    let item_class = if done {
        "onboarding-step onboarding-step-done"
    } else {
        "onboarding-step"
    };
    view! {
        <li class=item_class>
            <span class="onboarding-step-mark">
                {if done { view! { <IconCheck /> }.into_any() } else { ().into_any() }}
            </span>
            <span class="onboarding-step-label">{label}</span>
            {(!done)
                .then(|| view! {
                    // The arrow carries no accessible name of its own, so the
                    // step's label becomes the link's: a screen reader hears
                    // "Create a store, link", not "link". The icon is marked
                    // aria-hidden for the same reason - it would otherwise be
                    // announced as a second, nameless child.
                    <A href=href attr:class="onboarding-step-action" attr:aria-label=label>
                        <IconArrowRight />
                    </A>
                })}
        </li>
    }
}

/// The optional contact-email row. Styled apart from the required steps - a
/// dashed mark and an "Optional" tag - so it reads as a recommendation, not as
/// unfinished setup. Recovery is seed-based, so the row says plainly that
/// nothing depends on the address.
#[component]
fn OptionalEmailStep(on_skip: Callback<()>) -> impl IntoView {
    view! {
        <li class="onboarding-step onboarding-step-optional">
            <span class="onboarding-step-mark"></span>
            <span class="onboarding-step-label">"Add a contact email"</span>
            <span class="onboarding-step-tag">"Optional"</span>
            <span class="onboarding-step-hint">
                "For receipts and billing notices. Account recovery does not use it."
            </span>
            <A href="/settings/account" attr:class="onboarding-step-action" attr:aria-label="Add a contact email">
                <IconArrowRight />
            </A>
            <button
                type="button"
                class="onboarding-step-skip"
                on:click=move |_| on_skip.run(())
            >
                "Not now"
            </button>
        </li>
    }
}

#[cfg(test)]
mod tests {
    use super::ChecklistBox;
    use leptos::prelude::*;

    // The optional email row must never gate the three required steps: these
    // pin the decision that keeps the checklist's completion test intact.

    #[test]
    fn email_row_is_offered_only_where_mail_can_be_sent() {
        assert!(super::email_row_shown(true, false, false));
        assert!(!super::email_row_shown(false, false, false));
    }

    #[test]
    fn email_row_is_hidden_once_an_address_exists_or_is_skipped() {
        assert!(!super::email_row_shown(true, true, false));
        assert!(!super::email_row_shown(true, false, true));
    }

    #[test]
    fn checklist_with_required_steps_done_is_not_held_open_by_email() {
        // All three required steps done and no email row: nothing left to show.
        assert!(!super::checklist_shown(true, true, true, false));
        // With the email row offered, the box stays up on the same done steps.
        assert!(super::checklist_shown(true, true, true, true));
        // Required work remaining always shows the box.
        assert!(super::checklist_shown(false, true, true, false));
        assert!(super::checklist_shown(true, false, true, false));
        assert!(super::checklist_shown(true, true, false, false));
    }

    #[test]
    fn checklist_renders_nothing_when_done_and_no_email_row() {
        let html = view! {
            <ChecklistBox
                has_store=true
                has_wallet=true
                has_invoice=true
                email_row=false
                on_skip=Callback::new(|_| {})
            />
        }
        .to_html();
        assert!(!html.contains("onboarding-checklist"));
    }
}
