import {
  EuiAccordion,
  EuiBadge,
  EuiCheckbox,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormRow,
  EuiSpacer,
  EuiText,
  EuiToolTip,
  htmlIdGenerator,
} from '@elastic/eui';
import type { ReactNode } from 'react';
import { useCallback } from 'react';

import type { ParsedCertificate } from './certificate_import_utils';
import { certificateToTemplateAttributes, getDefaultCertificateName } from './certificate_import_utils';

export interface CertificateSelection {
  selected: boolean;
  name: string;
}

export interface CertificateImportPreviewProps {
  certificates: ParsedCertificate[];
  selections: CertificateSelection[];
  onSelectionsChange: (selections: CertificateSelection[]) => void;
}

function formatDate(date: Date): string {
  return date.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function CertificateDetail({ label, value }: { label: string; value?: string | null }) {
  if (!value) {
    return null;
  }

  return (
    <EuiFlexGroup gutterSize="s" alignItems="baseline" responsive={false} style={{ marginBottom: 4 }}>
      <EuiFlexItem grow={false} style={{ minWidth: 160 }}>
        <EuiText size="xs" color="subdued">
          <strong>{label}</strong>
        </EuiText>
      </EuiFlexItem>
      <EuiFlexItem>
        <EuiText size="xs" style={{ wordBreak: 'break-all' }}>
          {value}
        </EuiText>
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}

function CertificateSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div style={{ marginBottom: 12 }}>
      <EuiText size="xs">
        <strong>{title}</strong>
      </EuiText>
      <EuiSpacer size="xs" />
      <div style={{ paddingLeft: 8 }}>{children}</div>
    </div>
  );
}

const KEY_USAGE_LABELS: Record<string, string> = {
  digitalSignature: 'Digital Signature',
  nonRepudiation: 'Non-Repudiation',
  keyEncipherment: 'Key Encipherment',
  dataEncipherment: 'Data Encipherment',
  keyAgreement: 'Key Agreement',
  keyCertificateSigning: 'Certificate Signing',
  crlSigning: 'CRL Signing',
  encipherOnly: 'Encipher Only',
  decipherOnly: 'Decipher Only',
};

const EXT_KEY_USAGE_LABELS: Record<string, string> = {
  tlsWebServerAuthentication: 'TLS Web Server Authentication',
  tlsWebClientAuthentication: 'TLS Web Client Authentication',
  codeSigning: 'Code Signing',
  emailProtection: 'Email Protection',
  timeStamping: 'Timestamping',
};

