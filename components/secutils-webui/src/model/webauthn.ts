import { arrayBufferToSafeBase64Url, safeBase64UrlToArrayBuffer } from '../tools/webauthn';

interface SerializedPublicKeyCredentialDescriptor extends Omit<PublicKeyCredentialDescriptor, 'id'> {
  id: string;
}

interface SerializedPublicKeyCredentialUserEntity extends Omit<PublicKeyCredentialUserEntity, 'id'> {
  id: string;
}

export interface SerializedPublicKeyCredentialCreationOptions extends Omit<
  PublicKeyCredentialCreationOptions,
  'challenge' | 'excludeCredentials' | 'user'
> {
  challenge: string;
  excludeCredentials?: SerializedPublicKeyCredentialDescriptor[];
  user: SerializedPublicKeyCredentialUserEntity;
}

export interface SerializedPublicKeyCredentialRequestOptions extends Omit<
  PublicKeyCredentialRequestOptions,
  'challenge' | 'allowCredentials'
> {
  challenge: string;
  allowCredentials?: SerializedPublicKeyCredentialDescriptor[];
}

interface SerializedRegisterPublicKeyCredential {
  id: string;
  rawId: string;
  type: string;
  extensions: AuthenticationExtensionsClientOutputs;
  response: {
    attestationObject: string;
    clientDataJSON: string;
    transports?: string[];
  };
}

interface SerializedPublicKeyCredential {
  id: string;
  rawId: string;
  type: string;
  extensions: AuthenticationExtensionsClientOutputs;
  response: {
    authenticatorData: string;
    clientDataJSON: string;
    signature: string;
    userHandle?: string;
  };
}

function deserializePublicKeyCredentialCreationOptions(
  serializedPublicKey: SerializedPublicKeyCredentialCreationOptions,
): PublicKeyCredentialCreationOptions {
  return {
    ...serializedPublicKey,
    challenge: safeBase64UrlToArrayBuffer(serializedPublicKey.challenge),
    excludeCredentials: serializedPublicKey.excludeCredentials
      ? serializedPublicKey.excludeCredentials.map((serializedCredential) => ({
          ...serializedCredential,
          id: safeBase64UrlToArrayBuffer(serializedCredential.id),
        }))
      : undefined,
    user: {
      ...serializedPublicKey.user,
      id: safeBase64UrlToArrayBuffer(serializedPublicKey.user.id),
    },
  };
}

function deserializeCredentialRequestOptions(
  serializedPublicKey: SerializedPublicKeyCredentialRequestOptions,
): PublicKeyCredentialRequestOptions {
  return {
    ...serializedPublicKey,
    challenge: safeBase64UrlToArrayBuffer(serializedPublicKey.challenge),
    allowCredentials: serializedPublicKey.allowCredentials
      ? serializedPublicKey.allowCredentials.map((serializedCredential) => ({
          ...serializedCredential,
          id: safeBase64UrlToArrayBuffer(serializedCredential.id),
        }))
      : undefined,
  };
}

export function serializeRegisterCredential(credential: PublicKeyCredential): SerializedRegisterPublicKeyCredential {
  const attestationResponse = credential.response as AuthenticatorAttestationResponse;

  return {
    id: credential.id,
    rawId: arrayBufferToSafeBase64Url(credential.rawId),
    type: credential.type,
    extensions: credential.getClientExtensionResults(),
    response: {
      attestationObject: arrayBufferToSafeBase64Url(attestationResponse.attestationObject),
      clientDataJSON: arrayBufferToSafeBase64Url(attestationResponse.clientDataJSON),
      transports:
        typeof attestationResponse.getTransports === 'function' ? attestationResponse.getTransports() : undefined,
    },
  };
}

function serializeCredential(credential: PublicKeyCredential): SerializedPublicKeyCredential {
  const assertionResponse = credential.response as AuthenticatorAssertionResponse;

  return {
    id: credential.id,
    rawId: arrayBufferToSafeBase64Url(credential.rawId),
    type: credential.type,
    extensions: credential.getClientExtensionResults(),
    response: {
      authenticatorData: arrayBufferToSafeBase64Url(assertionResponse.authenticatorData),
      clientDataJSON: arrayBufferToSafeBase64Url(assertionResponse.clientDataJSON),
      signature: arrayBufferToSafeBase64Url(assertionResponse.signature),
      userHandle: assertionResponse.userHandle ? arrayBufferToSafeBase64Url(assertionResponse.userHandle) : undefined,
    },
  };
}

export async function signupWithPasskey(publicKey: SerializedPublicKeyCredentialCreationOptions) {
  // Then, ask browser to create credentials.
  const credentials = await navigator.credentials.create({
    publicKey: deserializePublicKeyCredentialCreationOptions(publicKey),
  });
  if (!credentials) {
    throw new Error('Browser could not create credentials.');
  }

  return JSON.stringify(serializeRegisterCredential(credentials as PublicKeyCredential));
}

export async function signinWithPasskey(publicKey: SerializedPublicKeyCredentialRequestOptions) {
  // Then, ask browser to create credentials.
  const credentials = await navigator.credentials.get({ publicKey: deserializeCredentialRequestOptions(publicKey) });
  if (!credentials) {
    throw new Error('Browser could not get credentials.');
  }

  return JSON.stringify(serializeCredential(credentials as PublicKeyCredential));
}
