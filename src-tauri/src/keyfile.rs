//! The key file: the snapshot key, stored in the backup repository only in
//! wrapped form, so nobody who can read the repository can read the backups.
//!
//! The key is wrapped twice, each time with AES-256-GCM under a key derived
//! with Argon2id: once from the user's passphrase and once from a random
//! recovery code. Either one unlocks it on a new device. Changing the
//! passphrase re-wraps the key; snapshots don't need re-encrypting.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::crypto::{self, DataKey};

/// Path of the key file in the backup repository.
pub const KEY_FILE_PATH: &str = "encryption-v2.json";
const FORMAT_VERSION: u32 = 1;
const KDF_NAME: &str = "argon2id";
const SALT_LEN: usize = 16;

pub const MIN_PASSPHRASE_CHARS: usize = 10;

const KEYCHAIN_SERVICE: &str = "zen-sync";
/// This device's copy of the data key.
const KEYCHAIN_DATA_KEY: &str = "data-key-v2";
/// Where zen-sync 0.5 and earlier kept the old, unwrapped key.
const KEYCHAIN_OLD_KEY: &str = "encryption-key";

/// Argon2id cost. The defaults (64 MiB, 3 passes) take well under a second on
/// a current PC and make guessing a passphrase from a copy of the key file
/// slow and memory-hungry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub const DEFAULT: Self = Self { memory_kib: 64 * 1024, iterations: 3, parallelism: 1 };

    /// Refuse parameters no version of the app would write, so a tampered
    /// key file can't make unlocking take minutes or gigabytes.
    fn check(&self) -> Result<(), String> {
        let sane = (8..=1024 * 1024).contains(&self.memory_kib)
            && (1..=16).contains(&self.iterations)
            && (1..=8).contains(&self.parallelism);
        if sane {
            Ok(())
        } else {
            Err("The key file has key-derivation settings this version doesn't accept.".into())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyFile {
    pub version: u32,
    /// `DataKey::id_hex` of the wrapped key, so a device can tell whether the
    /// key it already has is the current one without unlocking anything.
    pub key_id: String,
    pub passphrase: WrappedKey,
    pub recovery: WrappedKey,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WrappedKey {
    pub kdf: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Clone, Copy)]
enum Slot {
    Passphrase,
    Recovery,
}

impl Slot {
    /// Associated data: binds each wrapped copy to its slot, so the two
    /// can't be swapped.
    fn aad(self) -> &'static [u8] {
        match self {
            Slot::Passphrase => b"zen-sync key file v1: passphrase",
            Slot::Recovery => b"zen-sync key file v1: recovery code",
        }
    }
}

fn derive_kek(secret: &[u8], salt: &[u8], params: KdfParams) -> Result<[u8; 32], String> {
    params.check()?;
    let argon_params = argon2::Params::new(params.memory_kib, params.iterations, params.parallelism, Some(32))
        .map_err(|e| format!("Key derivation settings rejected: {e}"))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, argon_params);
    let mut kek = [0u8; 32];
    argon
        .hash_password_into(secret, salt, &mut kek)
        .map_err(|e| format!("Key derivation failed: {e}"))?;
    Ok(kek)
}

fn wrap_key(key: &DataKey, secret: &[u8], slot: Slot, params: KdfParams) -> Result<WrappedKey, String> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let kek = derive_kek(secret, &salt, params)?;
    let (nonce, ciphertext) = crypto::seal(&kek, key.bytes(), slot.aad())?;
    Ok(WrappedKey {
        kdf: KDF_NAME.into(),
        memory_kib: params.memory_kib,
        iterations: params.iterations,
        parallelism: params.parallelism,
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce),
        ciphertext: BASE64.encode(ciphertext),
    })
}

/// `Ok(None)` if the secret is wrong.
fn unwrap_key(wrapped: &WrappedKey, secret: &[u8], slot: Slot) -> Result<Option<DataKey>, String> {
    if wrapped.kdf != KDF_NAME {
        return Err(format!("The key file uses '{}', which this version doesn't support.", wrapped.kdf));
    }
    let params = KdfParams {
        memory_kib: wrapped.memory_kib,
        iterations: wrapped.iterations,
        parallelism: wrapped.parallelism,
    };
    let decode = |field: &str| {
        BASE64
            .decode(field)
            .map_err(|e| format!("The key file is damaged: {e}"))
    };
    let salt = decode(&wrapped.salt)?;
    let nonce = decode(&wrapped.nonce)?;
    let ciphertext = decode(&wrapped.ciphertext)?;
    let kek = derive_kek(secret, &salt, params)?;
    let Some(bytes) = crypto::open(&kek, &nonce, &ciphertext, slot.aad()) else {
        return Ok(None);
    };
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "The key file is damaged: wrong key length.".to_string())?;
    Ok(Some(DataKey::from_bytes(bytes)))
}

/// A passphrase must be at least `MIN_PASSPHRASE_CHARS` characters.
pub fn check_passphrase(passphrase: &str) -> Result<(), String> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(format!("Use at least {MIN_PASSPHRASE_CHARS} characters for the passphrase."));
    }
    Ok(())
}