function CertificateDetails({ cert }: { cert: ParsedCertificate }) {
  const canImport = certificateToTemplateAttributes(cert) !== null;

  return (
    <div>
      <CertificateSection title="Subject">
        <CertificateDetail label="Common Name (CN)" value={cert.subjectCN} />
        <CertificateDetail label="Organization (O)" value={cert.subjectO} />
        <CertificateDetail label="Org. Unit (OU)" value={cert.subjectOU} />
        <CertificateDetail label="Country (C)" value={cert.subjectC} />
        <CertificateDetail label="State (ST)" value={cert.subjectST} />
        <CertificateDetail label="Locality (L)" value={cert.subjectL} />
      </CertificateSection>

      <CertificateSection title="Issuer">
        <CertificateDetail label="Common Name (CN)" value={cert.issuerCN} />
        <CertificateDetail label="Organization (O)" value={cert.issuerO} />
        <CertificateDetail label="Country (C)" value={cert.issuerC} />
      </CertificateSection>

      <CertificateSection title="Validity">
        <CertificateDetail label="Not Before" value={formatDate(cert.notBefore)} />
        <CertificateDetail label="Not After" value={formatDate(cert.notAfter)} />
      </CertificateSection>

      <CertificateSection title="Certificate">
        <CertificateDetail label="Serial Number" value={cert.serialNumber} />
        <CertificateDetail label="Type" value={cert.isCa ? 'Certification Authority' : 'End Entity'} />
      </CertificateSection>

      <CertificateSection title="Key Info">
        <CertificateDetail label="Key Algorithm" value={cert.keyAlgorithmName} />
        <CertificateDetail label="Signature Algorithm" value={cert.signatureAlgorithmName} />
      </CertificateSection>

      {cert.keyUsage.length > 0 ? (
        <CertificateSection title="Key Usage">
          <EuiFlexGroup gutterSize="xs" wrap>
            {cert.keyUsage.map((usage) => (
              <EuiFlexItem grow={false} key={usage}>
                <EuiBadge color="hollow">{KEY_USAGE_LABELS[usage] ?? usage}</EuiBadge>
              </EuiFlexItem>
            ))}
          </EuiFlexGroup>
        </CertificateSection>
      ) : null}

      {cert.extendedKeyUsage.length > 0 ? (
        <CertificateSection title="Extended Key Usage">
          <EuiFlexGroup gutterSize="xs" wrap>
            {cert.extendedKeyUsage.map((usage) => (
              <EuiFlexItem grow={false} key={usage}>
                <EuiBadge color="hollow">{EXT_KEY_USAGE_LABELS[usage] ?? usage}</EuiBadge>
              </EuiFlexItem>
            ))}
          </EuiFlexGroup>
        </CertificateSection>
      ) : null}

      <CertificateSection title="Fingerprint">
        <CertificateDetail label="SHA-256" value={cert.sha256Fingerprint} />
      </CertificateSection>

      {!canImport ? (
        <>
          <EuiSpacer size="s" />
          <EuiText size="xs" color="warning">
            This certificate uses an unsupported key or signature algorithm and cannot be imported as a template.
          </EuiText>
        </>
      ) : null}
    </div>
  );
}

export function CertificateImportPreview({
  certificates,
  selections,
  onSelectionsChange,
}: CertificateImportPreviewProps) {
  const updateSelection = useCallback(
    (index: number, update: Partial<CertificateSelection>) => {
      const newSelections = [...selections];
      newSelections[index] = { ...newSelections[index], ...update };
      onSelectionsChange(newSelections);
    },
    [selections, onSelectionsChange],
  );

  return (
    <div>
      {certificates.map((cert, index) => {
        const canImport = certificateToTemplateAttributes(cert) !== null;
        const defaultName = getDefaultCertificateName(cert, index);
        const selection = selections[index];

        const buttonContent = (
          <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiCheckbox
                id={htmlIdGenerator()()}
                checked={selection.selected}
                disabled={!canImport}
                onChange={(e) => updateSelection(index, { selected: e.target.checked })}
                onClick={(e) => e.stopPropagation()}
              />
            </EuiFlexItem>
            <EuiFlexItem>
              <EuiText size="s">
                <strong>{defaultName}</strong>
                {!canImport ? (
                  <EuiToolTip content="Unsupported key algorithm">
                    <EuiBadge color="warning" style={{ marginLeft: 8 }}>
                      Unsupported
                    </EuiBadge>
                  </EuiToolTip>
                ) : null}
              </EuiText>
            </EuiFlexItem>
          </EuiFlexGroup>
        );

        return (
          <div key={index} style={{ marginBottom: 8 }}>
            <EuiAccordion id={`cert-accordion-${index}`} buttonContent={buttonContent} paddingSize="m">
              <CertificateDetails cert={cert} />
              {canImport && selection.selected ? (
                <>
                  <EuiSpacer size="m" />
                  <EuiFormRow label="Template name" fullWidth>
                    <EuiFieldText
                      fullWidth
                      value={selection.name}
                      placeholder={defaultName}
                      onChange={(e) => updateSelection(index, { name: e.target.value })}
                    />
                  </EuiFormRow>
                </>
              ) : null}
            </EuiAccordion>
          </div>
        );
      })}
    </div>
  );
}
