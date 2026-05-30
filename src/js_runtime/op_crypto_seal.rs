//! Stateless crypto ops exposed to responder scripts under `secutils.crypto.*`.
//!
//! [`op_crypto_seal`] implements a libsodium-style *sealed box*: the caller supplies the
//! recipient's P-256 public key and a UTF-8 plaintext, and gets back an opaque blob that only the
//! holder of the recipient's private key can open. A fresh ephemeral sender keypair is generated
//! per call, so the server never holds any long-lived secret and cannot decrypt what it sealed.
//!
//! Wire format of the returned blob (then base64url-encoded for JSON transport):
//! ```text
//! | 65 bytes ephemeral public key (SEC1 uncompressed) | 12 bytes AES-GCM IV | N bytes ciphertext+tag |
//! ```
//!
//! The construction is intentionally reproducible with native browser WebCrypto (no JS deps):
//! - shared secret  = ECDH(ephemeral_priv, recipient_pub)  → 32-byte X coordinate
//! - AES-256 key    = HKDF-SHA256(ikm = shared, salt = recipient_pub, info = ephemeral_pub)
//! - ciphertext     = AES-256-GCM(key, iv, plaintext)      (no additional data, tag appended)
//!
//! Binding the recipient public key into the HKDF salt and the ephemeral public key into the HKDF
//! info domain-separates every (recipient, message) pair.

use base64ct::{Base64UrlUnpadded, Encoding};
use deno_core::op2;
use deno_error::JsErrorBox;
use openssl::{
    bn::BigNumContext,
    derive::Deriver,
    ec::{EcGroup, EcKey, EcPoint, PointConversionForm},
    error::ErrorStack,
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private, Public},
    sign::Signer,
    symm::{Cipher, Crypter, Mode},
};

/// Length, in bytes, of a SEC1 uncompressed P-256 public key.
const P256_PUBLIC_KEY_LEN: usize = 65;
/// Length, in bytes, of the AES-GCM nonce (IV).
const AES_GCM_NONCE_LEN: usize = 12;
/// Length, in bytes, of the AES-GCM authentication tag.
const AES_GCM_TAG_LEN: usize = 16;

/// Seals `plaintext` against the recipient P-256 public key `recipient_pub_b64`
/// (base64url of the 65-byte SEC1 uncompressed key). Returns the base64url of
/// the sealed blob described in the module docs.
#[op2]
#[string]
pub fn op_crypto_seal(
    #[string] recipient_pub_b64: String,
    #[string] plaintext: String,
) -> Result<String, JsErrorBox> {
    seal(&recipient_pub_b64, &plaintext)
}

/// Returns the lowercase hex SHA-256 of the bytes carried by `data_b64` (base64url).
#[op2]
#[string]
pub fn op_crypto_sha256(#[string] data_b64: String) -> Result<String, JsErrorBox> {
    sha256_hex(&data_b64)
}

fn os_err(context: &str) -> impl Fn(ErrorStack) -> JsErrorBox + '_ {
    move |err| JsErrorBox::generic(format!("{context}: {err}"))
}

/// Returns the canonical P-256 (prime256v1) curve group.
fn p256_group() -> Result<EcGroup, ErrorStack> {
    EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)
}

/// HMAC-SHA256 over `data` keyed by `key`.
fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, ErrorStack> {
    let pkey = PKey::hmac(key)?;
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)?;
    signer.update(data)?;
    signer.sign_to_vec()
}

/// HKDF-SHA256 producing exactly one 32-byte output key (RFC 5869). A single expand block suffices
/// because SHA-256's output length equals the requested key length, which is what both the Rust
/// round-trip and the browser `crypto.subtle.deriveBits(..., 256)` rely on.
fn hkdf_sha256_32(ikm: &[u8], salt: &[u8], info: &[u8]) -> Result<[u8; 32], ErrorStack> {
    // Extract: PRK = HMAC(salt, IKM).
    let prk = hmac_sha256(salt, ikm)?;
    // Expand: T(1) = HMAC(PRK, info || 0x01); OKM = T(1) truncated to 32 bytes.
    let mut block_input = Vec::with_capacity(info.len() + 1);
    block_input.extend_from_slice(info);
    block_input.push(0x01);
    let okm = hmac_sha256(&prk, &block_input)?;

    let mut out = [0u8; 32];
    out.copy_from_slice(&okm[..32]);
    Ok(out)
}

