import type { ChangeEvent } from 'react';
import { useState } from 'react';

import {
  EuiComboBox,
  EuiDescribedFormGroup,
  EuiFieldText,
  EuiForm,
  EuiFormRow,
  EuiLink,
  EuiSelect,
} from '@elastic/eui';

import type { CertificateAttributes, SignatureAlgorithm } from './certificate_attributes';
import { CertificateLifetimeCalendar } from './certificate_lifetime_calendar';
import type { CertificateTemplate } from './certificate_template';
import type { PrivateKeyAlgorithm, PrivateKeyCurveName, PrivateKeySize } from './private_key_alg';
import { privateKeyCurveNameString } from './private_key_alg';

export interface CertificateTemplateFormProps {
  template: CertificateTemplate;
  onChange?: (template: CertificateTemplate) => void;
  isReadOnly?: boolean;
}

const SIGNATURE_ALGORITHMS = new Map<string, Array<{ text: string; value: SignatureAlgorithm }>>([
  [
    'rsa',
    [
      { value: 'md5', text: 'MD5' },
      { value: 'sha1', text: 'SHA-1' },
      { value: 'sha256', text: 'SHA-256' },
      { value: 'sha384', text: 'SHA-384' },
      { value: 'sha512', text: 'SHA-512' },
    ],
  ],
  [
    'dsa',
    [
      { value: 'sha1', text: 'SHA-1' },
      { value: 'sha256', text: 'SHA-256' },
    ],
  ],
  [
    'ecdsa',
    [
      { value: 'sha1', text: 'SHA-1' },
      { value: 'sha256', text: 'SHA-256' },
      { value: 'sha384', text: 'SHA-384' },
      { value: 'sha512', text: 'SHA-512' },
    ],
  ],
  ['ed25519', [{ value: 'ed25519', text: 'Ed25519' }]],
]);

const KEY_USAGE = new Map([
  ['crlSigning', { label: 'CRL signing', value: 'crlSigning' }],
  ['dataEncipherment', { label: 'Data encipherment', value: 'dataEncipherment' }],
  ['decipherOnly', { label: 'Decipher only', value: 'decipherOnly' }],
  ['digitalSignature', { label: 'Digital signature', value: 'digitalSignature' }],
  ['encipherOnly', { label: 'Encipher only', value: 'encipherOnly' }],
  ['keyAgreement', { label: 'Key agreement', value: 'keyAgreement' }],
  ['keyCertificateSigning', { label: 'Certificate signing', value: 'keyCertificateSigning' }],
  ['keyEncipherment', { label: 'Key encipherment', value: 'keyEncipherment' }],
  ['nonRepudiation', { label: 'Non-repudiation', value: 'nonRepudiation' }],
]);

const EXTENDED_KEY_USAGE = new Map([
  ['codeSigning', { label: 'Sign code', value: 'codeSigning' }],
  ['emailProtection', { label: 'Email protection', value: 'emailProtection' }],
  ['timeStamping', { label: 'Timestamping', value: 'timeStamping' }],
  ['tlsWebClientAuthentication', { label: 'TLS Web client authentication', value: 'tlsWebClientAuthentication' }],
  ['tlsWebServerAuthentication', { label: 'TLS Web server authentication', value: 'tlsWebServerAuthentication' }],
]);

