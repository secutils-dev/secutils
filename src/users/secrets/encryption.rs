use anyhow::{Context, bail};
use openssl::symm::{Cipher, Crypter, Mode};

/// AES-256-GCM nonce size in bytes.
const NONCE_SIZE: usize = 12;
/// AES-256-GCM authentication tag size in bytes.
const TAG_SIZE: usize = 16;

/// Handles encryption/decryption of user secret values using AES-256-GCM.
#[derive(Clone)]
pub struct SecretsEncryption {
    key: Vec<u8>,
}

impl SecretsEncryption {
    /// Creates a new instance from a hex-encoded 32-byte key.
    pub fn new(hex_key: &str) -> anyhow::Result<Self> {
        let key =
            hex::decode(hex_key).with_context(|| "Secrets encryption key is not valid hex.")?;
        if key.len() != 32 {
            bail!(
                "Secrets encryption key must be 32 bytes (256 bits), got {} bytes.",
                key.len()
            );
        }
        Ok(Self { key })
    }

    /// Encrypts plaintext using AES-256-GCM with a random nonce.
    /// Returns `nonce || ciphertext || tag`.
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let cipher = Cipher::aes_256_gcm();
        let nonce = Self::random_nonce();

        let mut crypter = Crypter::new(cipher, Mode::Encrypt, &self.key, Some(&nonce))?;
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

        Ok(output)
    }

    /// Decrypts data previously produced by [`encrypt`]. Expects `nonce || ciphertext || tag`.
    pub fn decrypt(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if data.len() < NONCE_SIZE + TAG_SIZE {
            bail!("Encrypted data is too short to contain nonce and tag.");
        }

        let cipher = Cipher::aes_256_gcm();
        let nonce = &data[..NONCE_SIZE];
        let tag = &data[data.len() - TAG_SIZE..];
        let ciphertext = &data[NONCE_SIZE..data.len() - TAG_SIZE];

        let mut crypter = Crypter::new(cipher, Mode::Decrypt, &self.key, Some(nonce))?;
        crypter.set_tag(tag)?;

        let mut plaintext = vec![0u8; ciphertext.len() + cipher.block_size()];
        let mut count = crypter.update(ciphertext, &mut plaintext)?;
        count += crypter.finalize(&mut plaintext[count..])?;
        plaintext.truncate(count);

        Ok(plaintext)
    }

    fn random_nonce() -> [u8; NONCE_SIZE] {
        let mut nonce = [0u8; NONCE_SIZE];
        openssl::rand::rand_bytes(&mut nonce).expect("Failed to generate random nonce.");
        nonce
    }
}

#[cfg(test)]
mod tests {
    use super::SecretsEncryption;

    fn test_key_hex() -> String {
        "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string()
    }

    #[test]
    fn rejects_invalid_hex_key() {
        assert!(SecretsEncryption::new("not-hex").is_err());
    }

    #[test]
    fn rejects_wrong_length_key() {
        assert!(SecretsEncryption::new("aabb").is_err());
    }

    #[test]
    fn encrypt_decrypt_round_trip() -> anyhow::Result<()> {
        let enc = SecretsEncryption::new(&test_key_hex())?;
        let plaintext = b"my-super-secret-api-key-12345";
        let encrypted = enc.encrypt(plaintext)?;
        assert_ne!(encrypted, plaintext);
        let decrypted = enc.decrypt(&encrypted)?;
        assert_eq!(decrypted, plaintext);
        Ok(())
    }

    #[test]
    fn encrypt_produces_different_ciphertext_each_time() -> anyhow::Result<()> {
        let enc = SecretsEncryption::new(&test_key_hex())?;
        let plaintext = b"hello";
        let a = enc.encrypt(plaintext)?;
        let b = enc.encrypt(plaintext)?;
        assert_ne!(a, b, "Random nonce should yield different ciphertext");
        assert_eq!(enc.decrypt(&a)?, enc.decrypt(&b)?);
        Ok(())
    }

    #[test]
    fn decrypt_rejects_tampered_data() -> anyhow::Result<()> {
        let enc = SecretsEncryption::new(&test_key_hex())?;
        let mut encrypted = enc.encrypt(b"secret")?;
        let mid = encrypted.len() / 2;
        encrypted[mid] ^= 0xFF;
        assert!(enc.decrypt(&encrypted).is_err());
        Ok(())
    }

    #[test]
    fn decrypt_rejects_too_short_data() -> anyhow::Result<()> {
        let enc = SecretsEncryption::new(&test_key_hex())?;
        assert!(enc.decrypt(&[0u8; 10]).is_err());
        Ok(())
    }

    #[test]
    fn encrypt_empty_plaintext() -> anyhow::Result<()> {
        let enc = SecretsEncryption::new(&test_key_hex())?;
        let encrypted = enc.encrypt(b"")?;
        let decrypted = enc.decrypt(&encrypted)?;
        assert!(decrypted.is_empty());
        Ok(())
    }

    #[test]
    fn encrypt_large_payload() -> anyhow::Result<()> {
        let enc = SecretsEncryption::new(&test_key_hex())?;
        let plaintext = vec![0xAB; 10 * 1024];
        let encrypted = enc.encrypt(&plaintext)?;
        let decrypted = enc.decrypt(&encrypted)?;
        assert_eq!(decrypted, plaintext);
        Ok(())
    }
}
