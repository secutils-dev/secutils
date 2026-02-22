import { describe, expect, it } from 'vitest';

import {
  certificateToTemplateAttributes,
  getDefaultCertificateName,
  parseCertificateFromDer,
  parsePemContent,
} from './certificate_import_utils';

// Wildcard cert with escaped comma in O (RSA 2048, SHA-256, CA:FALSE, keyUsage + extKeyUsage).
const ESCAPED_COMMA_PEM = [
  '-----BEGIN CERTIFICATE-----',
  'MIID3zCCAsegAwIBAgIUeWjWQ1RbzdsM+QfaczM6ZV1pu6EwDQYJKoZIhvcNAQEL',
  'BQAwbDELMAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcM',
  'DVNhbiBGcmFuY2lzY28xFzAVBgNVBAoMDlNlY3V0aWxzLCBJbmMuMRcwFQYDVQQD',
  'DA4qLnNlY3V0aWxzLmRldjAeFw0yNjAyMjIyMTA4NThaFw0yNzAyMjIyMTA4NTha',
  'MGwxCzAJBgNVBAYTAlVTMRMwEQYDVQQIDApDYWxpZm9ybmlhMRYwFAYDVQQHDA1T',
  'YW4gRnJhbmNpc2NvMRcwFQYDVQQKDA5TZWN1dGlscywgSW5jLjEXMBUGA1UEAwwO',
  'Ki5zZWN1dGlscy5kZXYwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDH',
  'jMrrdIQxt3/3qQMPAhagxQkgyIPU/Uo+vPGq7pQ2KYG99jP2TO3aM1nekCWqGBXq',
  'dlsRmfU6jIRIFEIR8AR0KlDtRw/u687b8ZDOUSzqtYsCehNrLyeS/JmBj6dfrPvz',
  'gg0VB+TiuTnkOXDi5/yGLuCuxREC2waZjIHBC1lGH7GY72ZbjkTtGMh9kNlvA6uh',
  'bPOPnYj1b6xBx9qyzc7BtxJyUWiyrPvZtHvKKkDnQm7PYXP68/iJZiR6nrv1HQEl',
  'gS2aUexnuUBn1yKGC8zLKsOLECLSh+ZAhyIzIqBCXVgCHK8xR2Vpi0gBV4QevvBf',
  '9yFm0HkAtgPPrBp7GZu5AgMBAAGjeTB3MB0GA1UdDgQWBBTfqj49gXdx+12eXMJK',
  'Jt+ooRVzwzAfBgNVHSMEGDAWgBTfqj49gXdx+12eXMJKJt+ooRVzwzAJBgNVHRME',
  'AjAAMAsGA1UdDwQEAwIFoDAdBgNVHSUEFjAUBggrBgEFBQcDAQYIKwYBBQUHAwIw',
  'DQYJKoZIhvcNAQELBQADggEBAL9aryHlDW9OpgaRxr69YTWik5BSH8/Ot4JoqPgW',
  '3dkLjaQ50lQY92xugFZSPGs5QTVaBPndTxJHpxqVfibePD9ITVHGgIgeAoYTw29K',
  'g9vNpVcpRD4CTkt84yXBR4Ov5eZhAG/RnOrUfeArmoOrn/1NcBRuetpCjSHxV/nl',
  '5I0qbttIcIhguwUx82vY1+HWcxpdjUOO1RE3U9r0P5YXcmSpvSuZd+PgR8WyTkfv',
  'HyjbPN3kG2KEg53cWrkvRvSUfLGS77e9wzPRjJjSX3HATXHL98c7LNj3pa7b/Pir',
  '8woxMHAwoJcuzwr9n2fpbWmpZJhwWnuRGHUoJKI9FsB232s=',
  '-----END CERTIFICATE-----',
].join('\n');

