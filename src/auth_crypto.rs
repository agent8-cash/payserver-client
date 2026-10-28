//! Client-side crypto for the recovery-phrase flows: redeeming a lost
//! account (`pages/recovery.rs`) and proving continued possession of the
//! phrase before adding a device (`pages/settings/security.rs`).
//!
//! Every function here is pure and runs on the host target under `cargo
//! test`; only the WebAuthn ceremony and the network calls around these need
//! wasm. Built on `crypto::mnemonic`/`crypto::symmetric`/`crypto::recovery_salt`,
//! the same primitives `payserver-commons`'s own registration flow uses,
//! rather than anything invented here, so a derivation can never disagree
//! with what the server was wrapped under.
//!
//! Adding a device does **not** unwrap or re-wrap anything: the account's
//! `encrypted_symmetric_key` is already valid for every device of the
//! account (the server copies it byte-for-byte into a lazily-created device
//! row today - `auth::service::device::device_for_new_login`), so
//! `/auth/recovery/start`'s wrapped key is relayed to the new device as-is,
//! once proof of the phrase unlocks it. Only recovery replaces the wrap,
//! because only recovery replaces the phrase it is wrapped under.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ui_kit::auth::types::{EncryptedBlob as WireBlob, KdfParams as WireKdfParams};

/// The identifier the recovery KDF is salted with - matches whichever of
/// email, `wallet:{addr}` or `passkey:{user_id}` the account registered (or
/// last recovered) under. Mirrors `crypto::SaltIdentity` exactly rather than
/// redefining the rule, since a divergence fails silently as a wrong-phrase
/// error (see `crypto::recovery_salt`'s module doc).
pub fn identifier(email: Option<&str>, wallet_address: Option<&str>, user_id: &str) -> String {
    if let Some(email) = email {
        crypto::SaltIdentity::Email(email.to_string()).as_identifier()
    } else if let Some(address) = wallet_address {
        crypto::SaltIdentity::Wallet(address.to_string()).as_identifier()
    } else {
        crypto::SaltIdentity::Passkey(user_id.to_string()).as_identifier()
    }
}

/// Encode a `crypto::EncryptedBlob` (raw bytes) as the base64-string shape
/// the wire types / the server's JSONB columns use.
fn to_wire_blob(blob: &crypto::EncryptedBlob) -> WireBlob {
    WireBlob {
        ciphertext: B64.encode(&blob.ciphertext),
        iv: B64.encode(&blob.iv),
        mac: B64.encode(&blob.mac),
    }
}

/// Decode the wire shape back to raw bytes. Fails on malformed base64 rather
/// than panicking - this runs on whatever a server or a pasted phrase hands
/// back, never on input this crate controls end to end.
fn from_wire_blob(blob: &WireBlob) -> Result<crypto::EncryptedBlob, String> {
    Ok(crypto::EncryptedBlob::new(
        B64.decode(&blob.ciphertext)
            .map_err(|e| format!("ciphertext is not valid base64: {e}"))?,
        B64.decode(&blob.iv)
            .map_err(|e| format!("iv is not valid base64: {e}"))?,
        B64.decode(&blob.mac)
            .map_err(|e| format!("mac is not valid base64: {e}"))?,
    ))
}

/// The `KdfParams` for a *fresh* recovery wrap under `identifier`: the fixed
/// `crypto::RECOVERY_*` cost, salted from the identifier alone (not random),
/// so a later recovery attempt can rebuild the same salt from nothing but
/// the phrase and the identifier - there is nothing else for the user to
/// have kept.
fn fresh_kdf_params(identifier: &str) -> WireKdfParams {
    WireKdfParams {
        algorithm: "argon2id".to_string(),
        memory_kb: crypto::RECOVERY_MEMORY_KB,
        iterations: crypto::RECOVERY_ITERATIONS,
        parallelism: crypto::RECOVERY_PARALLELISM,
        salt: B64.encode(crypto::recovery_salt_for(identifier).as_bytes()),
    }
}

/// Classify what a recovery-page visitor typed - email, a `0x`-prefixed
/// 42-character wallet address, or (for a passkey-only account with
/// neither) the account id - matching the server's own
/// `resolve_recovery_user` detection exactly, so the two sides can never
/// resolve to different salts for the same account. Returns the salt
/// identifier used to derive and verify the recovery key; the raw `input`
/// is what the server expects verbatim as `StartRecoveryRequest::identifier`,
/// since it does its own normalization (lowercasing email, checksumming a
/// wallet address) for the account lookup itself.
pub fn salt_identifier_for_recovery_input(input: &str) -> String {
    let lower = input.trim().to_lowercase();
    if lower.contains('@') {
        crypto::SaltIdentity::Email(lower).as_identifier()
    } else if lower.starts_with("0x") && lower.len() == 42 {
        crypto::SaltIdentity::Wallet(input.trim().to_string()).as_identifier()
    } else {
        crypto::SaltIdentity::Passkey(lower).as_identifier()
    }
}

