// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  SerializedPublicKeyCredentialCreationOptions,
  SerializedPublicKeyCredentialRequestOptions,
} from './webauthn';
import { serializeRegisterCredential, signinWithPasskey, signupWithPasskey } from './webauthn';
import { arrayBufferToSafeBase64Url } from '../tools/webauthn';

function toArrayBuffer(data: number[]): ArrayBuffer {
  return new Uint8Array(data).buffer;
}

function mockAttestationCredential(overrides?: {
  transports?: string[];
  noGetTransports?: boolean;
}): PublicKeyCredential {
  const response: Partial<AuthenticatorAttestationResponse> = {
    attestationObject: toArrayBuffer([1, 2, 3]),
    clientDataJSON: toArrayBuffer([4, 5, 6]),
  };
  if (!overrides?.noGetTransports) {
    (response as AuthenticatorAttestationResponse).getTransports = () => overrides?.transports ?? ['internal'];
  }

  return {
    id: 'cred-id-1',
    rawId: toArrayBuffer([10, 20, 30]),
    type: 'public-key',
    authenticatorAttachment: null,
    getClientExtensionResults: () => ({}),
    response: response as AuthenticatorAttestationResponse,
  } as unknown as PublicKeyCredential;
}

function mockAssertionCredential(): PublicKeyCredential {
  const response: Partial<AuthenticatorAssertionResponse> = {
    authenticatorData: toArrayBuffer([7, 8, 9]),
    clientDataJSON: toArrayBuffer([4, 5, 6]),
    signature: toArrayBuffer([11, 12, 13]),
    userHandle: toArrayBuffer([14, 15]),
  };

  return {
    id: 'cred-id-2',
    rawId: toArrayBuffer([40, 50, 60]),
    type: 'public-key',
    authenticatorAttachment: null,
    getClientExtensionResults: () => ({ appid: true }),
    response: response as AuthenticatorAssertionResponse,
  } as unknown as PublicKeyCredential;
}

describe('serializeRegisterCredential', () => {
  it('serializes all credential fields to base64url strings', () => {
    const credential = mockAttestationCredential();
    const serialized = serializeRegisterCredential(credential);

    expect(serialized.id).toBe('cred-id-1');
    expect(serialized.type).toBe('public-key');
    expect(serialized.extensions).toEqual({});

    expect(serialized.rawId).toBe(arrayBufferToSafeBase64Url(toArrayBuffer([10, 20, 30])));
    expect(serialized.response.attestationObject).toBe(arrayBufferToSafeBase64Url(toArrayBuffer([1, 2, 3])));
    expect(serialized.response.clientDataJSON).toBe(arrayBufferToSafeBase64Url(toArrayBuffer([4, 5, 6])));
    expect(serialized.response.transports).toEqual(['internal']);
  });

  it('sets transports to undefined when getTransports is not available', () => {
    const credential = mockAttestationCredential({ noGetTransports: true });
    const serialized = serializeRegisterCredential(credential);
    expect(serialized.response.transports).toBeUndefined();
  });
});

describe('signupWithPasskey', () => {
  let mockCreate: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockCreate = vi.fn();
    vi.stubGlobal('navigator', { credentials: { create: mockCreate, get: vi.fn() } });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('deserializes options, calls navigator.credentials.create, and returns serialized JSON', async () => {
    const credential = mockAttestationCredential();
    mockCreate.mockResolvedValueOnce(credential);

    const challenge = arrayBufferToSafeBase64Url(toArrayBuffer([99, 100]));
    const userId = arrayBufferToSafeBase64Url(toArrayBuffer([1]));

    const options: SerializedPublicKeyCredentialCreationOptions = {
      rp: { name: 'Test RP' },
      user: { id: userId, name: 'user@test.com', displayName: 'Test User' },
      challenge,
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
    };

    const result = await signupWithPasskey(options);
    const parsed = JSON.parse(result);

    expect(parsed.id).toBe('cred-id-1');
    expect(parsed.type).toBe('public-key');
    expect(parsed.response.attestationObject).toBeDefined();
    expect(parsed.response.clientDataJSON).toBeDefined();

    const createArg = mockCreate.mock.calls[0][0];
    expect(createArg.publicKey.challenge).toBeInstanceOf(ArrayBuffer);
    expect(createArg.publicKey.user.id).toBeInstanceOf(ArrayBuffer);
    expect(createArg.publicKey.rp.name).toBe('Test RP');
  });

  it('deserializes excludeCredentials when provided', async () => {
    const credential = mockAttestationCredential();
    mockCreate.mockResolvedValueOnce(credential);

    const excludeId = arrayBufferToSafeBase64Url(toArrayBuffer([77]));
    const options: SerializedPublicKeyCredentialCreationOptions = {
      rp: { name: 'RP' },
      user: { id: arrayBufferToSafeBase64Url(toArrayBuffer([1])), name: 'u', displayName: 'U' },
      challenge: arrayBufferToSafeBase64Url(toArrayBuffer([2])),
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
      excludeCredentials: [{ id: excludeId, type: 'public-key' }],
    };

    await signupWithPasskey(options);

    const createArg = mockCreate.mock.calls[0][0];
    expect(createArg.publicKey.excludeCredentials).toHaveLength(1);
    expect(createArg.publicKey.excludeCredentials[0].id).toBeInstanceOf(ArrayBuffer);
  });

  it('throws when navigator.credentials.create returns null', async () => {
    mockCreate.mockResolvedValueOnce(null);

    const options: SerializedPublicKeyCredentialCreationOptions = {
      rp: { name: 'RP' },
      user: { id: arrayBufferToSafeBase64Url(toArrayBuffer([1])), name: 'u', displayName: 'U' },
      challenge: arrayBufferToSafeBase64Url(toArrayBuffer([2])),
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
    };

    await expect(signupWithPasskey(options)).rejects.toThrow('Browser could not create credentials.');
  });
});