impl KeyFile {
    pub fn create(key: &DataKey, passphrase: &str, recovery: &RecoveryCode) -> Result<Self, String> {
        Self::create_with(key, passphrase, recovery, KdfParams::DEFAULT)
    }

    fn create_with(
        key: &DataKey,
        passphrase: &str,
        recovery: &RecoveryCode,
        params: KdfParams,
    ) -> Result<Self, String> {
        check_passphrase(passphrase)?;
        Ok(Self {
            version: FORMAT_VERSION,
            key_id: key.id_hex(),
            passphrase: wrap_key(key, passphrase.as_bytes(), Slot::Passphrase, params)?,
            recovery: wrap_key(key, recovery.secret(), Slot::Recovery, params)?,
        })
    }

    /// Check the version before using a key file read from the repository.
    pub fn check_version(&self) -> Result<(), String> {
        if self.version == FORMAT_VERSION {
            Ok(())
        } else {
            Err(format!(
                "The backup repository's key file has format {}, which needs a newer Zen Sync. Update the app.",
                self.version
            ))
        }
    }

    /// Unlock with either the passphrase or the recovery code. `Ok(None)` if
    /// it is neither.
    pub fn unlock(&self, secret: &str) -> Result<Option<DataKey>, String> {
        self.check_version()?;
        // Try the recovery code first when the input looks like one; each
        // attempt costs one key derivation.
        let key = match RecoveryCode::parse(secret) {
            Some(code) => match unwrap_key(&self.recovery, code.secret(), Slot::Recovery)? {
                Some(key) => Some(key),
                None => unwrap_key(&self.passphrase, secret.as_bytes(), Slot::Passphrase)?,
            },
            None => unwrap_key(&self.passphrase, secret.as_bytes(), Slot::Passphrase)?,
        };
        self.verified(key)
    }

    /// Unlock with the passphrase only, to confirm it before a change.
    pub fn unlock_with_passphrase(&self, passphrase: &str) -> Result<Option<DataKey>, String> {
        self.check_version()?;
        let key = unwrap_key(&self.passphrase, passphrase.as_bytes(), Slot::Passphrase)?;
        self.verified(key)
    }

    fn verified(&self, key: Option<DataKey>) -> Result<Option<DataKey>, String> {
        match key {
            Some(key) if key.id_hex() != self.key_id => {
                Err("The key file is inconsistent: the unwrapped key doesn't match its id.".into())
            }
            other => Ok(other),
        }
    }

    /// This key file with the passphrase replaced.
    pub fn with_passphrase(&self, key: &DataKey, passphrase: &str) -> Result<Self, String> {
        check_passphrase(passphrase)?;
        Ok(Self {
            passphrase: wrap_key(key, passphrase.as_bytes(), Slot::Passphrase, KdfParams::DEFAULT)?,
            ..self.clone()
        })
    }

    /// This key file with the recovery code replaced.
    pub fn with_recovery_code(&self, key: &DataKey, recovery: &RecoveryCode) -> Result<Self, String> {
        Ok(Self {
            recovery: wrap_key(key, recovery.secret(), Slot::Recovery, KdfParams::DEFAULT)?,
            ..self.clone()
        })
    }
}

// ── Recovery code ─────────────────────────────────────────────────────────────

/// Crockford base32: no I, L, O or U, so the code survives being read aloud
/// or copied by hand.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LEN: usize = 25;
const GROUP_LEN: usize = 5;

/// 25 random base32 characters (125 bits): far too many to guess, so unlike
/// a passphrase it doesn't depend on Argon2 being slow.
pub struct RecoveryCode(String);

impl RecoveryCode {
    pub fn generate() -> Self {
        let mut bytes = [0u8; CODE_LEN];
        rand::thread_rng().fill_bytes(&mut bytes);
        // 256 is a multiple of 32, so masking keeps every symbol equally likely.
        Self(bytes.iter().map(|b| ALPHABET[(b & 31) as usize] as char).collect())
    }

    /// Accepts the code as typed: any case, with or without dashes or spaces,
    /// and with I/L read as 1 and O as 0.
    pub fn parse(input: &str) -> Option<Self> {
        let mut code = String::with_capacity(CODE_LEN);
        for c in input.chars() {
            let c = match c.to_ascii_uppercase() {
                '-' | ' ' => continue,
                'I' | 'L' => '1',
                'O' => '0',
                c => c,
            };
            if !c.is_ascii() || !ALPHABET.contains(&(c as u8)) {
                return None;
            }
            code.push(c);
        }
        (code.len() == CODE_LEN).then_some(Self(code))
    }

    /// For showing to the user: `ABCDE-FGHJK-…`.
    pub fn display(&self) -> String {
        self.0
            .as_bytes()
            .chunks(GROUP_LEN)
            .map(|g| String::from_utf8_lossy(g).into_owned())
            .collect::<Vec<_>>()
            .join("-")
    }

