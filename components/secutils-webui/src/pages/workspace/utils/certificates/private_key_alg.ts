export type PrivateKeyAlgorithm =
  | { keyType: 'ed25519' }
  | { keyType: 'rsa' | 'dsa'; keySize: PrivateKeySize }
  | { keyType: 'ecdsa'; curve: PrivateKeyCurveName };

export type PrivateKeySize = '1024' | '2048' | '4096' | '8192';
export type PrivateKeyCurveName = 'secp256r1' | 'secp384r1' | 'secp521r1';

export function privateKeyAlgString(alg: PrivateKeyAlgorithm) {
  switch (alg.keyType) {
    case 'rsa':
    case 'dsa':
      return `${alg.keyType.toUpperCase()} (${alg.keySize} bits)`;
    case 'ecdsa':
      return `ECDSA (${privateKeyCurveNameString(alg.curve)})`;
    default:
      return 'Ed25519 (256 bits)';
  }
}

export function privateKeyCurveNameString(curve: PrivateKeyCurveName) {
  switch (curve) {
    case 'secp256r1':
      return 'prime256v1 / secp256r1 / NIST P-256';
    case 'secp384r1':
      return 'secp384r1 / NIST P-384';
    case 'secp521r1':
      return 'secp521r1 / NIST P-521';
    default:
      return curve;
  }
}
