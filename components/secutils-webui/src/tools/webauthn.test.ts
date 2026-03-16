// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { arrayBufferToSafeBase64Url, isWebAuthnSupported, safeBase64UrlToArrayBuffer } from './webauthn';

function toArrayBuffer(data: number[]): ArrayBuffer {
  return new Uint8Array(data).buffer;
}

function toByteArray(buf: ArrayBuffer): number[] {
  return Array.from(new Uint8Array(buf));
}

describe('arrayBufferToSafeBase64Url', () => {
  it('encodes an empty buffer', () => {
    expect(arrayBufferToSafeBase64Url(toArrayBuffer([]))).toBe('');
  });

  it('encodes a simple buffer', () => {
    const encoded = arrayBufferToSafeBase64Url(toArrayBuffer([72, 101, 108, 108, 111]));
    expect(encoded).toBe('SGVsbG8');
  });

  it('replaces + with - and / with _', () => {
    // [251, 255, 254] in standard base64 is "u//+" - after URL-safe replacement: "u__-"
    const encoded = arrayBufferToSafeBase64Url(toArrayBuffer([0xbb, 0xff, 0xfe]));
    expect(encoded).not.toContain('+');
    expect(encoded).not.toContain('/');
  });

  it('strips trailing = padding', () => {
    // Single byte encodes to 2 base64 chars + "==" padding
    const encoded = arrayBufferToSafeBase64Url(toArrayBuffer([65]));
    expect(encoded).not.toContain('=');
    expect(encoded).toBe('QQ');
  });
});

describe('safeBase64UrlToArrayBuffer', () => {
  it('decodes an empty string', () => {
    expect(toByteArray(safeBase64UrlToArrayBuffer(''))).toEqual([]);
  });

  it('decodes a simple base64url string', () => {
    const bytes = toByteArray(safeBase64UrlToArrayBuffer('SGVsbG8'));
    expect(bytes).toEqual([72, 101, 108, 108, 111]);
  });

  it('handles URL-safe characters (- and _)', () => {
    const original = [0xbb, 0xff, 0xfe];
    const encoded = arrayBufferToSafeBase64Url(toArrayBuffer(original));
    const decoded = toByteArray(safeBase64UrlToArrayBuffer(encoded));
    expect(decoded).toEqual(original);
  });
});

describe('roundtrip', () => {
  it('preserves data through encode → decode', () => {
    const inputs = [[], [0], [255], [1, 2, 3], [0, 0, 0, 0], Array.from({ length: 256 }, (_, i) => i)];

    for (const input of inputs) {
      const encoded = arrayBufferToSafeBase64Url(toArrayBuffer(input));
      const decoded = toByteArray(safeBase64UrlToArrayBuffer(encoded));
      expect(decoded).toEqual(input);
    }
  });
});

describe('isWebAuthnSupported', () => {
  it('returns false when PublicKeyCredential is undefined', () => {
    expect(isWebAuthnSupported()).toBe(false);
  });

  it('returns true when PublicKeyCredential is a function', () => {
    const original = window.PublicKeyCredential;
    try {
      Object.defineProperty(window, 'PublicKeyCredential', {
        value: function MockPublicKeyCredential() {},
        writable: true,
        configurable: true,
      });
      expect(isWebAuthnSupported()).toBe(true);
    } finally {
      Object.defineProperty(window, 'PublicKeyCredential', {
        value: original,
        writable: true,
        configurable: true,
      });
    }
  });
});
