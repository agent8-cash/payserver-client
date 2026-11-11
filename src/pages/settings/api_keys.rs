//! API Keys settings tab.

use crate::api::{
    ApiClient, ApiKeyInfo, CreateApiKeyRequest, CreateApiKeyResponsePayload, RotateApiKeyResponse,
};
use leptos::prelude::*;

use super::{IconInfo, IconPlus};

/// A permission that can be granted to an API key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionOption {
    pub policy: &'static str,
    pub label: &'static str,
    pub category: &'static str,
}

/// Available permissions for API keys, organized by category.
fn get_available_permissions() -> Vec<PermissionOption> {
    vec![
        // Server permissions
        PermissionOption {
            policy: "ethpay.server.canviewserversettings",
            label: "View server settings",
            category: "Server",
        },
        PermissionOption {
            policy: "ethpay.server.canmodifyserversettings",
            label: "Modify server settings",
            category: "Server",
        },
        PermissionOption {
            policy: "ethpay.server.canmanagetokens",
            label: "Manage tokens",
            category: "Server",
        },
        PermissionOption {
            policy: "ethpay.server.canviewusers",
            label: "View users",
            category: "Server",
        },
        PermissionOption {
            policy: "ethpay.server.canmanageusers",
            label: "Manage users",
            category: "Server",
        },
        // Store permissions
        PermissionOption {
            policy: "ethpay.store.canviewstoresettings",
            label: "View store settings",
            category: "Store",
        },
        PermissionOption {
            policy: "ethpay.store.canmodifystoresettings",
            label: "Modify store settings",
            category: "Store",
        },
        PermissionOption {
            policy: "ethpay.store.canviewinvoices",
            label: "View invoices",
            category: "Store",
        },
        PermissionOption {
            policy: "ethpay.store.cancreateinvoice",
            label: "Create invoices",
            category: "Store",
        },
        PermissionOption {
            policy: "ethpay.store.canmodifyinvoices",
            label: "Modify invoices",
            category: "Store",
        },
        PermissionOption {
            policy: "ethpay.store.canviewpayments",
            label: "View payments",
            category: "Store",
        },
        PermissionOption {
            policy: "ethpay.store.canmanagewebhooks",
            label: "Manage webhooks",
            category: "Store",
        },
        // User permissions
        PermissionOption {
            policy: "ethpay.user.canviewprofile",
            label: "View profile",
            category: "User",
        },
        PermissionOption {
            policy: "ethpay.user.canmodifyprofile",
            label: "Modify profile",
            category: "User",
        },
        PermissionOption {
            policy: "ethpay.user.candeleteaccount",
            label: "Delete account",
            category: "User",
        },
        PermissionOption {
            policy: "ethpay.user.canmanagenotifications",
            label: "Manage notifications",
            category: "User",
        },
        // Special permission
        PermissionOption {
            policy: "unrestricted",
            label: "Unrestricted (full access)",
            category: "Special",
        },
    ]
}

/// Display a user-friendly name for a permission policy.
fn format_permission_name(policy: &str) -> String {
    get_available_permissions()
        .iter()
        .find(|p| p.policy == policy)
        .map(|p| p.label.to_string())
        .unwrap_or_else(|| policy.to_string())
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
    let (selected_permissions, set_selected_permissions) = signal(vec!["unrestricted".to_string()]);
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
        let perms = selected_permissions.get();
        let request = CreateApiKeyRequest {
            name: name.trim().to_string(),
            expires_at: None,
            permissions: perms,
        };
        set_loading.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(resp) = client.create_api_key(&request).await {
                set_created_key.set(Some(resp));
                set_show_create.set(false);
                set_new_key_name.set(String::new());
                set_selected_permissions.set(vec!["unrestricted".to_string()]);
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
            {move || show_create.get().then(|| {
                let all_permissions = get_available_permissions();
                let mut grouped_permissions: std::collections::HashMap<&'static str, Vec<PermissionOption>> = std::collections::HashMap::new();
                for perm in all_permissions {
                    grouped_permissions.entry(perm.category).or_insert(Vec::new()).push(perm);
                }
                let mut categories: Vec<_> = grouped_permissions.keys().copied().collect();
                categories.sort();

                view! {
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
                                <p class="form-help" style="margin-bottom: 12px;">"Select which permissions this key should have. Leave none selected for a key that can authenticate but cannot perform any actions."</p>

                                {categories.into_iter().map(|category| {
                                    let perms = grouped_permissions.get(category).cloned().unwrap_or_default();
                                    view! {
                                        <div class="permission-category" style="margin-bottom: 12px;">
                                            <h4 class="permission-category-title" style="font-size: 0.875rem; font-weight: 600; margin-bottom: 8px; text-transform: uppercase; opacity: 0.7;">{category}</h4>
                                            <div class="permission-checkboxes" style="display: flex; flex-direction: column; gap: 6px;">
                                                {perms.into_iter().map(|perm| {
                                                    let policy = perm.policy.to_string();
                                                    let policy_for_event = perm.policy.to_string();
                                                    let is_checked = move || selected_permissions.get().contains(&policy_for_event);
                                                    view! {
                                                        <label class="permission-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                                                            <input
                                                                type="checkbox"
                                                                checked=is_checked
                                                                on:change=move |_| toggle_permission(&policy)
                                                            />
                                                            <span>{perm.label}</span>
                                                        </label>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        </div>
                                    }
                                }).collect_view()}
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
                }
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
                                                        {key.permissions.as_ref().map(|perms| {
                                                            if perms.is_empty() {
                                                                view! {
                                                                    <div class="api-key-permissions" style="margin-top: 8px; font-size: 0.875rem; color: var(--text-secondary);">
                                                                        "Permissions: None (authentication only)"
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="api-key-permissions" style="margin-top: 8px;">
                                                                        <div style="font-size: 0.875rem; font-weight: 500; margin-bottom: 4px;">"Permissions:"</div>
                                                                        <div style="display: flex; flex-wrap: wrap; gap: 6px;">
                                                                            {perms.iter().map(|perm| {
                                                                                view! {
                                                                                    <span class="permission-badge" style="display: inline-block; padding: 2px 8px; background: var(--color-bg-secondary); border-radius: 4px; font-size: 0.75rem; white-space: nowrap;">
                                                                                        {format_permission_name(perm)}
                                                                                    </span>
                                                                                }
                                                                            }).collect_view()}
                                                                        </div>
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                        })}
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