export function CertificateTemplateForm({ template, onChange, isReadOnly }: CertificateTemplateFormProps) {
  const [name, setName] = useState<string>(template.name);
  const onNameChange = (e: ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
    onChange?.({ ...template, name: e.target.value.trim(), attributes });
  };

  const [attributes, setAttributes] = useState<CertificateAttributes>(template.attributes);
  const onAttributesChange = (partialAttributes: Partial<CertificateAttributes>) => {
    const newAttributes = { ...attributes, ...partialAttributes };
    setAttributes(newAttributes);
    onChange?.({ ...template, name: name.trim(), attributes: newAttributes });
  };

  const [signatureAlgorithms, setSignatureAlgorithms] = useState(
    SIGNATURE_ALGORITHMS.get(attributes.keyAlgorithm?.keyType ?? 'ed25519')!,
  );

  const onKeyAlgorithmChange = (e: ChangeEvent<HTMLSelectElement>) => {
    const keyType = e.target.value as PrivateKeyAlgorithm['keyType'];

    const newSignatureAlgorithms = SIGNATURE_ALGORITHMS.get(keyType)!;
    setSignatureAlgorithms(newSignatureAlgorithms);

    onAttributesChange({
      keyAlgorithm:
        keyType === 'ed25519'
          ? { keyType }
          : keyType === 'ecdsa'
            ? { keyType, curve: 'secp256r1' }
            : { keyType, keySize: '2048' },
      signatureAlgorithm: newSignatureAlgorithms[0].value,
    });
  };

  return (
    <EuiForm id="update-form" component="form" fullWidth>
      <EuiDescribedFormGroup title={<h3>General</h3>} description={'General properties of the certificate template'}>
        <EuiFormRow label="Name" helpText="Unique name of the certificate template." isDisabled={isReadOnly}>
          <EuiFieldText value={name} required type={'text'} onChange={onNameChange} />
        </EuiFormRow>
        <EuiFormRow label="Key algorithm" helpText="Private key algorithm." isDisabled={isReadOnly}>
          <EuiSelect
            options={[
              { value: 'rsa', text: 'RSA' },
              { value: 'dsa', text: 'DSA' },
              { value: 'ecdsa', text: 'ECDSA' },
              { value: 'ed25519', text: 'Ed25519' },
            ]}
            value={attributes.keyAlgorithm.keyType}
            onChange={onKeyAlgorithmChange}
          />
        </EuiFormRow>
        {'keySize' in attributes.keyAlgorithm ? (
          <EuiFormRow label="Key size" helpText="Private key size." isDisabled={isReadOnly}>
            <EuiSelect
              options={[
                { value: '1024', text: '1024 bit' },
                { value: '2048', text: '2048 bit' },
                { value: '4096', text: '4096 bit' },
                { value: '8192', text: '8192 bit' },
              ]}
              value={attributes.keyAlgorithm.keySize}
              onChange={(e) =>
                onAttributesChange({
                  keyAlgorithm:
                    'keySize' in attributes.keyAlgorithm
                      ? { ...attributes.keyAlgorithm, keySize: e.target.value as PrivateKeySize }
                      : attributes.keyAlgorithm,
                })
              }
            />
          </EuiFormRow>
        ) : null}
        {'curve' in attributes.keyAlgorithm ? (
          <EuiFormRow
            label="Curve name"
            helpText={
              <span>
                <EuiLink target="_blank" href="https://www.rfc-editor.org/rfc/rfc8422.html#section-5.1.1">
                  Elliptic curve
                </EuiLink>{' '}
                used for cryptographic operations.
              </span>
            }
            isDisabled={isReadOnly}
          >
            <EuiSelect
              options={[
                { value: 'secp256r1', text: privateKeyCurveNameString('secp256r1') },
                { value: 'secp384r1', text: privateKeyCurveNameString('secp384r1') },
                { value: 'secp521r1', text: privateKeyCurveNameString('secp521r1') },
              ]}
              value={attributes.keyAlgorithm.curve}
              onChange={(e) =>
                onAttributesChange({
                  keyAlgorithm:
                    'curve' in attributes.keyAlgorithm
                      ? { ...attributes.keyAlgorithm, curve: e.target.value as PrivateKeyCurveName }
                      : attributes.keyAlgorithm,
                })
              }
            />
          </EuiFormRow>
        ) : null}
        <EuiFormRow label="Signature algorithm" helpText="Public key signature algorithm." isDisabled={isReadOnly}>
          <EuiSelect
            options={signatureAlgorithms}
            value={attributes.signatureAlgorithm}
            disabled={signatureAlgorithms.length === 1 || isReadOnly}
            onChange={(e) => onAttributesChange({ signatureAlgorithm: e.target.value as SignatureAlgorithm })}
          />
        </EuiFormRow>
      </EuiDescribedFormGroup>
      <EuiDescribedFormGroup
        title={<h3>Extensions</h3>}
        description={
          <span>
            Properties defined by the{' '}
            <EuiLink target="_blank" href="https://www.ietf.org/rfc/rfc3280.html">
              X.509 extensions
            </EuiLink>
          </span>
        }
      >
        <EuiFormRow
          label="Certificate type"
          helpText="Specifies whether the certificate can be used to sign other certificates (Certification Authority) or not."
          isDisabled={isReadOnly}
        >
          <EuiSelect
            value={attributes.isCa ? 'ca' : 'endEntity'}
            onChange={(e) => onAttributesChange({ isCa: e.target.value === 'ca' })}
            options={[
              { value: 'ca', text: 'Certification Authority' },
              { value: 'endEntity', text: 'End Entity' },
            ]}
          />
        </EuiFormRow>
        {!isReadOnly || (attributes.keyUsage?.length ?? 0) > 0 ? (
          <EuiFormRow
            label="Key usage"
            helpText="Defines the purpose of the public key contained in the certificate."
            fullWidth
            isDisabled={isReadOnly}
          >
            <EuiComboBox
              isDisabled={isReadOnly}
              fullWidth
              options={Array.from(KEY_USAGE.values())}
              selectedOptions={attributes.keyUsage?.map((usage) => KEY_USAGE.get(usage)!) ?? []}
              onChange={(options) =>
                onAttributesChange({ keyUsage: options.length > 0 ? options.map(({ value }) => value!) : undefined })
              }
            />
          </EuiFormRow>
        ) : null}
        {!isReadOnly || (attributes.extendedKeyUsage?.length ?? 0) > 0 ? (
          <EuiFormRow
            label="Extended key usage"
            helpText="Defines the purpose of the public key contained in the certificate, in addition to or in place of the basic purposes indicated in the key usage property."
            fullWidth
            isDisabled={isReadOnly}
          >
            <EuiComboBox
              isDisabled={isReadOnly}
              fullWidth
              options={Array.from(EXTENDED_KEY_USAGE.values())}
              selectedOptions={attributes.extendedKeyUsage?.map((usage) => EXTENDED_KEY_USAGE.get(usage)!) ?? []}
              onChange={(options) =>
                onAttributesChange({
                  extendedKeyUsage: options.length > 0 ? options.map(({ value }) => value!) : undefined,
                })
              }
            />
          </EuiFormRow>
        ) : null}
      </EuiDescribedFormGroup>
      <EuiDescribedFormGroup
        title={<h3>Distinguished Name (DN)</h3>}
        description={'Properties of the issuer Distinguished Name (DN)'}
      >
        <EuiFormRow
          label="Country (C)"
          helpText="List of country (C) 2 character codes. The field can contain an array of values. Example: US"
          isDisabled={isReadOnly}
        >
          <EuiFieldText
            value={attributes.country ?? ''}
            type={'text'}
            onChange={(e) => onAttributesChange({ country: e.target.value ? e.target.value : undefined })}
          />
        </EuiFormRow>
        <EuiFormRow
          label="State or province (ST, S, or P)"
          helpText="List of state or province names (ST, S, or P). The field can contain an array of values. Example: California"
          isDisabled={isReadOnly}
        >
          <EuiFieldText
            value={attributes.stateOrProvince ?? ''}
            type={'text'}
            onChange={(e) => onAttributesChange({ stateOrProvince: e.target.value ? e.target.value : undefined })}
          />
        </EuiFormRow>
        <EuiFormRow
          label="Locality (L)"
          helpText="List of locality names (L). The field can contain an array of values. Example: Berlin"
          isDisabled={isReadOnly}
        >
          <EuiFieldText
            value={attributes.locality ?? ''}
            type={'text'}
            onChange={(e) => onAttributesChange({ locality: e.target.value ? e.target.value : undefined })}
          />
        </EuiFormRow>
        <EuiFormRow
          label="Organization (O)"
          helpText="List of organizations (O) of issuing certificate authority. The field can contain an array of values. Example: CA Issuer, Inc"
          isDisabled={isReadOnly}
        >
          <EuiFieldText
            value={attributes.organization ?? ''}
            type={'text'}
            onChange={(e) => onAttributesChange({ organization: e.target.value ? e.target.value : undefined })}
          />
        </EuiFormRow>
        <EuiFormRow
          label="Organizational unit (OU)"
          helpText="List of organizational units (OU) of issuing certificate authority. The field can contain an array of values. Example: www.example.com"
          isDisabled={isReadOnly}
        >
          <EuiFieldText
            value={attributes.organizationalUnit ?? ''}
            type={'text'}
            onChange={(e) => onAttributesChange({ organizationalUnit: e.target.value ? e.target.value : undefined })}
          />
        </EuiFormRow>
        <EuiFormRow
          label="Common name (CN)"
          helpText="List of common name (CN) of issuing certificate authority. The field can contain an array of values. Example: CA Issuer"
          isDisabled={isReadOnly}
        >
          <EuiFieldText
            value={attributes.commonName ?? ''}
            type={'text'}
            onChange={(e) => onAttributesChange({ commonName: e.target.value ? e.target.value : undefined })}
          />
        </EuiFormRow>
      </EuiDescribedFormGroup>
      <EuiDescribedFormGroup title={<h3>Validity</h3>} description="Certificate Authority certificate validity.">
        <EuiFormRow label="Not valid before" isDisabled={isReadOnly}>
          <CertificateLifetimeCalendar
            isDisabled={isReadOnly}
            currentTimestamp={attributes.notValidBefore}
            onChange={(notValidBefore) => onAttributesChange({ notValidBefore })}
          />
        </EuiFormRow>
        <EuiFormRow label="Not valid after" isDisabled={isReadOnly}>
          <CertificateLifetimeCalendar
            isDisabled={isReadOnly}
            currentTimestamp={attributes.notValidAfter}
            onChange={(notValidAfter) => onAttributesChange({ notValidAfter })}
          />
        </EuiFormRow>
      </EuiDescribedFormGroup>
    </EuiForm>
  );
}