/// Generate a real 24-word BIP-39 recovery phrase. 24 words / 256 bits of
/// entropy, matching the recovery derivation's expectations.
pub fn generate_phrase() -> Result<Vec<String>, String> {
    crypto::RecoveryMnemonic::generate()
        .map(|m| m.words().into_iter().map(str::to_string).collect())
        .map_err(|e| format!("could not generate a recovery phrase: {e}"))
}

/// `base64(SHA-256(Argon2id(phrase, identifier)))` - what `/auth/recovery/start`
/// compares against the account's stored `recovery_verification_hash`, proving
/// possession of the phrase without ever sending it.
pub fn verification_hash(phrase: &str, identifier: &str) -> Result<String, String> {
    let mnemonic = crypto::RecoveryMnemonic::from_phrase(phrase)
        .map_err(|e| format!("not a valid recovery phrase: {e}"))?;
    let recovery_key = mnemonic
        .derive_recovery_key(identifier)
        .map_err(|e| format!("could not derive the recovery key: {e}"))?;
    Ok(crypto::recovery_verification_hash(&recovery_key))
}

/// Unwrap the account's symmetric key from `blob`, given the phrase that was
/// used to wrap it and the identifier it is salted with. This is the only
/// way this account's data key is ever recovered - there is no session key
/// material that can do it (see the module doc on why add-device does not
/// need this at all).
pub fn unwrap_account_key(
    phrase: &str,
    identifier: &str,
    blob: &WireBlob,
) -> Result<crypto::SymmetricKey, String> {
    let mnemonic = crypto::RecoveryMnemonic::from_phrase(phrase)
        .map_err(|e| format!("not a valid recovery phrase: {e}"))?;
    let recovery_key = mnemonic
        .derive_recovery_key(identifier)
        .map_err(|e| format!("could not derive the recovery key: {e}"))?;
    let stretched = crypto::stretch_master_key(&recovery_key)
        .map_err(|e| format!("could not stretch the recovery key: {e}"))?;
    let raw = from_wire_blob(blob)?;
    crypto::symmetric::decrypt_key(&raw, &stretched)
        .map_err(|_| "the phrase does not unwrap this account's key".to_string())
}