// Simple self-signed test certificate (RSA 2048, SHA-256, no escaped characters in DN).
const SIMPLE_PEM = [
  '-----BEGIN CERTIFICATE-----',
  'MIIDsTCCApmgAwIBAgIUYWNwS/Zjq9Dg3k7p1mMOgHN2HPwwDQYJKoZIhvcNAQEL',
  'BQAwaDELMAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcM',
  'DVNhbiBGcmFuY2lzY28xETAPBgNVBAoMCFRlc3QgT3JnMRkwFwYDVQQDDBB0ZXN0',
  'LmV4YW1wbGUuY29tMB4XDTI2MDIyMjE2MDQyMloXDTI3MDIyMjE2MDQyMlowaDEL',
  'MAkGA1UEBhMCVVMxEzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcMDVNhbiBG',
  'cmFuY2lzY28xETAPBgNVBAoMCFRlc3QgT3JnMRkwFwYDVQQDDBB0ZXN0LmV4YW1w',
  'bGUuY29tMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEArRRWC6JnpD14',
  'nqLaGC/GDbavICOLXJOvnsUmmSQneFyGKF/21oz/+ywnznM6BkmjXQJQH7lSfjf6',
  '2nyavZvN21v0uZ1JwCUl3gqEvqoBPwlo57ZC8lrEm/OfGs9R+AMBZHr3AelmoV1r',
  'giwFbSVhth9Thquby2RPF/jbgs2m/oSPSVRooOCkUfdCbp1DAC17+lyyhrByczMw',
  'TCfZZi/bi6Bl9mUyIOImfxw4VDUIjG2z+3htoRMlt7DGmAcf0nHOtl6Y/PgNKGOL',
  'lAuiDp31cRGU7u2+ptrHH2nSrQbWkcDO7QClAFFsUyMWudVoSWp2LB5faBDtLr/K',
  'Buu6H+hM9QIDAQABo1MwUTAdBgNVHQ4EFgQUfuLq1fvV3xoyMudVt1WXuqbKvaAw',
  'HwYDVR0jBBgwFoAUfuLq1fvV3xoyMudVt1WXuqbKvaAwDwYDVR0TAQH/BAUwAwEB',
  '/zANBgkqhkiG9w0BAQsFAAOCAQEAQNGez0mH+lSa2R43Ex+20R+OECUnYu9CuCCK',
  'tfX1rVUCejYbRKXr/w2UsQ2jQ5vzyOUOtlg9gEccnI7lqrXzi+tXYwQtsF0RSBvQ',
  'HDhxTr7N2ZPch3E6Pu1VjK7GaKM6J0iLal76AhFZI5lUPxftRP1wvb4xFeU0/HCR',
  'Lj1tTefuCCXM7dOrSUFau7I56ythgbppFW6052AVdXhypPrIqWaiKwnXBO+Y7znQ',
  'fPWakaZEY44H0JWR7v6g9qk9RtCTDsxEr9qDH40PPQTT5dR6Y2nUd4nqqSXnoOTf',
  'rL6NaXtNHWpD0yoc9+z0o1uBEI19++PrtMnl0j3fgtVyNIl5UQ==',
  '-----END CERTIFICATE-----',
].join('\n');

describe('parsePemContent', () => {
  it('parses a single PEM certificate', () => {
    const buffers = parsePemContent(SIMPLE_PEM);
    expect(buffers).toHaveLength(1);
    expect(buffers[0].byteLength).toBeGreaterThan(100);
  });

  it('parses multiple PEM certificates', () => {
    const combined = `${SIMPLE_PEM}\n\n${ESCAPED_COMMA_PEM}`;
    const buffers = parsePemContent(combined);
    expect(buffers).toHaveLength(2);
  });

  it('parses raw base64 DER (no PEM headers)', () => {
    const base64Only = SIMPLE_PEM.replace(/-----BEGIN CERTIFICATE-----/, '')
      .replace(/-----END CERTIFICATE-----/, '')
      .trim();
    const buffers = parsePemContent(base64Only);
    expect(buffers).toHaveLength(1);
    expect(buffers[0].byteLength).toBeGreaterThan(100);
  });

  it('throws on empty input', () => {
    expect(() => parsePemContent('')).toThrow('No certificate data found');
  });

  it('throws on invalid base64', () => {
    expect(() => parsePemContent('not-valid-base64!!!')).toThrow();
  });
});