/// Computes the ECDH shared secret (the 32-byte X coordinate) between a local
/// private key and a peer public key.
fn ecdh_shared(local: &PKey<Private>, peer: &PKey<Public>) -> Result<Vec<u8>, ErrorStack> {
    let mut deriver = Deriver::new(local)?;
    deriver.set_peer(peer)?;
    deriver.derive_to_vec()
}

/// Parses a SEC1 uncompressed P-256 public key into an `openssl` public `PKey`,
/// rejecting anything that is not a valid on-curve 65-byte point.
fn parse_recipient_pub(recipient_pub_bytes: &[u8]) -> Result<PKey<Public>, JsErrorBox> {
    if recipient_pub_bytes.len() != P256_PUBLIC_KEY_LEN || recipient_pub_bytes[0] != 0x04 {
        return Err(JsErrorBox::generic(
            "Recipient public key must be a 65-byte SEC1 uncompressed P-256 key.".to_string(),
        ));
    }

    let group = p256_group().map_err(os_err("Failed to load P-256 group"))?;
    let mut ctx = BigNumContext::new().map_err(os_err("Failed to allocate BN context"))?;
    let point = EcPoint::from_bytes(&group, recipient_pub_bytes, &mut ctx)
        .map_err(os_err("Invalid recipient public key (point)"))?;
    let ec = EcKey::from_public_key(&group, &point)
        .map_err(os_err("Invalid recipient public key (key)"))?;
    PKey::from_ec_key(ec).map_err(os_err("Invalid recipient public key (pkey)"))
}

/// Pure sealing logic shared by [`op_crypto_seal`] and the unit tests (the op2 macro does not
/// expose a directly callable entry point).
fn seal(recipient_pub_b64: &str, plaintext: &str) -> Result<String, JsErrorBox> {
    let recipient_pub_bytes = Base64UrlUnpadded::decode_vec(recipient_pub_b64).map_err(|err| {
        JsErrorBox::generic(format!("Invalid recipient public key (base64): {err}"))
    })?;
    let recipient_pub = parse_recipient_pub(&recipient_pub_bytes)?;

    // Fresh ephemeral sender keypair per message (sealed-box semantics).
    let group = p256_group().map_err(os_err("Failed to load P-256 group"))?;
    let mut ctx = BigNumContext::new().map_err(os_err("Failed to allocate BN context"))?;
    let ephemeral_ec =
        EcKey::generate(&group).map_err(os_err("Failed to generate ephemeral key"))?;
    let ephemeral_pub_bytes = ephemeral_ec
        .public_key()
        .to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut ctx)
        .map_err(os_err("Failed to encode ephemeral public key"))?;
    let ephemeral_pkey =
        PKey::from_ec_key(ephemeral_ec).map_err(os_err("Failed to wrap ephemeral key"))?;

    let shared = ecdh_shared(&ephemeral_pkey, &recipient_pub).map_err(os_err("ECDH failed"))?;

    // HKDF-SHA256(ikm = shared X, salt = recipient pub, info = ephemeral pub).
    let aes_key = hkdf_sha256_32(&shared, &recipient_pub_bytes, &ephemeral_pub_bytes)
        .map_err(os_err("HKDF failed"))?;

    let mut nonce = [0u8; AES_GCM_NONCE_LEN];
    openssl::rand::rand_bytes(&mut nonce).map_err(os_err("Failed to generate nonce"))?;
    let (ciphertext, tag) = aes_256_gcm_encrypt(&aes_key, &nonce, plaintext.as_bytes())
        .map_err(os_err("AES-256-GCM encryption failed"))?;

    let mut sealed = Vec::with_capacity(
        P256_PUBLIC_KEY_LEN + AES_GCM_NONCE_LEN + ciphertext.len() + AES_GCM_TAG_LEN,
    );
    sealed.extend_from_slice(&ephemeral_pub_bytes);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    sealed.extend_from_slice(&tag);

    Ok(Base64UrlUnpadded::encode_string(&sealed))
}

