import type { CertificateAttributes, SignatureAlgorithm } from './certificate_attributes';
import type { PrivateKeyAlgorithm, PrivateKeyCurveName, PrivateKeySize } from './private_key_alg';

export interface ParsedCertificate {
  subjectCN?: string;
  subjectC?: string;
  subjectST?: string;
  subjectL?: string;
  subjectO?: string;
  subjectOU?: string;
  issuerCN?: string;
  issuerO?: string;
  issuerC?: string;
  serialNumber: string;
  notBefore: Date;
  notAfter: Date;
  isCa: boolean;
  keyAlgorithm?: PrivateKeyAlgorithm;
  keyAlgorithmName: string;
  signatureAlgorithm?: SignatureAlgorithm;
  signatureAlgorithmName: string;
  keyUsage: string[];
  extendedKeyUsage: string[];
  sha256Fingerprint: string;
  pem: string;
}

const PEM_CERT_REGEX = /-----BEGIN CERTIFICATE-----\s*([\s\S]*?)\s*-----END CERTIFICATE-----/g;

export function parsePemContent(input: string): ArrayBuffer[] {
  const results: ArrayBuffer[] = [];
  let match;
  PEM_CERT_REGEX.lastIndex = 0;

  while ((match = PEM_CERT_REGEX.exec(input)) !== null) {
    const base64 = match[1].replace(/\s/g, '');
    results.push(base64ToArrayBuffer(base64));
  }

  if (results.length > 0) {
    return results;
  }

  // No PEM headers found - try treating entire input as raw base64 DER.
  const cleaned = input.replace(/\s/g, '');
  if (cleaned.length === 0) {
    throw new Error('No certificate data found in the provided input.');
  }

  try {
    const buffer = base64ToArrayBuffer(cleaned);
    if (buffer.byteLength < 10) {
      throw new Error('Data too short to be a valid certificate.');
    }
    return [buffer];
  } catch {
    throw new Error(
      'Unable to parse the provided input. Please provide a valid PEM-encoded certificate (with or without -----BEGIN/END CERTIFICATE----- headers).',
    );
  }
}

function base64ToArrayBuffer(base64: string) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

function arrayBufferToHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer))
    .map((b) => b.toString(16).padStart(2, '0').toUpperCase())
    .join(':');
}

const KEY_USAGE_OID = '2.5.29.15';
const EXT_KEY_USAGE_OID = '2.5.29.37';
const BASIC_CONSTRAINTS_OID = '2.5.29.19';

// Key usage bit names matching the backend enum values.
const KEY_USAGE_BITS: Record<number, string> = {
  0: 'digitalSignature',
  1: 'nonRepudiation',
  2: 'keyEncipherment',
  3: 'dataEncipherment',
  4: 'keyAgreement',
  5: 'keyCertificateSigning',
  6: 'crlSigning',
  7: 'encipherOnly',
  8: 'decipherOnly',
};

// Extended key usage OID to backend enum value mapping.
const EXT_KEY_USAGE_MAP: Record<string, string> = {
  '1.3.6.1.5.5.7.3.1': 'tlsWebServerAuthentication',
  '1.3.6.1.5.5.7.3.2': 'tlsWebClientAuthentication',
  '1.3.6.1.5.5.7.3.3': 'codeSigning',
  '1.3.6.1.5.5.7.3.4': 'emailProtection',
  '1.3.6.1.5.5.7.3.8': 'timeStamping',
};

// Hash algorithm name (from Web Crypto API) to our backend SignatureAlgorithm.
const HASH_TO_SIG_ALG: Record<string, SignatureAlgorithm> = {
  MD5: 'md5',
  'SHA-1': 'sha1',
  'SHA-256': 'sha256',
  'SHA-384': 'sha384',
  'SHA-512': 'sha512',
};

function getDnField(dn: string, field: string) {
  const regex = new RegExp(`(?:^|,)\\s*${field}=([^,]*)`);
  const match = dn.match(regex);
  return match?.[1]?.trim() || undefined;
}

