//! API Keys settings tab.

use crate::api::{
    ApiClient, ApiKeyInfo, CreateApiKeyRequest, CreateApiKeyResponsePayload, RotateApiKeyResponse,
};
use leptos::prelude::*;

use super::{IconInfo, IconPlus};

/// Store-scoped permissions a key can be granted, as (policy, label).
///
/// Full access is a separate toggle, not an entry here, so a scoped list and
/// the full-access sentinel can never be sent together.
const STORE_PERMISSIONS: [(&str, &str); 7] = [
    ("ethpay.store.canviewstoresettings", "View store settings"),
    (
        "ethpay.store.canmodifystoresettings",
        "Modify store settings",
    ),
    ("ethpay.store.canviewinvoices", "View invoices"),
    ("ethpay.store.cancreateinvoice", "Create invoices"),
    ("ethpay.store.canmodifyinvoices", "Modify invoices"),
    ("ethpay.store.canviewpayments", "View payments"),
    ("ethpay.store.canmanagewebhooks", "Manage webhooks"),
];

/// Wire sentinel for the owner's full role.
const UNRESTRICTED: &str = "unrestricted";

/// Display name for a permission policy, or the raw string when unknown.
fn format_permission_name(policy: &str) -> String {
    STORE_PERMISSIONS
        .iter()
        .find(|(p, _)| *p == policy)
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| policy.to_string())
}

/// A new key starts with nothing granted. Widening is always a deliberate act.
fn default_selected_permissions() -> Vec<String> {
    Vec::new()
}

/// The `permissions` list a create request sends. Full access overrides the
/// per-action picks, so the two cannot be combined on the wire.
fn request_permissions(full_access: bool, selected: &[String]) -> Vec<String> {
    if full_access {
        vec![UNRESTRICTED.to_string()]
    } else {
        selected.to_vec()
    }
}

/// What a key's permissions column shows. `None` is a legacy key that inherits
/// the owner's role in full, so it reads the same as `["unrestricted"]`.
fn key_permissions_summary(permissions: Option<&[String]>) -> AnyView {
    let full_access = || {
        view! {
            <div class="api-key-permissions">
                <span class="permission-badge">"Full access"</span>
            </div>
        }
        .into_any()
    };
    match permissions {
        None => full_access(),
        Some(perms) if perms.iter().any(|p| p == UNRESTRICTED) => full_access(),
        Some([]) => view! {
            <div class="api-key-permissions">"No access beyond login"</div>
        }
        .into_any(),
        Some(perms) => view! {
            <div class="api-key-permissions">
                <span class="api-key-permissions-label">"Permissions:"</span>
                {perms
                    .iter()
                    .map(|perm| {
                        let name = format_permission_name(perm);
                        view! { <span class="permission-badge">{name}</span> }
                    })
                    .collect_view()}
            </div>
        }
        .into_any(),
    }
}

