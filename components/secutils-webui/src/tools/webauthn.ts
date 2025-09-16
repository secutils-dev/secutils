export function arrayBufferToSafeBase64Url(buffer: ArrayBuffer) {
  const array = new Uint8Array(buffer);

  let string = '';
  for (let i = 0; i < array.byteLength; i++) {
    string += String.fromCharCode(array[i]);
  }

  return btoa(string).replace(/\+/g, '-').replace(/\//g, '_').replace(/=*$/g, '');
}

export function safeBase64UrlToArrayBuffer(base64Url: string): ArrayBuffer {
  const base64 = atob(base64Url.replace(/-/g, '+').replace(/_/g, '/'));
  const bytes = new Uint8Array(base64.length);
  for (let i = 0; i < base64.length; i++) {
    bytes[i] = base64.charCodeAt(i);
  }

  return bytes.buffer;
}

export function isWebAuthnSupported() {
  return window.PublicKeyCredential !== undefined && typeof window.PublicKeyCredential === 'function';
}
