//! Deriving a receiving account key from a BIP-39 recovery phrase.
//!
//! The output is an extended PUBLIC key and only that: the private half exists
//! for the length of one call and is dropped before anything is returned, so
//! the server (and the page) never hold a spending key.

use std::str::FromStr;

use bip32::{DerivationPath, Prefix, XPrv};
use zeroize::Zeroize;

/// The account-level path the server documents for an EVM wallet. The coin
/// type cannot be read back out of an xpub, so this must match the family the
/// wallet is registered under (`eip155`).
pub const EVM_ACCOUNT_PATH: &str = "m/44'/60'/0'";

/// Why a phrase could not become a receiving key. The messages are shown to
/// the merchant and never contain any part of the phrase.
#[derive(Debug, PartialEq, Eq)]
pub enum ReceivingKeyError {
    InvalidPhrase,
    Derivation,
}

impl std::fmt::Display for ReceivingKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPhrase => {
                f.write_str("That is not a valid recovery phrase. Check the words and their order.")
            }
            Self::Derivation => f.write_str("Could not derive a key from that phrase."),
        }
    }
}

/// The base58 `xpub` at [`EVM_ACCOUNT_PATH`] for a phrase, using the standard
/// BIP-39 seed with an empty passphrase - what an external wallet restoring
/// the same words would show.
pub fn evm_account_xpub(phrase: &str) -> Result<String, ReceivingKeyError> {
    let mnemonic = bip39::Mnemonic::parse_normalized(phrase.trim())
        .map_err(|_| ReceivingKeyError::InvalidPhrase)?;
    let mut seed = mnemonic.to_seed("");
    let path =
        DerivationPath::from_str(EVM_ACCOUNT_PATH).map_err(|_| ReceivingKeyError::Derivation)?;
    let derived = XPrv::derive_from_path(seed, &path);
    seed.zeroize();
    let account = derived.map_err(|_| ReceivingKeyError::Derivation)?;
    Ok(account.public_key().to_string(Prefix::XPUB))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bip32::{ChildNumber, PublicKey as _, XPub};
    use sha3::{Digest, Keccak256};

    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    /// EIP-55-less lowercase address of external/0/`index` under `xpub`.
    fn address(xpub: &str, index: u32) -> String {
        let account = XPub::from_str(xpub).unwrap();
        let key = account
            .derive_child(ChildNumber::new(0, false).unwrap())
            .unwrap()
            .derive_child(ChildNumber::new(index, false).unwrap())
            .unwrap();
        let point = k256::PublicKey::from_sec1_bytes(&key.public_key().to_bytes()).unwrap();
        let uncompressed =
            k256::elliptic_curve::sec1::ToEncodedPoint::to_encoded_point(&point, false);
        let hash = Keccak256::digest(&uncompressed.as_bytes()[1..]);
        format!("0x{}", hex::encode(&hash[12..]))
    }

    // The constants below are the first addresses every standard wallet
    // (MetaMask, Ledger Live, ethers' docs) shows for this public test phrase.
    // They are written out rather than computed, so a wrong depth or coin type
    // here cannot agree with itself.
    #[test]
    fn first_addresses_match_what_an_external_wallet_shows() {
        let xpub = evm_account_xpub(PHRASE).unwrap();
        assert_eq!(
            address(&xpub, 0),
            "0x9858effd232b4033e47d90003d41ec34ecaeda94"
        );
        assert_eq!(
            address(&xpub, 1),
            "0x6fac4d18c912343bf86fa7049364dd4e424ab9c0"
        );
    }

    #[test]
    fn result_is_an_account_level_xpub() {
        let xpub = evm_account_xpub(PHRASE).unwrap();
        assert!(xpub.starts_with("xpub"));
        let parsed = XPub::from_str(&xpub).unwrap();
        let attrs = parsed.attrs();
        assert_eq!(attrs.depth, 3);
        // Hardened index 0 at the account level.
        assert_eq!(u32::from(attrs.child_number), 0x8000_0000);
    }

    #[test]
    fn bad_phrase_is_refused_without_echoing_it() {
        let err = evm_account_xpub("not a phrase at all").unwrap_err();
        assert_eq!(err, ReceivingKeyError::InvalidPhrase);
        assert!(!err.to_string().contains("phrase at all"));
        // A valid word list with a broken checksum.
        let bad = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        assert_eq!(evm_account_xpub(bad), Err(ReceivingKeyError::InvalidPhrase));
    }
}
