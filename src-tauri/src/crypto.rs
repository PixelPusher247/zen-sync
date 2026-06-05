use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm,
};
use generic_array::GenericArray;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;

const APP_SALT: &[u8] = b"zen-sync-v1-salt";
const PBKDF2_ROUNDS: u32 = 100_000;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

fn derive_key(key_bytes: &[u8]) -> [u8; KEY_LEN] {
    let mut out = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(key_bytes, APP_SALT, PBKDF2_ROUNDS, &mut out);
    out
}

/// Encrypt `data` with AES-256-GCM. Returns `[nonce (12 B) || ciphertext+tag]`.
pub fn encrypt(data: &[u8], raw_key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let key_bytes = derive_key(raw_key);
    let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("Encryption error: {e}"))?;

    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data produced by `encrypt`.
pub fn decrypt(data: &[u8], raw_key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if data.len() < NONCE_LEN {
        return Err("Payload too short — not a valid zen-sync bundle".into());
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let key_bytes = derive_key(raw_key);
    let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = GenericArray::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed — wrong key or corrupted bundle".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = i as u8;
        }
        k
    }

    #[test]
    fn round_trip() {
        let key = test_key();
        let plain = b"hello zen-sync";
        let enc = encrypt(plain, &key).unwrap();
        assert_ne!(&enc[NONCE_LEN..], plain.as_ref());
        let dec = decrypt(&enc, &key).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn wrong_key_fails() {
        let key = test_key();
        let mut wrong_key = test_key();
        wrong_key[0] ^= 0xff;
        let enc = encrypt(b"secret", &key).unwrap();
        assert!(decrypt(&enc, &wrong_key).is_err());
    }

    #[test]
    fn truncated_fails() {
        assert!(decrypt(&[0u8; 4], &test_key()).is_err());
    }
}