/// API Keys tab.
#[component]
pub fn ApiKeysTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    // Track version to trigger refetches after create/revoke
    let (version, set_version) = signal(0u32);

    // Load API keys from backend
    let keys_resource = LocalResource::new(move || {
        let client = api.get();
        let _v = version.get();
        async move { client.list_api_keys().await.ok() }
    });

    // State for create form
    let (show_create, set_show_create) = signal(false);
    let (new_key_name, set_new_key_name) = signal(String::new());
    let (full_access, set_full_access) = signal(false);
    let (selected_permissions, set_selected_permissions) = signal(default_selected_permissions());
    let (created_key, set_created_key) = signal(Option::<CreateApiKeyResponsePayload>::None);
    let (loading, set_loading) = signal(false);

    // Handler for toggling a permission checkbox
    let toggle_permission = move |policy: &str| {
        let mut perms = selected_permissions.get();
        if let Some(pos) = perms.iter().position(|p| p == policy) {
            perms.remove(pos);
        } else {
            perms.push(policy.to_string());
        }
        set_selected_permissions.set(perms);
    };

    // Create handler
    let on_create = move |_| {
        let name = new_key_name.get();
        if name.trim().is_empty() {
            return;
        }
        let client = api.get();
        let request = CreateApiKeyRequest {
            name: name.trim().to_string(),
            expires_at: None,
            permissions: request_permissions(full_access.get(), &selected_permissions.get()),
        };
        set_loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(resp) = client.create_api_key(&request).await {
                set_created_key.set(Some(resp));
                set_show_create.set(false);
                set_new_key_name.set(String::new());
                set_full_access.set(false);
                set_selected_permissions.set(default_selected_permissions());
                set_version.update(|v| *v += 1);
            }
            set_loading.set(false);
        });
    };

    // Revoke handler factory
    let make_revoke_handler = move |key_id: String| {
        let client = api.get();
        move |_| {
            let client = client.clone();
            let id = key_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = client.revoke_api_key(&id).await;
                set_version.update(|v| *v += 1);
            });
        }
    };

    // State for rotated key display
    let (rotated_key, set_rotated_key) = signal(Option::<RotateApiKeyResponse>::None);
    // Error surfaced on a failed rotation — previously the handler swallowed
    // errors silently, leaving the user wondering if the click had any effect.
    let (rotate_error, set_rotate_error) = signal(Option::<String>::None);

    // Rotate handler factory
    let make_rotate_handler = move |key_id: String| {
        let client = api.get();
        move |_| {
            let client = client.clone();
            let id = key_id.clone();
            set_rotate_error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                match client.rotate_api_key(&id).await {
                    Ok(resp) => {
                        set_rotated_key.set(Some(resp));
                        set_version.update(|v| *v += 1);
                    }
                    Err(err) => {
                        set_rotate_error.set(Some(format!("Failed to rotate key: {err}")));
                    }
                }
            });
        }
    };

    view! {
        <div class="settings-tab-api-keys">
            <div class="section-header">
                <div>
                    <h3 class="section-title">"API Keys"</h3>
                    <p class="section-desc">"Manage API keys for programmatic access"</p>
                </div>
                <button
                    class="ps-btn ps-btn-primary ps-btn-sm"
                    on:click=move |_| set_show_create.set(true)
                >
                    <IconPlus />
                    "Create API key"
                </button>
            </div>

            // Create form
            {move || show_create.get().then(|| view! {
                <div class="ps-card" style="margin-bottom: 16px;">
                    <div class="ps-card-header">
                        <h3>"Create new API key"</h3>
                    </div>
                    <div class="ps-card-body">
                        <div class="form-group">
                            <label class="form-label">"Key name"</label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="e.g., Production Key"
                                prop:value=move || new_key_name.get()
                                on:input=move |ev| set_new_key_name.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="form-group">
                            <label class="form-label">"Permissions"</label>
                            <p class="form-help">"Leave everything off for a key that can authenticate but cannot perform any actions."</p>
                            <label class="permission-checkbox">
                                <input
                                    type="checkbox"
                                    prop:checked=move || full_access.get()
                                    on:change=move |ev| set_full_access.set(event_target_checked(&ev))
                                />
                                <span>"Full access (unrestricted)"</span>
                            </label>
                            <div class="permission-checkboxes">
                                {STORE_PERMISSIONS.iter().map(|&(policy, label)| {
                                    let checked = move || selected_permissions.get().iter().any(|p| p == policy);
                                    view! {
                                        <label class="permission-checkbox">
                                            <input
                                                type="checkbox"
                                                checked=checked
                                                prop:disabled=move || full_access.get()
                                                on:change=move |_| toggle_permission(policy)
                                            />
                                            <span>{label}</span>
                                        </label>
                                    }
                                }).collect_view()}
                            </div>
                        </div>

                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                prop:disabled=move || loading.get()
                                on:click=on_create
                            >
                                {move || if loading.get() { "Creating..." } else { "Create key" }}
                            </button>
                            <button
                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                on:click=move |_| set_show_create.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </div>
            })}

            // Show newly created key (plaintext shown once)
            {move || created_key.get().map(|key| view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-success);">
                    <div class="ps-card-body">
                        <p><strong>"Your new API key has been created. Copy it now — it will not be shown again."</strong></p>
                        <code class="api-key-value" style="display: block; margin: 8px 0; padding: 8px; background: var(--color-bg-secondary); word-break: break-all;">
                            {key.key.clone()}
                        </code>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_created_key.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            })}

            // Rotation error — user must know when a rotate click failed.
            {move || rotate_error.get().map(|msg| view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-danger);">
                    <div class="ps-card-body">
                        <p><strong>{msg}</strong></p>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_rotate_error.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            })}

            // Show rotated key (plaintext shown once)
            {move || rotated_key.get().map(|key| {
                // Always sent by the server, so there is no "unknown" branch to
                // render - the client used to model this as optional and carry
                // a fallback that could never fire.
                let grace_line = format!(
                    "The old key remains valid until {}.",
                    key.old_key_grace_expires_at.to_rfc3339()
                );
                view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-warning);">
                    <div class="ps-card-body">
                        <p><strong>"Key rotated successfully. Copy your new key now — it will not be shown again."</strong></p>
                        <p>{grace_line}</p>
                        <code class="api-key-value" style="display: block; margin: 8px 0; padding: 8px; background: var(--color-bg-secondary); word-break: break-all;">
                            {key.key.clone()}
                        </code>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_rotated_key.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            }})}

            // API keys list
            <Suspense fallback=move || view! { <p>"Loading API keys..."</p> }>
                {move || Suspend::new(async move {
                    match keys_resource.await {
                        Some(resp) => {
                            let keys = resp.keys;
                            if keys.is_empty() {
                                view! { <p class="empty-state">"No API keys yet. Create one to get started."</p> }.into_any()
                            } else {
                                view! {
                                    <div class="api-keys-list">
                                        {keys.into_iter().map(|key: ApiKeyInfo| {
                                            let (status_class, status_label) = if !key.is_active {
                                                ("badge badge-neutral".to_string(), "Revoked".to_string())
                                            } else if key.deprecated_at.is_some() {
                                                // Surface the actual expiry rather than a
                                                // static "Deprecated" — users need to know
                                                // when the grace window ends.
                                                let label = match key.deprecation_expires_at.map(|d| d.to_rfc3339()).as_deref() {
                                                    Some(exp) => format!("Deprecated — expires {exp}"),
                                                    None => "Deprecated".to_string(),
                                                };
                                                ("badge badge-warning".to_string(), label)
                                            } else {
                                                ("badge badge-success".to_string(), "Active".to_string())
                                            };
                                            let is_active = key.is_active;
                                            let is_deprecated = key.deprecated_at.is_some();
                                            let revoke_handler = make_revoke_handler(key.id.to_string());
                                            let rotate_handler = make_rotate_handler(key.id.to_string());

                                            view! {
                                                <div class="api-key-item">
                                                    <div class="api-key-info">
                                                        <div class="api-key-header">
                                                            <span class="api-key-name">{key.name}</span>
                                                            <span class=status_class>{status_label}</span>
                                                        </div>
                                                        <code class="api-key-value">{key.key_prefix}</code>
                                                        <span class="api-key-created">"Created "{key.created_at.to_rfc3339()}</span>
                                                        {key_permissions_summary(key.permissions.as_deref())}
                                                    </div>
                                                    <div class="api-key-actions">
                                                        {(is_active && !is_deprecated).then(|| view! {
                                                            <button
                                                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                on:click=rotate_handler
                                                            >
                                                                "Rotate"
                                                            </button>
                                                        })}
                                                        {is_active.then(|| view! {
                                                            <button
                                                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                on:click=revoke_handler
                                                            >
                                                                "Revoke"
                                                            </button>
                                                        })}
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }
                        None => view! { <p class="error">"Failed to load API keys"</p> }.into_any(),
                    }
                })}
            </Suspense>

            <div class="settings-info">
                <IconInfo />
                <div>
                    <p><strong>"Keep your API keys secure"</strong></p>
                    <p>"Never share your API keys publicly or commit them to version control. Use environment variables in production."</p>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Policy strings the server accepts on an API key, as its policy list
    /// defines them. Mirrored here because this crate does not depend on the
    /// server's auth crate.
    const ACCEPTED_POLICIES: &[&str] = &[
        "ethpay.server.canviewserversettings",
        "ethpay.server.canmodifyserversettings",
        "ethpay.server.canmanagetokens",
        "ethpay.server.canviewusers",
        "ethpay.server.canmanageusers",
        "ethpay.store.canviewstoresettings",
        "ethpay.store.canmodifystoresettings",
        "ethpay.store.canviewinvoices",
        "ethpay.store.cancreateinvoice",
        "ethpay.store.canmodifyinvoices",
        "ethpay.store.canviewpayments",
        "ethpay.store.canmanagewebhooks",
        "ethpay.user.canviewprofile",
        "ethpay.user.canmodifyprofile",
        "ethpay.user.candeleteaccount",
        "ethpay.user.canmanagenotifications",
        "unrestricted",
    ];

    /// The stylesheet and this file, read at compile time.
    const STYLES: &str = include_str!("../../../styles.css");
    const SOURCE: &str = include_str!("api_keys.rs");

    #[test]
    fn default_selection_grants_nothing() {
        assert!(default_selected_permissions().is_empty());
    }

    #[test]
    fn every_offered_policy_is_one_the_server_accepts() {
        for (policy, _) in STORE_PERMISSIONS {
            assert!(
                ACCEPTED_POLICIES.contains(&policy),
                "offered policy {policy} is not in the accepted set"
            );
        }
    }

    #[test]
    fn full_access_overrides_picked_actions() {
        let picked = vec!["ethpay.store.canviewinvoices".to_string()];
        assert_eq!(request_permissions(true, &picked), vec![UNRESTRICTED]);
        assert_eq!(request_permissions(false, &picked), picked);
        assert!(request_permissions(false, &[]).is_empty());
    }

    #[test]
    fn format_permission_name_maps_known_and_passes_through_unknown() {
        assert_eq!(
            format_permission_name("ethpay.store.canviewinvoices"),
            "View invoices"
        );
        assert_eq!(
            format_permission_name("ethpay.store.somethingnew"),
            "ethpay.store.somethingnew"
        );
    }

    /// `/* ... */` removed, so a class named in prose is not taken as styled.
    fn strip_comments(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(open) = rest.find("/*") {
            out.push_str(&rest[..open]);
            match rest[open + 2..].find("*/") {
                Some(close) => rest = &rest[open + 2 + close + 2..],
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    /// Whether the stylesheet has a rule whose selector names `class` as a
    /// whole token.
    fn is_defined(class: &str) -> bool {
        let styles = strip_comments(STYLES);
        let needle = format!(".{class}");
        let mut from = 0;
        while let Some(at) = styles[from..].find(&needle) {
            let start = from + at;
            let after = styles[start + needle.len()..].chars().next();
            let before = styles[..start].chars().next_back();
            let boundary_before =
                before.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            let boundary_after =
                after.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            if boundary_before && boundary_after {
                return true;
            }
            from = start + 1;
        }
        false
    }

    /// Every literal permission class in this renderer's markup must have a
    /// rule. Reads the source above the test module so the assertions do not
    /// count. Classes outside this feature are covered by the repo-wide
    /// `check-classes-styled.py` gate, not repeated here.
    #[test]
    fn every_class_this_renderer_emits_is_defined_in_the_stylesheet() {
        let code = SOURCE.split("#[cfg(test)]").next().unwrap_or(SOURCE);
        let mut missing = Vec::new();
        let mut rest = code;
        while let Some(at) = rest.find("class=\"") {
            rest = &rest[at + 7..];
            let Some(end) = rest.find('"') else { break };
            let (value, tail) = rest.split_at(end);
            rest = tail;
            for class in value.split_whitespace() {
                let ours =
                    class.starts_with("permission") || class.starts_with("api-key-permissions");
                if ours && !is_defined(class) {
                    missing.push(class.to_string());
                }
            }
        }
        assert!(missing.is_empty(), "no rule in styles.css for: {missing:?}");
    }
}