/// Re-wrap `account_key` under a freshly generated phrase, for
/// `POST /auth/recovery/complete`. Returns the new phrase (to show the user
/// exactly once - this is their only copy), the new `kdf_params`, the new
/// wrapped blob, and the new verification hash.
///
/// `identifier` does not change: recovery replaces the phrase, never the
/// handle (email / wallet / account id) the account is salted under.
pub fn rewrap_under_new_phrase(
    account_key: &crypto::SymmetricKey,
    identifier: &str,
) -> Result<(Vec<String>, WireKdfParams, WireBlob, String), String> {
    let words = generate_phrase()?;
    let phrase = words.join(" ");
    let mnemonic = crypto::RecoveryMnemonic::from_phrase(&phrase)
        .map_err(|e| format!("generated phrase failed to re-parse: {e}"))?;
    let recovery_key = mnemonic
        .derive_recovery_key(identifier)
        .map_err(|e| format!("could not derive the recovery key: {e}"))?;
    let hash = crypto::recovery_verification_hash(&recovery_key);
    let stretched = crypto::stretch_master_key(&recovery_key)
        .map_err(|e| format!("could not stretch the recovery key: {e}"))?;
    let wrapped = crypto::symmetric::encrypt_key(account_key, &stretched)
        .map_err(|e| format!("could not wrap the account key: {e}"))?;
    Ok((
        words,
        fresh_kdf_params(identifier),
        to_wire_blob(&wrapped),
        hash,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Break the thing this module exists for: wrap under one phrase, confirm
    /// a different phrase (or a different identifier) does not unwrap it, and
    /// the right phrase does - not by calling the same function twice, which
    /// would only prove determinism, but by rebuilding the wrap key
    /// independently of `unwrap_account_key`/`rewrap_under_new_phrase`.
    fn wrap_for_test(phrase: &str, identifier: &str, key: &crypto::SymmetricKey) -> WireBlob {
        let mnemonic = crypto::RecoveryMnemonic::from_phrase(phrase).expect("phrase");
        let recovery_key = mnemonic.derive_recovery_key(identifier).expect("derive");
        let stretched = crypto::stretch_master_key(&recovery_key).expect("stretch");
        let wrapped = crypto::symmetric::encrypt_key(key, &stretched).expect("encrypt");
        to_wire_blob(&wrapped)
    }

    #[test]
    fn recovery_input_classification_matches_the_server_side_branches() {
        assert_eq!(
            salt_identifier_for_recovery_input("USER@Example.com"),
            identifier(Some("user@example.com"), None, "irrelevant")
        );
        let wallet = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        assert_eq!(
            salt_identifier_for_recovery_input(wallet),
            identifier(None, Some(wallet), "irrelevant")
        );
        let account_id = "3f6b2c0a-1111-4a2b-9c3d-abcdefabcdef";
        assert_eq!(
            salt_identifier_for_recovery_input(account_id),
            identifier(None, None, account_id)
        );
        // Case-insensitive, the way the server's own `Uuid::parse_str` branch
        // is too (it lowercases before parsing).
        assert_eq!(
            salt_identifier_for_recovery_input(&account_id.to_uppercase()),
            identifier(None, None, account_id)
        );
    }

    #[test]
    fn identifier_prefers_email_then_wallet_then_passkey() {
        assert_eq!(
            identifier(Some("USER@Example.com"), Some("0xabc"), "u1"),
            "user@example.com"
        );
        assert_eq!(
            identifier(
                None,
                Some("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"),
                "u1"
            ),
            "wallet:0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
        assert_eq!(identifier(None, None, "u1"), "passkey:u1");
    }

    #[test]
    fn the_right_phrase_unwraps_what_it_wrapped() {
        let id = "passkey:test-user";
        let key = crypto::kdf::generate_symmetric_key();
        let words = generate_phrase().expect("generate");
        let phrase = words.join(" ");
        let blob = wrap_for_test(&phrase, id, &key);

        let unwrapped = unwrap_account_key(&phrase, id, &blob).expect("unwrap");
        assert_eq!(unwrapped.as_bytes(), key.as_bytes());
    }

    #[test]
    fn a_different_phrase_does_not_unwrap_it() {
        let id = "passkey:test-user";
        let key = crypto::kdf::generate_symmetric_key();
        let blob = wrap_for_test(&generate_phrase().unwrap().join(" "), id, &key);

        let other_phrase = generate_phrase().unwrap().join(" ");
        assert!(unwrap_account_key(&other_phrase, id, &blob).is_err());
    }

    #[test]
    fn the_same_phrase_under_a_different_identifier_does_not_unwrap_it() {
        let key = crypto::kdf::generate_symmetric_key();
        let phrase = generate_phrase().unwrap().join(" ");
        let blob = wrap_for_test(&phrase, "passkey:user-a", &key);

        assert!(unwrap_account_key(&phrase, "passkey:user-b", &blob).is_err());
    }

    #[test]
    fn verification_hash_matches_what_the_wrap_was_built_from() {
        let id = "wallet:0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        let phrase = generate_phrase().unwrap().join(" ");

        let mnemonic = crypto::RecoveryMnemonic::from_phrase(&phrase).unwrap();
        let recovery_key = mnemonic.derive_recovery_key(id).unwrap();
        let expected = crypto::recovery_verification_hash(&recovery_key);

        assert_eq!(verification_hash(&phrase, id).unwrap(), expected);
    }

    #[test]
    fn rewrap_produces_a_blob_the_new_phrase_can_unwrap_and_the_old_cannot() {
        let id = "passkey:test-user";
        let key = crypto::kdf::generate_symmetric_key();
        let old_phrase = generate_phrase().unwrap().join(" ");

        let (new_words, params, new_blob, hash) =
            rewrap_under_new_phrase(&key, id).expect("rewrap");
        let new_phrase = new_words.join(" ");

        assert_ne!(
            new_phrase, old_phrase,
            "a fresh phrase must actually be fresh"
        );
        assert_eq!(params.algorithm, "argon2id");
        assert_eq!(verification_hash(&new_phrase, id).unwrap(), hash);

        let unwrapped = unwrap_account_key(&new_phrase, id, &new_blob).expect("new phrase unwraps");
        assert_eq!(unwrapped.as_bytes(), key.as_bytes());

        assert!(
            unwrap_account_key(&old_phrase, id, &new_blob).is_err(),
            "the old phrase must not unwrap the re-wrapped blob"
        );
    }

    #[test]
    fn malformed_base64_is_refused_not_panicked_on() {
        let bad = WireBlob {
            ciphertext: "not base64 !!".to_string(),
            iv: "AAAA".to_string(),
            mac: "AAAA".to_string(),
        };
        assert!(unwrap_account_key("whatever phrase", "passkey:x", &bad).is_err());
    }
}