export async function parseCertificateFromDer(der: ArrayBuffer, pem: string): Promise<ParsedCertificate> {
  const x509 = await import('@peculiar/x509');
  const cert = new x509.X509Certificate(der);

  const subject = cert.subject;
  const issuer = cert.issuer;

  // Key algorithm detection.
  let keyAlgorithm: PrivateKeyAlgorithm | undefined;
  let keyAlgorithmName = 'Unknown';
  const pubKeyAlg = cert.publicKey.algorithm;

  if (pubKeyAlg.name === 'RSASSA-PKCS1-v1_5' || pubKeyAlg.name === 'RSA-PSS' || pubKeyAlg.name === 'RSA-OAEP') {
    const modulusLength = (pubKeyAlg as RsaHashedKeyAlgorithm).modulusLength;
    keyAlgorithmName = `RSA (${modulusLength} bits)`;
    const sizeStr = String(modulusLength) as PrivateKeySize;
    if (['1024', '2048', '4096', '8192'].includes(sizeStr)) {
      keyAlgorithm = { keyType: 'rsa', keySize: sizeStr };
    }
  } else if (pubKeyAlg.name === 'ECDSA' || pubKeyAlg.name === 'ECDH') {
    const namedCurve = (pubKeyAlg as EcKeyAlgorithm).namedCurve;
    keyAlgorithmName = `ECDSA (${namedCurve})`;
    const curveMap: Record<string, PrivateKeyCurveName> = {
      'P-256': 'secp256r1',
      'P-384': 'secp384r1',
      'P-521': 'secp521r1',
    };
    if (curveMap[namedCurve]) {
      keyAlgorithm = { keyType: 'ecdsa', curve: curveMap[namedCurve] };
    }
  } else if (pubKeyAlg.name === 'Ed25519' || pubKeyAlg.name === 'EdDSA') {
    keyAlgorithmName = 'Ed25519';
    keyAlgorithm = { keyType: 'ed25519' };
  } else if (pubKeyAlg.name === 'DSA') {
    keyAlgorithmName = 'DSA';
  }

  // Signature algorithm: use the Web Crypto `name` and `hash` from HashedAlgorithm.
  const sigAlg = cert.signatureAlgorithm;
  const sigAlgName = sigAlg.name ?? 'Unknown';
  const hashAlgName = (sigAlg.hash as Algorithm)?.name;

  let signatureAlgorithm: SignatureAlgorithm | undefined;
  let signatureAlgorithmName: string;
  if (sigAlgName === 'Ed25519' || sigAlgName === 'EdDSA') {
    signatureAlgorithm = 'ed25519';
    signatureAlgorithmName = 'Ed25519';
  } else if (hashAlgName) {
    signatureAlgorithm = HASH_TO_SIG_ALG[hashAlgName];
    signatureAlgorithmName = `${hashAlgName} with ${sigAlgName}`;
  } else {
    signatureAlgorithmName = sigAlgName;
  }

  // Basic constraints (CA).
  let isCa = false;
  const basicConstraints = cert.getExtension(BASIC_CONSTRAINTS_OID);
  if (basicConstraints) {
    const bcExt = new x509.BasicConstraintsExtension(basicConstraints.rawData);
    isCa = bcExt.ca;
  }

  // Key usage.
  const keyUsage: string[] = [];
  const kuExt = cert.getExtension(KEY_USAGE_OID);
  if (kuExt) {
    const kuExtObj = new x509.KeyUsagesExtension(kuExt.rawData);
    const usages = kuExtObj.usages;
    for (const [bit, name] of Object.entries(KEY_USAGE_BITS)) {
      if (usages & (1 << Number(bit))) {
        keyUsage.push(name);
      }
    }
  }

  // Extended key usage.
  const extendedKeyUsage: string[] = [];
  const ekuExt = cert.getExtension(EXT_KEY_USAGE_OID);
  if (ekuExt) {
    const ekuExtObj = new x509.ExtendedKeyUsageExtension(ekuExt.rawData);
    for (const oid of ekuExtObj.usages) {
      const mapped = EXT_KEY_USAGE_MAP[String(oid)];
      if (mapped) {
        extendedKeyUsage.push(mapped);
      }
    }
  }

  // SHA-256 fingerprint.
  const thumbprint = await cert.getThumbprint('SHA-256');
  const sha256Fingerprint = arrayBufferToHex(thumbprint);

  return {
    subjectCN: getDnField(subject, 'CN'),
    subjectC: getDnField(subject, 'C'),
    subjectST: getDnField(subject, 'ST'),
    subjectL: getDnField(subject, 'L'),
    subjectO: getDnField(subject, 'O'),
    subjectOU: getDnField(subject, 'OU'),
    issuerCN: getDnField(issuer, 'CN'),
    issuerO: getDnField(issuer, 'O'),
    issuerC: getDnField(issuer, 'C'),
    serialNumber: cert.serialNumber,
    notBefore: cert.notBefore,
    notAfter: cert.notAfter,
    isCa,
    keyAlgorithm,
    keyAlgorithmName,
    signatureAlgorithm,
    signatureAlgorithmName,
    keyUsage,
    extendedKeyUsage,
    sha256Fingerprint,
    pem,
  };
}

export function certificateToTemplateAttributes(cert: ParsedCertificate): CertificateAttributes | null {
  if (!cert.keyAlgorithm || !cert.signatureAlgorithm) {
    return null;
  }

  return {
    commonName: cert.subjectCN,
    country: cert.subjectC,
    stateOrProvince: cert.subjectST,
    locality: cert.subjectL,
    organization: cert.subjectO,
    organizationalUnit: cert.subjectOU,
    keyAlgorithm: cert.keyAlgorithm,
    signatureAlgorithm: cert.signatureAlgorithm,
    notValidBefore: Math.floor(cert.notBefore.getTime() / 1000),
    notValidAfter: Math.floor(cert.notAfter.getTime() / 1000),
    isCa: cert.isCa,
    keyUsage: cert.keyUsage.length > 0 ? cert.keyUsage : undefined,
    extendedKeyUsage: cert.extendedKeyUsage.length > 0 ? cert.extendedKeyUsage : undefined,
  };
}

export function getDefaultCertificateName(cert: ParsedCertificate, index: number): string {
  return cert.subjectCN || cert.subjectO || `Certificate ${index + 1}`;
}