describe('parseCertificateFromDer', () => {
  function parsePem(pem: string) {
    const [der] = parsePemContent(pem);
    return parseCertificateFromDer(der, pem);
  }

  describe('simple self-signed certificate', () => {
    it('extracts subject DN fields', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.subjectCN).toBe('test.example.com');
      expect(cert.subjectO).toBe('Test Org');
      expect(cert.subjectC).toBe('US');
      expect(cert.subjectST).toBe('California');
      expect(cert.subjectL).toBe('San Francisco');
    });

    it('extracts issuer DN fields', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.issuerCN).toBe('test.example.com');
      expect(cert.issuerO).toBe('Test Org');
      expect(cert.issuerC).toBe('US');
    });

    it('detects RSA 2048 key algorithm', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.keyAlgorithmName).toBe('RSA (2048 bits)');
      expect(cert.keyAlgorithm).toEqual({ keyType: 'rsa', keySize: '2048' });
    });

    it('detects SHA-256 signature algorithm', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.signatureAlgorithm).toBe('sha256');
    });

    it('detects CA certificate', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.isCa).toBe(true);
    });

    it('extracts validity dates', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.notBefore).toBeInstanceOf(Date);
      expect(cert.notAfter).toBeInstanceOf(Date);
      expect(cert.notAfter.getTime()).toBeGreaterThan(cert.notBefore.getTime());
    });

    it('computes SHA-256 fingerprint', async () => {
      const cert = await parsePem(SIMPLE_PEM);
      expect(cert.sha256Fingerprint).toMatch(/^[0-9A-F]{2}(:[0-9A-F]{2}){31}$/);
    });
  });

  describe('certificate with escaped comma in DN', () => {
    it('unescapes organization name containing a comma', async () => {
      const cert = await parsePem(ESCAPED_COMMA_PEM);
      expect(cert.subjectO).toBe('Secutils, Inc.');
    });

    it('extracts wildcard CN', async () => {
      const cert = await parsePem(ESCAPED_COMMA_PEM);
      expect(cert.subjectCN).toBe('*.secutils.dev');
    });

    it('extracts all subject DN fields', async () => {
      const cert = await parsePem(ESCAPED_COMMA_PEM);
      expect(cert.subjectC).toBe('US');
      expect(cert.subjectST).toBe('California');
      expect(cert.subjectL).toBe('San Francisco');
    });

    it('is not a CA certificate', async () => {
      const cert = await parsePem(ESCAPED_COMMA_PEM);
      expect(cert.isCa).toBe(false);
    });

    it('extracts key usage and extended key usage', async () => {
      const cert = await parsePem(ESCAPED_COMMA_PEM);
      expect(cert.keyUsage).toContain('digitalSignature');
      expect(cert.keyUsage).toContain('keyEncipherment');
      expect(cert.extendedKeyUsage).toContain('tlsWebServerAuthentication');
      expect(cert.extendedKeyUsage).toContain('tlsWebClientAuthentication');
    });
  });
});

describe('certificateToTemplateAttributes', () => {
  it('converts a supported certificate to template attributes', async () => {
    const [der] = parsePemContent(ESCAPED_COMMA_PEM);
    const cert = await parseCertificateFromDer(der, ESCAPED_COMMA_PEM);
    const attrs = certificateToTemplateAttributes(cert);

    expect(attrs).not.toBeNull();
    expect(attrs!.commonName).toBe('*.secutils.dev');
    expect(attrs!.organization).toBe('Secutils, Inc.');
    expect(attrs!.country).toBe('US');
    expect(attrs!.stateOrProvince).toBe('California');
    expect(attrs!.locality).toBe('San Francisco');
    expect(attrs!.keyAlgorithm).toEqual({ keyType: 'rsa', keySize: '2048' });
    expect(attrs!.signatureAlgorithm).toBe('sha256');
    expect(attrs!.isCa).toBe(false);
  });

  it('returns null for unsupported key algorithm', async () => {
    const [der] = parsePemContent(SIMPLE_PEM);
    const cert = await parseCertificateFromDer(der, SIMPLE_PEM);
    const modified = { ...cert, keyAlgorithm: undefined };
    expect(certificateToTemplateAttributes(modified)).toBeNull();
  });

  it('returns null for unsupported signature algorithm', async () => {
    const [der] = parsePemContent(SIMPLE_PEM);
    const cert = await parseCertificateFromDer(der, SIMPLE_PEM);
    const modified = { ...cert, signatureAlgorithm: undefined };
    expect(certificateToTemplateAttributes(modified)).toBeNull();
  });
});

describe('getDefaultCertificateName', () => {
  it('uses CN when available', async () => {
    const [der] = parsePemContent(ESCAPED_COMMA_PEM);
    const cert = await parseCertificateFromDer(der, ESCAPED_COMMA_PEM);
    expect(getDefaultCertificateName(cert, 0)).toBe('*.secutils.dev');
  });

  it('falls back to O when CN is absent', async () => {
    const [der] = parsePemContent(SIMPLE_PEM);
    const cert = await parseCertificateFromDer(der, SIMPLE_PEM);
    const modified = { ...cert, subjectCN: undefined };
    expect(getDefaultCertificateName(modified, 0)).toBe('Test Org');
  });

  it('falls back to "Certificate N" when both CN and O are absent', async () => {
    const [der] = parsePemContent(SIMPLE_PEM);
    const cert = await parseCertificateFromDer(der, SIMPLE_PEM);
    const modified = { ...cert, subjectCN: undefined, subjectO: undefined };
    expect(getDefaultCertificateName(modified, 2)).toBe('Certificate 3');
  });
});
