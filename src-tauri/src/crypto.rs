//! Snapshot encryption.
//!
//! Snapshots are encrypted with AES-256-GCM under a random 256-bit data key.
//! The data key never leaves a device in the clear: the backup repository
//! only holds it wrapped (see `keyfile`), and each device keeps its own copy
//! in Windows Credential Manager.
//!
//! Snapshot layout: `"ZSE2" || key id (8 B) || nonce (12 B) || ciphertext+tag`.
//! The magic and key id are authenticated as associated data, so a snapshot
//! can't be passed off under another key id.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm,
};
use generic_array::GenericArray;
use rand::RngCore;
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 4] = b"ZSE2";
pub const KEY_ID_LEN: usize = 8;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = MAGIC.len() + KEY_ID_LEN;

/// The key snapshots are encrypted with.
#[derive(Clone)]
pub struct DataKey {
    bytes: [u8; 32],
}

impl DataKey {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self { bytes }
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Short public identifier, so a snapshot or key file can say which key
    /// it belongs to. A truncated hash of a random 256-bit key reveals
    /// nothing useful about it.
    pub fn id(&self) -> [u8; KEY_ID_LEN] {
        let digest = Sha256::new()
            .chain_update(b"zen-sync key id v1")
            .chain_update(self.bytes)
            .finalize();
        let mut id = [0u8; KEY_ID_LEN];
        id.copy_from_slice(&digest[..KEY_ID_LEN]);
        id
    }

    pub fn id_hex(&self) -> String {
        self.id().iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Encrypt a snapshot.
pub fn encrypt(data: &[u8], key: &DataKey) -> Result<Vec<u8>, String> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&key.id());
    let (nonce, ciphertext) = seal(key.bytes(), data, &header)?;

    let mut out = header;
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a snapshot made by `encrypt`.
pub fn decrypt(data: &[u8], key: &DataKey) -> Result<Vec<u8>, String> {
    if data.len() < HEADER_LEN + NONCE_LEN || &data[..MAGIC.len()] != MAGIC {
        return Err(
            "This snapshot was made by Zen Sync 0.5 or earlier, which used a different encryption key. It can't be restored; make a new backup on the device it came from."
                .into(),
        );
    }
    let (header, rest) = data.split_at(HEADER_LEN);
    if header[MAGIC.len()..] != key.id() {
        return Err(
            "This snapshot was encrypted with a different key, from before encryption was set up again. Make a new backup on the device it came from."
                .into(),
        );
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    open(key.bytes(), nonce, ciphertext, header)
        .ok_or_else(|| "Decryption failed: the snapshot is damaged.".to_string())
}

/// AES-256-GCM encryption with associated data. Returns `(nonce, ciphertext)`.
pub(crate) fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<([u8; NONCE_LEN], Vec<u8>), String> {
    let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(key));
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(GenericArray::from_slice(&nonce), Payload { msg: plaintext, aad })
        .map_err(|e| format!("Encryption error: {e}"))?;
    Ok((nonce, ciphertext))
}

/// Counterpart of `seal`. `None` if the key, nonce, data or associated data
/// don't match.
pub(crate) fn open(key: &[u8; 32], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Option<Vec<u8>> {
    if nonce.len() != NONCE_LEN {
        return None;
    }
    let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(GenericArray::from_slice(nonce), Payload { msg: ciphertext, aad })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key(seed: u8) -> DataKey {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = seed.wrapping_add(i as u8);
        }
        DataKey::from_bytes(k)
    }

    #[test]
    fn round_trip() {
        let key = test_key(0);
        let plain = b"hello zen-sync";
        let enc = encrypt(plain, &key).unwrap();
        assert_eq!(&enc[..4], MAGIC);
        assert_eq!(enc[4..12], key.id());
        assert_eq!(decrypt(&enc, &key).unwrap(), plain);
    }

    #[test]
    fn other_key_is_named_as_such() {
        let enc = encrypt(b"secret", &test_key(0)).unwrap();
        let err = decrypt(&enc, &test_key(1)).unwrap_err();
        assert!(err.contains("different key"), "{err}");
    }

    #[test]
    fn tampered_header_or_body_fails() {
        let key = test_key(0);
        let mut enc = encrypt(b"secret", &key).unwrap();
        let last = enc.len() - 1;
        enc[last] ^= 1;
        assert!(decrypt(&enc, &key).unwrap_err().contains("damaged"));
    }

    #[test]
    fn old_format_is_recognised() {
        // 0.5.x: nonce || ciphertext, no header.
        let err = decrypt(&[7u8; 64], &test_key(0)).unwrap_err();
        assert!(err.contains("0.5 or earlier"), "{err}");
        assert!(decrypt(&[0u8; 4], &test_key(0)).is_err());
    }

    #[test]
    fn key_ids_differ_per_key() {
        assert_ne!(test_key(0).id(), test_key(1).id());
        assert_eq!(test_key(0).id_hex().len(), KEY_ID_LEN * 2);
    }
}
