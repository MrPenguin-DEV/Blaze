//! Local authenticated encryption for user-authored Blaze files.
//!
//! Passwords are never persisted. Every encrypted envelope carries a random
//! salt and nonce. This is a file-encryption primitive, not a key-management
//! system or a replacement for OS account security.

use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand_core::{OsRng, RngCore};
use std::path::Path;
use thiserror::Error;
use zeroize::Zeroizing;

const MAGIC: &[u8; 5] = b"BLZV1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("encrypted data is truncated or has an unknown format")]
    InvalidEnvelope,
    #[error("password derivation failed")]
    KeyDerivation,
    #[error("random number generation failed")]
    Randomness,
    #[error("authentication failed: password is wrong or data was modified")]
    Authentication,
    #[error("file operation failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Encrypts bytes using Argon2-derived key material and ChaCha20-Poly1305.
///
/// The returned envelope is self-contained: magic || salt || nonce ||
/// authenticated ciphertext. Use a strong, unique password.
pub fn seal(password: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut salt)
        .map_err(|_| VaultError::Randomness)?;
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|_| VaultError::Randomness)?;

    let key = derive_key(password, &salt)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key[..])
        .map_err(|_| VaultError::KeyDerivation)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| VaultError::Authentication)?;

    let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    envelope.extend_from_slice(MAGIC);
    envelope.extend_from_slice(&salt);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// Authenticates and decrypts a value produced by seal.
pub fn open(password: &[u8], envelope: &[u8]) -> Result<Vec<u8>, VaultError> {
    if envelope.len() < HEADER_LEN || &envelope[..MAGIC.len()] != MAGIC {
        return Err(VaultError::InvalidEnvelope);
    }

    let salt_start = MAGIC.len();
    let nonce_start = salt_start + SALT_LEN;
    let cipher_start = nonce_start + NONCE_LEN;
    let salt = &envelope[salt_start..nonce_start];
    let nonce = &envelope[nonce_start..cipher_start];
    let ciphertext = &envelope[cipher_start..];

    let key = derive_key(password, salt)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key[..])
        .map_err(|_| VaultError::KeyDerivation)?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| VaultError::Authentication)
}

/// Encrypts then atomically replaces a file in the same directory.
pub fn write_encrypted(
    path: impl AsRef<Path>,
    password: &[u8],
    plaintext: &[u8],
) -> Result<(), VaultError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let temp_path = path.with_extension("blaze-tmp");
    let encrypted = seal(password, plaintext)?;
    std::fs::write(&temp_path, encrypted)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
}

/// Reads an encrypted file and authenticates it before returning plaintext.
pub fn read_encrypted(
    path: impl AsRef<Path>,
    password: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let encrypted = std::fs::read(path)?;
    open(password, &encrypted)
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let mut key = Zeroizing::new([0_u8; 32]);
    Argon2::default()
        .hash_password_into(password, salt, &mut key[..])
        .map_err(|_| VaultError::KeyDerivation)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let sealed = seal(b"correct horse battery staple", b"private notes").unwrap();
        let plain = open(b"correct horse battery staple", &sealed).unwrap();
        assert_eq!(plain, b"private notes");
    }

    #[test]
    fn wrong_password_is_rejected() {
        let sealed = seal(b"right password", b"secret").unwrap();
        assert!(matches!(
            open(b"wrong password", &sealed),
            Err(VaultError::Authentication)
        ));
    }

    #[test]
    fn tampering_is_detected() {
        let mut sealed = seal(b"password", b"secret").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x80;
        assert!(matches!(
            open(b"password", &sealed),
            Err(VaultError::Authentication)
        ));
    }
}
