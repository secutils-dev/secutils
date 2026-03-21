use anyhow::bail;
use argon2::Argon2;
use openssl::symm::{Cipher, Crypter, Mode};
use serde::{Deserialize, Serialize};

/// AES-256-GCM nonce size in bytes.
const NONCE_SIZE: usize = 12;
/// AES-256-GCM authentication tag size in bytes.
const TAG_SIZE: usize = 16;
/// Salt size in bytes (shared across all secrets in one export).
const SALT_SIZE: usize = 16;
/// Minimum passphrase length.
pub const SECRET_ENCRYPTION_MIN_PASSPHRASE_LENGTH: usize = 8;

/// Argon2id parameters (OWASP recommendation).
const ARGON2_M_COST: u32 = 19456; // 19 MiB
const ARGON2_T_COST: u32 = 2;
const ARGON2_P_COST: u32 = 1;

/// Metadata stored alongside encrypted secrets in the export file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsEncryptionMeta {
    pub alg: String,
    pub kdf: String,
    pub kdf_params: KdfParams,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub m: u32,
    pub t: u32,
    pub p: u32,
}

impl SecretsEncryptionMeta {
    /// Creates metadata for a new export with a random salt.
    pub fn new() -> Self {
        let salt = random_bytes::<SALT_SIZE>();
        Self {
            alg: "aes-256-gcm".to_string(),
            kdf: "argon2id".to_string(),
            kdf_params: KdfParams {
                m: ARGON2_M_COST,
                t: ARGON2_T_COST,
                p: ARGON2_P_COST,
            },
            salt: openssl::base64::encode_block(&salt),
        }
    }
}

/// Derives a 256-bit key from a passphrase and salt using Argon2id.
fn derive_key(passphrase: &str, salt: &[u8]) -> anyhow::Result<[u8; 32]> {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(32))
            .map_err(|e| anyhow::anyhow!("Invalid Argon2 parameters: {e}"))?,
    );
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("Argon2 key derivation failed: {e}"))?;
    Ok(key)
}

/// Encrypts a secret value for export using a passphrase-derived key.
/// Returns base64-encoded `nonce || ciphertext || tag`.
pub fn encrypt_secret_for_export(
    plaintext: &[u8],
    passphrase: &str,
    meta: &SecretsEncryptionMeta,
) -> anyhow::Result<String> {
    let salt = openssl::base64::decode_block(&meta.salt)
        .map_err(|e| anyhow::anyhow!("Failed to decode salt from export metadata: {e}"))?;
    let key = derive_key(passphrase, &salt)?;

    let cipher = Cipher::aes_256_gcm();
    let nonce = random_bytes::<NONCE_SIZE>();

    let mut crypter = Crypter::new(cipher, Mode::Encrypt, &key, Some(&nonce))?;
    let mut ciphertext = vec![0u8; plaintext.len() + cipher.block_size()];
    let mut count = crypter.update(plaintext, &mut ciphertext)?;
    count += crypter.finalize(&mut ciphertext[count..])?;
    ciphertext.truncate(count);

    let mut tag = vec![0u8; TAG_SIZE];
    crypter.get_tag(&mut tag)?;

    let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len() + TAG_SIZE);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    output.extend_from_slice(&tag);

    Ok(openssl::base64::encode_block(&output))
}

/// Decrypts a secret value from an export file using a passphrase-derived key.
/// Input is base64-encoded `nonce || ciphertext || tag`.
pub fn decrypt_secret_from_export(
    encrypted_base64: &str,
    passphrase: &str,
    meta: &SecretsEncryptionMeta,
) -> anyhow::Result<Vec<u8>> {
    let salt = openssl::base64::decode_block(&meta.salt)
        .map_err(|e| anyhow::anyhow!("Failed to decode salt from export metadata: {e}"))?;
    let key = derive_key(passphrase, &salt)?;

    let data = openssl::base64::decode_block(encrypted_base64)
        .map_err(|e| anyhow::anyhow!("Failed to decode encrypted secret value: {e}"))?;
    if data.len() < NONCE_SIZE + TAG_SIZE {
        bail!("Encrypted data is too short to contain nonce and tag.");
    }

    let cipher = Cipher::aes_256_gcm();
    let nonce = &data[..NONCE_SIZE];
    let tag = &data[data.len() - TAG_SIZE..];
    let ciphertext = &data[NONCE_SIZE..data.len() - TAG_SIZE];

    let mut crypter = Crypter::new(cipher, Mode::Decrypt, &key, Some(nonce))?;
    crypter.set_tag(tag)?;

    let mut plaintext = vec![0u8; ciphertext.len() + cipher.block_size()];
    let mut count = crypter.update(ciphertext, &mut plaintext)?;
    count += crypter.finalize(&mut plaintext[count..])?;
    plaintext.truncate(count);

    Ok(plaintext)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    openssl::rand::rand_bytes(&mut buf).expect("Failed to generate random bytes.");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt() -> anyhow::Result<()> {
        let meta = SecretsEncryptionMeta::new();
        let passphrase = "my-strong-passphrase";
        let plaintext = b"sk-live-abc123-secret-value";

        let encrypted = encrypt_secret_for_export(plaintext, passphrase, &meta)?;
        let decrypted = decrypt_secret_from_export(&encrypted, passphrase, &meta)?;

        assert_eq!(decrypted, plaintext);
        Ok(())
    }

    #[test]
    fn wrong_passphrase_fails() -> anyhow::Result<()> {
        let meta = SecretsEncryptionMeta::new();
        let encrypted = encrypt_secret_for_export(b"secret", "correct-passphrase", &meta)?;
        let result = decrypt_secret_from_export(&encrypted, "wrong-passphrase", &meta);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn different_encryptions_produce_different_output() -> anyhow::Result<()> {
        let meta = SecretsEncryptionMeta::new();
        let passphrase = "my-passphrase";
        let plaintext = b"hello";

        let a = encrypt_secret_for_export(plaintext, passphrase, &meta)?;
        let b = encrypt_secret_for_export(plaintext, passphrase, &meta)?;
        assert_ne!(a, b, "Random nonce should yield different ciphertext");

        assert_eq!(
            decrypt_secret_from_export(&a, passphrase, &meta)?,
            decrypt_secret_from_export(&b, passphrase, &meta)?
        );
        Ok(())
    }

    #[test]
    fn tampered_data_fails() -> anyhow::Result<()> {
        let meta = SecretsEncryptionMeta::new();
        let encrypted = encrypt_secret_for_export(b"secret", "passphrase", &meta)?;
        let mut data = openssl::base64::decode_block(&encrypted)?;
        let mid = data.len() / 2;
        data[mid] ^= 0xFF;
        let tampered = openssl::base64::encode_block(&data);
        assert!(decrypt_secret_from_export(&tampered, "passphrase", &meta).is_err());
        Ok(())
    }

    #[test]
    fn empty_plaintext_round_trip() -> anyhow::Result<()> {
        let meta = SecretsEncryptionMeta::new();
        let encrypted = encrypt_secret_for_export(b"", "passphrase", &meta)?;
        let decrypted = decrypt_secret_from_export(&encrypted, "passphrase", &meta)?;
        assert!(decrypted.is_empty());
        Ok(())
    }

    #[test]
    fn rejects_short_encrypted_data() {
        let meta = SecretsEncryptionMeta::new();
        let short = openssl::base64::encode_block(&[0u8; 10]);
        assert!(decrypt_secret_from_export(&short, "passphrase", &meta).is_err());
    }
}