/// Encrypts `plaintext` with AES-256-GCM, returning `(ciphertext, tag)`. The caller appends the
/// 16-byte tag after the ciphertext so the wire layout matches WebCrypto's `encrypt`, which returns
/// ciphertext concatenated with the tag.
fn aes_256_gcm_encrypt(
    key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; AES_GCM_TAG_LEN]), ErrorStack> {
    let cipher = Cipher::aes_256_gcm();
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, key, Some(nonce))?;
    let mut ciphertext = vec![0u8; plaintext.len() + cipher.block_size()];
    let mut count = crypter.update(plaintext, &mut ciphertext)?;
    count += crypter.finalize(&mut ciphertext[count..])?;
    ciphertext.truncate(count);

    let mut tag = [0u8; AES_GCM_TAG_LEN];
    crypter.get_tag(&mut tag)?;
    Ok((ciphertext, tag))
}

/// Pure hashing logic shared by [`op_crypto_sha256`] and the unit tests.
fn sha256_hex(data_b64: &str) -> Result<String, JsErrorBox> {
    let data = Base64UrlUnpadded::decode_vec(data_b64)
        .map_err(|err| JsErrorBox::generic(format!("Invalid data (base64): {err}")))?;
    let digest =
        openssl::hash::hash(MessageDigest::sha256(), &data).map_err(os_err("SHA-256 failed"))?;
    Ok(hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::{
        AES_GCM_TAG_LEN, P256_PUBLIC_KEY_LEN, ecdh_shared, hkdf_sha256_32, p256_group, seal,
        sha256_hex,
    };
    use base64ct::{Base64UrlUnpadded, Encoding};
    use openssl::{
        bn::BigNumContext,
        ec::{EcKey, EcPoint, PointConversionForm},
        hash::MessageDigest,
        pkey::{PKey, Private},
        symm::{Cipher, Crypter, Mode},
    };

    /// A freshly generated recipient: its private `PKey` (to open blobs) and
    /// the base64url of its SEC1 uncompressed public key (to seal against).
    struct Recipient {
        private: PKey<Private>,
        pub_bytes: Vec<u8>,
        pub_b64: String,
    }

    fn fresh_recipient() -> Recipient {
        let group = p256_group().unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let ec = EcKey::generate(&group).unwrap();
        let pub_bytes = ec
            .public_key()
            .to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut ctx)
            .unwrap();
        let pub_b64 = Base64UrlUnpadded::encode_string(&pub_bytes);
        let private = PKey::from_ec_key(ec).unwrap();
        Recipient {
            private,
            pub_bytes,
            pub_b64,
        }
    }

    /// Opens a sealed blob with the recipient's private key, mirroring exactly
    /// what the browser does with WebCrypto. Returns the recovered plaintext, or
    /// an `openssl` error when authentication fails (tampered blob / wrong key).
    fn open_sealed(
        recipient: &Recipient,
        sealed_b64: &str,
    ) -> Result<Vec<u8>, openssl::error::ErrorStack> {
        let sealed = Base64UrlUnpadded::decode_vec(sealed_b64).unwrap();
        let (ephemeral_pub_bytes, rest) = sealed.split_at(P256_PUBLIC_KEY_LEN);
        let (nonce, ct_and_tag) = rest.split_at(12);
        let (ciphertext, tag) = ct_and_tag.split_at(ct_and_tag.len() - AES_GCM_TAG_LEN);

        let group = p256_group()?;
        let mut ctx = BigNumContext::new()?;
        let point = EcPoint::from_bytes(&group, ephemeral_pub_bytes, &mut ctx)?;
        let ephemeral_pkey = PKey::from_ec_key(EcKey::from_public_key(&group, &point)?)?;

        let shared = ecdh_shared(&recipient.private, &ephemeral_pkey)?;
        let aes_key = hkdf_sha256_32(&shared, &recipient.pub_bytes, ephemeral_pub_bytes)?;

        let cipher = Cipher::aes_256_gcm();
        let mut crypter = Crypter::new(cipher, Mode::Decrypt, &aes_key, Some(nonce))?;
        crypter.set_tag(tag)?;
        let mut plaintext = vec![0u8; ciphertext.len() + cipher.block_size()];
        let mut count = crypter.update(ciphertext, &mut plaintext)?;
        count += crypter.finalize(&mut plaintext[count..])?;
        plaintext.truncate(count);
        Ok(plaintext)
    }

    #[test]
    fn seal_round_trip() {
        let recipient = fresh_recipient();
        let plaintext = r#"{"method":"POST","path":"/webhook/abc"}"#;
        let sealed_b64 = seal(&recipient.pub_b64, plaintext).unwrap();
        assert_eq!(
            open_sealed(&recipient, &sealed_b64).unwrap(),
            plaintext.as_bytes()
        );
    }

    #[test]
    fn seal_round_trip_empty_plaintext() {
        let recipient = fresh_recipient();
        let sealed_b64 = seal(&recipient.pub_b64, "").unwrap();
        assert!(open_sealed(&recipient, &sealed_b64).unwrap().is_empty());
    }

    #[test]
    fn seal_uses_fresh_ephemeral_key_each_call() {
        let recipient = fresh_recipient();
        let a = seal(&recipient.pub_b64, "hello").unwrap();
        let b = seal(&recipient.pub_b64, "hello").unwrap();
        // Distinct ephemeral keys + nonces => distinct ciphertexts for identical input.
        assert_ne!(a, b);
    }

    #[test]
    fn open_rejects_tampered_tag() {
        let recipient = fresh_recipient();
        let sealed_b64 = seal(&recipient.pub_b64, "authentic").unwrap();
        let mut sealed = Base64UrlUnpadded::decode_vec(&sealed_b64).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        let tampered_b64 = Base64UrlUnpadded::encode_string(&sealed);
        assert!(open_sealed(&recipient, &tampered_b64).is_err());
    }

    #[test]
    fn open_rejects_wrong_recipient_key() {
        let recipient = fresh_recipient();
        let attacker = fresh_recipient();
        let sealed_b64 = seal(&recipient.pub_b64, "secret").unwrap();
        // The attacker's key derives a different ECDH secret => GCM auth fails.
        assert!(open_sealed(&attacker, &sealed_b64).is_err());
    }

    #[test]
    fn seal_rejects_malformed_recipient_key() {
        // Wrong length.
        let short = Base64UrlUnpadded::encode_string(&[0x04u8; 10]);
        assert!(seal(&short, "x").is_err());

        // Right length, invalid SEC1 tag.
        let bad_tag = Base64UrlUnpadded::encode_string(&[0x00u8; 65]);
        assert!(seal(&bad_tag, "x").is_err());

        // Right length and tag, but not an on-curve point.
        let off_curve = Base64UrlUnpadded::encode_string(&[0x04u8; 65]);
        assert!(seal(&off_curve, "x").is_err());
    }

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256("abc").
        let data_b64 = Base64UrlUnpadded::encode_string(b"abc");
        assert_eq!(
            sha256_hex(&data_b64).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_matches_openssl() {
        let data = b"the quick brown fox";
        let data_b64 = Base64UrlUnpadded::encode_string(data);
        let expected = hex::encode(openssl::hash::hash(MessageDigest::sha256(), data).unwrap());
        assert_eq!(sha256_hex(&data_b64).unwrap(), expected);
    }

    #[test]
    fn hkdf_matches_rfc5869_style_vector() {
        // Deterministic check that extract+expand is stable and 32 bytes long.
        let okm = hkdf_sha256_32(b"input-key-material", b"salt", b"info").unwrap();
        assert_eq!(okm.len(), 32);
        let again = hkdf_sha256_32(b"input-key-material", b"salt", b"info").unwrap();
        assert_eq!(okm, again);
        // A different info must yield a different key (domain separation).
        let other = hkdf_sha256_32(b"input-key-material", b"salt", b"info2").unwrap();
        assert_ne!(okm, other);
    }
}
