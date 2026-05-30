/**
 * Wire-format parity test for the webhook end-to-end-encryption sealed box.
 *
 * The server seals captured requests in Rust (`src/js_runtime/op_crypto_seal.rs`)
 * and the browser opens them with WebCrypto (`decryptSealed` in
 * `dev/tools/webhook.html`). Both sides must agree on one byte layout:
 *
 *   | 65B ephemeral SEC1 pubkey | 12B AES-GCM IV | N B ciphertext+tag |
 *
 *   shared = ECDH(ephemeral_priv, recipient_pub)            (32-byte X)
 *   aesKey = HKDF-SHA256(ikm=shared, salt=recipient_pub, info=ephemeral_pub)
 *   ct     = AES-256-GCM(aesKey, iv, plaintext)             (no AAD)
 *
 * This test re-implements *both* roles with native Node WebCrypto - `seal`
 * exactly as the Rust op documents it, and `decryptSealed` exactly as the
 * browser does - and round-trips a payload through them. A drift on either
 * side (curve, HKDF salt/info, segment offsets) breaks the round-trip, which is
 * the same failure a real Rust-sealed / browser-opened pair would exhibit.
 *
 * Run:
 *   cd e2e && npx playwright test --config=playwright.standalone.config.ts
 */
import { expect, test } from '@playwright/test';

const subtle = globalThis.crypto.subtle;

function b64url(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64url');
}
function fromB64url(s: string): Uint8Array<ArrayBuffer> {
  // Copy into a fresh, ArrayBuffer-backed view. A `Uint8Array` wrapping a Node `Buffer` is typed
  // `Uint8Array<ArrayBufferLike>`, which WebCrypto's `BufferSource` parameters (typed
  // `ArrayBufferView<ArrayBuffer>` since TS 5.7) reject.
  const decoded = Buffer.from(s, 'base64url');
  const out = new Uint8Array(decoded.byteLength);
  out.set(decoded);
  return out;
}

/** Server side: mirrors `seal()` in src/js_runtime/op_crypto_seal.rs. */
async function seal(recipientPubRawB64: string, plaintext: string): Promise<string> {
  const recipientPubRaw = fromB64url(recipientPubRawB64);
  const recipientPub = await subtle.importKey('raw', recipientPubRaw, { name: 'ECDH', namedCurve: 'P-256' }, false, []);

  const ephemeral = await subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']);
  const ephPubRaw = new Uint8Array(await subtle.exportKey('raw', ephemeral.publicKey));

  const shared = await subtle.deriveBits({ name: 'ECDH', public: recipientPub }, ephemeral.privateKey, 256);
  const hkdfKey = await subtle.importKey('raw', shared, 'HKDF', false, ['deriveBits']);
  const aesBits = await subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt: recipientPubRaw, info: ephPubRaw },
    hkdfKey,
    256,
  );
  const aesKey = await subtle.importKey('raw', aesBits, { name: 'AES-GCM' }, false, ['encrypt']);

  const iv = globalThis.crypto.getRandomValues(new Uint8Array(12));
  const ct = new Uint8Array(await subtle.encrypt({ name: 'AES-GCM', iv }, aesKey, new TextEncoder().encode(plaintext)));

  const sealed = new Uint8Array(65 + 12 + ct.length);
  sealed.set(ephPubRaw, 0);
  sealed.set(iv, 65);
  sealed.set(ct, 77);
  return b64url(sealed);
}

/** Browser side: mirrors `decryptSealed()` in dev/tools/webhook.html. */
async function decryptSealed(privJwk: JsonWebKey, myPubRawB64: string, sealedB64: string): Promise<string> {
  const sealed = fromB64url(sealedB64);
  if (sealed.length < 65 + 12 + 16) {
    throw new Error('sealed blob too short');
  }
  const ephPubRaw = sealed.subarray(0, 65);
  const iv = sealed.subarray(65, 77);
  const ct = sealed.subarray(77);
  const myPubRaw = fromB64url(myPubRawB64);

  const privKey = await subtle.importKey('jwk', privJwk, { name: 'ECDH', namedCurve: 'P-256' }, false, ['deriveBits']);
  const ephPub = await subtle.importKey('raw', ephPubRaw, { name: 'ECDH', namedCurve: 'P-256' }, false, []);
  const shared = await subtle.deriveBits({ name: 'ECDH', public: ephPub }, privKey, 256);

  const hkdfKey = await subtle.importKey('raw', shared, 'HKDF', false, ['deriveBits']);
  const aesBits = await subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt: myPubRaw, info: ephPubRaw },
    hkdfKey,
    256,
  );
  const aesKey = await subtle.importKey('raw', aesBits, { name: 'AES-GCM' }, false, ['decrypt']);
  const plain = await subtle.decrypt({ name: 'AES-GCM', iv }, aesKey, ct);
  return new TextDecoder().decode(new Uint8Array(plain));
}

async function freshRecipient(): Promise<{ jwk: JsonWebKey; pubRawB64: string }> {
  const pair = await subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']);
  const jwk = await subtle.exportKey('jwk', pair.privateKey);
  const pubRaw = new Uint8Array(await subtle.exportKey('raw', pair.publicKey));
  return { jwk, pubRawB64: b64url(pubRaw) };
}

test.describe('webhook E2EE sealed-box wire parity', () => {
  test('a server-sealed payload is recovered byte-for-byte by the browser routine', async () => {
    const { jwk, pubRawB64 } = await freshRecipient();
    const payload = JSON.stringify({ method: 'POST', path: '/webhook/abc', body: 'hello \u00e9 \u4e16\u754c' });

    const sealed = await seal(pubRawB64, payload);
    const opened = await decryptSealed(jwk, pubRawB64, sealed);

    expect(opened).toBe(payload);
  });

  test('the sealed blob has the documented 65|12|N layout', async () => {
    const { pubRawB64 } = await freshRecipient();
    const sealed = fromB64url(await seal(pubRawB64, 'x'));
    // 65 (eph pubkey) + 12 (iv) + 1 (plaintext) + 16 (GCM tag) = 94.
    expect(sealed.length).toBe(94);
    // SEC1 uncompressed prefix.
    expect(sealed[0]).toBe(0x04);
  });

  test('a fresh ephemeral key per call yields distinct ciphertexts', async () => {
    const { pubRawB64 } = await freshRecipient();
    const a = await seal(pubRawB64, 'same');
    const b = await seal(pubRawB64, 'same');
    expect(a).not.toBe(b);
  });

  test('GCM authentication rejects a tampered ciphertext', async () => {
    const { jwk, pubRawB64 } = await freshRecipient();
    const sealed = fromB64url(await seal(pubRawB64, 'authentic'));
    sealed[sealed.length - 1] ^= 0x01; // flip a tag bit
    await expect(decryptSealed(jwk, pubRawB64, b64url(sealed))).rejects.toThrow();
  });

  test('the wrong recipient key cannot open the blob', async () => {
    const recipient = await freshRecipient();
    const attacker = await freshRecipient();
    const sealed = await seal(recipient.pubRawB64, 'secret');
    // Attacker's private key with their own pub as HKDF salt -> ECDH mismatch.
    await expect(decryptSealed(attacker.jwk, attacker.pubRawB64, sealed)).rejects.toThrow();
  });
});