    fn secret(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

// ── This device's copy ────────────────────────────────────────────────────────

fn keychain(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| format!("Keychain error: {e}"))
}

/// Keep the data key in Windows Credential Manager, replacing an old one.
pub fn save_local_key(key: &DataKey) -> Result<(), String> {
    keychain(KEYCHAIN_DATA_KEY)?
        .set_password(&BASE64.encode(key.bytes()))
        .map_err(|e| format!("Failed to save the encryption key: {e}"))?;
    forget_old_key();
    Ok(())
}

pub fn load_local_key() -> Option<DataKey> {
    let b64 = keychain(KEYCHAIN_DATA_KEY).ok()?.get_password().ok()?;
    let bytes: [u8; 32] = BASE64.decode(b64.trim()).ok()?.try_into().ok()?;
    Some(DataKey::from_bytes(bytes))
}

/// Remove this device's copy of the key (and any pre-0.6 one).
pub fn forget_local_key() -> Result<(), String> {
    match keychain(KEYCHAIN_DATA_KEY)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(format!("Failed to remove the encryption key: {e}")),
    }
    forget_old_key();
    Ok(())
}

fn forget_old_key() {
    if let Ok(entry) = keychain(KEYCHAIN_OLD_KEY) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap settings: the defaults are slow in unoptimised test builds.
    const FAST: KdfParams = KdfParams { memory_kib: 64, iterations: 1, parallelism: 1 };

    fn key_file() -> (DataKey, RecoveryCode, KeyFile) {
        let key = DataKey::generate();
        let code = RecoveryCode::generate();
        let file = KeyFile::create_with(&key, "correct horse battery", &code, FAST).unwrap();
        (key, code, file)
    }

    #[test]
    fn unlocks_with_passphrase_or_recovery_code() {
        let (key, code, file) = key_file();
        let by_pass = file.unlock("correct horse battery").unwrap().unwrap();
        assert_eq!(by_pass.bytes(), key.bytes());
        // Typed loosely: lower case, spaces instead of dashes.
        let typed = code.display().to_lowercase().replace('-', " ");
        let by_code = file.unlock(&typed).unwrap().unwrap();
        assert_eq!(by_code.bytes(), key.bytes());
    }

    #[test]
    fn wrong_secret_is_none_not_error() {
        let (_, _, file) = key_file();
        assert!(file.unlock("wrong horse battery").unwrap().is_none());
        assert!(file.unlock(&RecoveryCode::generate().display()).unwrap().is_none());
        assert!(file.unlock_with_passphrase("wrong horse battery").unwrap().is_none());
    }

    #[test]
    fn key_file_holds_no_usable_key() {
        let (key, _, file) = key_file();
        let json = serde_json::to_string(&file).unwrap();
        assert!(!json.contains(&BASE64.encode(key.bytes())));
        assert_eq!(file.key_id, key.id_hex());
    }

    #[test]
    fn changing_the_passphrase_keeps_the_key_and_recovery_code() {
        let (key, code, file) = key_file();
        let changed = KeyFile { passphrase: wrap_key(&key, b"a new passphrase", Slot::Passphrase, FAST).unwrap(), ..file.clone() };
        assert!(changed.unlock("correct horse battery").unwrap().is_none());
        assert_eq!(changed.unlock("a new passphrase").unwrap().unwrap().bytes(), key.bytes());
        assert_eq!(changed.unlock(&code.display()).unwrap().unwrap().bytes(), key.bytes());
    }

    #[test]
    fn slots_cannot_be_swapped() {
        let (_, _, file) = key_file();
        let swapped = KeyFile { passphrase: file.recovery.clone(), recovery: file.passphrase.clone(), ..file };
        assert!(swapped.unlock("correct horse battery").unwrap().is_none());
    }

    #[test]
    fn short_passphrases_and_odd_settings_are_refused() {
        let key = DataKey::generate();
        assert!(KeyFile::create_with(&key, "short", &RecoveryCode::generate(), FAST).is_err());
        let (_, _, mut file) = key_file();
        file.passphrase.memory_kib = 64 * 1024 * 1024;
        assert!(file.unlock_with_passphrase("correct horse battery").is_err());
    }

    #[test]
    fn newer_key_file_format_is_refused() {
        let (_, _, mut file) = key_file();
        file.version = 2;
        assert!(file.unlock("correct horse battery").unwrap_err().contains("newer"));
    }

    #[test]
    fn recovery_codes_parse_leniently_and_display_grouped() {
        let code = RecoveryCode::generate();
        let shown = code.display();
        assert_eq!(shown.len(), CODE_LEN + CODE_LEN / GROUP_LEN - 1);
        assert_eq!(RecoveryCode::parse(&shown).unwrap().0, code.0);
        assert_eq!(RecoveryCode::parse("abcde fghjk mnpqr stvwx yz01i").unwrap().0, "ABCDEFGHJKMNPQRSTVWXYZ011");
        assert!(RecoveryCode::parse("too short").is_none());
        assert!(RecoveryCode::parse("correct horse battery").is_none());
        assert!(RecoveryCode::parse("UUUUU-UUUUU-UUUUU-UUUUU-UUUUU").is_none());
    }
}
