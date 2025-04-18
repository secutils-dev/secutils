import type { PrivateKeyAlgorithm } from './private_key_alg';

export type SignatureAlgorithm = 'ed25519' | 'md5' | 'sha1' | 'sha256' | 'sha384' | 'sha512';

export interface CertificateAttributes {
  commonName?: string;
  country?: string;
  stateOrProvince?: string;
  locality?: string;
  organization?: string;
  organizationalUnit?: string;
  keyAlgorithm: PrivateKeyAlgorithm;
  signatureAlgorithm: SignatureAlgorithm;
  notValidBefore: number;
  notValidAfter: number;
  isCa: boolean;
  keyUsage?: string[];
  extendedKeyUsage?: string[];
}

export function getDistinguishedNameString(attributes: CertificateAttributes) {
  return [
    attributes.country ? [`C=${attributes.country}`] : [],
    attributes.stateOrProvince ? [`ST=${attributes.stateOrProvince}`] : [],
    attributes.locality ? [`L=${attributes.locality}`] : [],
    attributes.organization ? [`O=${attributes.organization}`] : [],
    attributes.organizationalUnit ? [`OU=${attributes.organizationalUnit}`] : [],
    attributes.commonName ? [`CN=${attributes.commonName}`] : [],
  ]
    .flat()
    .join(',');
}

export function certificateTypeString(attributes: CertificateAttributes) {
  if (attributes.isCa) {
    return 'Certification Authority';
  }

  return 'End Entity';
}

export function signatureAlgorithmString(attributes: CertificateAttributes) {
  switch (attributes.signatureAlgorithm) {
    case 'md5':
      return attributes.signatureAlgorithm.toUpperCase();
    case 'sha1':
    case 'sha256':
    case 'sha384':
    case 'sha512':
      return attributes.signatureAlgorithm.replace('sha', 'sha-').toUpperCase();
    default:
      return 'Ed25519';
  }
}