describe('signinWithPasskey', () => {
  let mockGet: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockGet = vi.fn();
    vi.stubGlobal('navigator', { credentials: { create: vi.fn(), get: mockGet } });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('deserializes options, calls navigator.credentials.get, and returns serialized JSON', async () => {
    const credential = mockAssertionCredential();
    mockGet.mockResolvedValueOnce(credential);

    const options: SerializedPublicKeyCredentialRequestOptions = {
      challenge: arrayBufferToSafeBase64Url(toArrayBuffer([88, 89])),
    };

    const result = await signinWithPasskey(options);
    const parsed = JSON.parse(result);

    expect(parsed.id).toBe('cred-id-2');
    expect(parsed.type).toBe('public-key');
    expect(parsed.extensions).toEqual({ appid: true });
    expect(parsed.response.authenticatorData).toBeDefined();
    expect(parsed.response.clientDataJSON).toBeDefined();
    expect(parsed.response.signature).toBeDefined();
    expect(parsed.response.userHandle).toBeDefined();

    const getArg = mockGet.mock.calls[0][0];
    expect(getArg.publicKey.challenge).toBeInstanceOf(ArrayBuffer);
  });

  it('deserializes allowCredentials when provided', async () => {
    const credential = mockAssertionCredential();
    mockGet.mockResolvedValueOnce(credential);

    const allowId = arrayBufferToSafeBase64Url(toArrayBuffer([33]));
    const options: SerializedPublicKeyCredentialRequestOptions = {
      challenge: arrayBufferToSafeBase64Url(toArrayBuffer([1])),
      allowCredentials: [{ id: allowId, type: 'public-key' }],
    };

    await signinWithPasskey(options);

    const getArg = mockGet.mock.calls[0][0];
    expect(getArg.publicKey.allowCredentials).toHaveLength(1);
    expect(getArg.publicKey.allowCredentials[0].id).toBeInstanceOf(ArrayBuffer);
  });

  it('throws when navigator.credentials.get returns null', async () => {
    mockGet.mockResolvedValueOnce(null);

    const options: SerializedPublicKeyCredentialRequestOptions = {
      challenge: arrayBufferToSafeBase64Url(toArrayBuffer([1])),
    };

    await expect(signinWithPasskey(options)).rejects.toThrow('Browser could not get credentials.');
  });

  it('handles assertion credential without userHandle', async () => {
    const response: Partial<AuthenticatorAssertionResponse> = {
      authenticatorData: toArrayBuffer([7, 8, 9]),
      clientDataJSON: toArrayBuffer([4, 5, 6]),
      signature: toArrayBuffer([11, 12, 13]),
      userHandle: null,
    };
    const credential = {
      id: 'cred-no-handle',
      rawId: toArrayBuffer([40, 50]),
      type: 'public-key',
      authenticatorAttachment: null,
      getClientExtensionResults: () => ({}),
      response: response as AuthenticatorAssertionResponse,
    } as unknown as PublicKeyCredential;
    mockGet.mockResolvedValueOnce(credential);

    const result = await signinWithPasskey({
      challenge: arrayBufferToSafeBase64Url(toArrayBuffer([1])),
    });
    const parsed = JSON.parse(result);
    expect(parsed.response.userHandle).toBeUndefined();
  });
});
