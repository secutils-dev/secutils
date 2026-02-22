import {
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiFieldText,
  EuiFilePicker,
  EuiForm,
  EuiFormRow,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSpacer,
  EuiTab,
  EuiTabs,
  EuiText,
  EuiTextArea,
  EuiTitle,
  htmlIdGenerator,
} from '@elastic/eui';
import { useCallback, useState } from 'react';

import type { CertificateSelection } from './certificate_import_preview';
import { CertificateImportPreview } from './certificate_import_preview';
import type { ParsedCertificate } from './certificate_import_utils';
import {
  certificateToTemplateAttributes,
  getDefaultCertificateName,
  parseCertificateFromDer,
  parsePemContent,
} from './certificate_import_utils';
import {
  type AsyncData,
  getApiRequestConfig,
  getApiUrl,
  getErrorMessage,
  isClientError,
  ResponseError,
} from '../../../../model';
import { isValidURL } from '../../../../tools/url';
import { useWorkspaceContext } from '../../hooks';

export interface CertificateTemplateImportModalProps {
  onClose: (success?: boolean) => void;
}

type ImportTab = 'pem' | 'file' | 'url';

export function CertificateTemplateImportModal({ onClose }: CertificateTemplateImportModalProps) {
  const { uiState, addToast } = useWorkspaceContext();

  const [activeTab, setActiveTab] = useState<ImportTab>('pem');
  const [pemContent, setPemContent] = useState('');
  const [urlValue, setUrlValue] = useState('');
  const [parseError, setParseError] = useState<string | null>(null);

  const [certificates, setCertificates] = useState<ParsedCertificate[]>([]);
  const [selections, setSelections] = useState<CertificateSelection[]>([]);

  const [parseStatus, setParseStatus] = useState<AsyncData<undefined> | null>(null);
  const [importStatus, setImportStatus] = useState<AsyncData<undefined> | null>(null);

  const clearResults = useCallback(() => {
    setCertificates([]);
    setSelections([]);
    setParseError(null);
  }, []);

  const handleParsedCerts = useCallback(async (derBuffers: ArrayBuffer[], pemStrings: string[]) => {
    try {
      const parsed: ParsedCertificate[] = [];
      for (let i = 0; i < derBuffers.length; i++) {
        parsed.push(await parseCertificateFromDer(derBuffers[i], pemStrings[i]));
      }

      if (parsed.length === 0) {
        setParseError('No valid certificates found.');
        return;
      }

      setCertificates(parsed);
      setSelections(
        parsed.map((cert, index) => ({
          selected: certificateToTemplateAttributes(cert) !== null,
          name: getDefaultCertificateName(cert, index),
        })),
      );
      setParseError(null);
    } catch (err) {
      setParseError(err instanceof Error ? err.message : 'Failed to parse certificates.');
    }
  }, []);

  const handleParsePem = useCallback(
    async (content: string) => {
      clearResults();
      setParseStatus({ status: 'pending' });

      try {
        const derBuffers = parsePemContent(content);
        // Reconstruct PEM strings for each certificate.
        const pemStrings = derBuffers.map((der) => {
          const base64 = btoa(Array.from(new Uint8Array(der), (b) => String.fromCharCode(b)).join(''));
          const lines = base64.match(/.{1,64}/g) ?? [];
          return `-----BEGIN CERTIFICATE-----\n${lines.join('\n')}\n-----END CERTIFICATE-----`;
        });

        await handleParsedCerts(derBuffers, pemStrings);
        setParseStatus({ status: 'succeeded', data: undefined });
      } catch (err) {
        setParseError(err instanceof Error ? err.message : 'Failed to parse PEM content.');
        setParseStatus({ status: 'failed', error: 'Parse failed' });
      }
    },
    [clearResults, handleParsedCerts],
  );

  const handleFileUpload = useCallback(
    async (files: FileList | null) => {
      if (!files || files.length === 0) {
        clearResults();
        return;
      }

      const file = files[0];
      try {
        const content = await file.text();
        await handleParsePem(content);
      } catch (err) {
        setParseError(err instanceof Error ? err.message : 'Failed to read file.');
      }
    },
    [clearResults, handleParsePem],
  );

  const handleFetchUrl = useCallback(async () => {
    if (!uiState.synced || parseStatus?.status === 'pending') {
      return;
    }

    clearResults();
    setParseStatus({ status: 'pending' });

    try {
      const response = await fetch(getApiUrl('/api/utils/certificates/templates/peer_certificates'), {
        ...getApiRequestConfig('POST'),
        body: JSON.stringify({ url: urlValue }),
      });

      if (!response.ok) {
        throw await ResponseError.fromResponse(response);
      }

      const pemStrings: string[] = await response.json();
      if (pemStrings.length === 0) {
        setParseError('No certificates found at the specified URL.');
        setParseStatus({ status: 'failed', error: 'No certificates' });
        return;
      }

      const derBuffers = pemStrings.flatMap((pem) => {
        try {
          return parsePemContent(pem);
        } catch {
          return [];
        }
      });

      await handleParsedCerts(derBuffers, pemStrings);
      setParseStatus({ status: 'succeeded', data: undefined });
    } catch (err: unknown) {
      const errorMessage = getErrorMessage(err as Error);
      setParseError(
        isClientError(err as Error) ? errorMessage : 'Unable to fetch certificates from the specified URL.',
      );
      setParseStatus({ status: 'failed', error: errorMessage });
    }
  }, [uiState.synced, parseStatus?.status, clearResults, urlValue, handleParsedCerts]);

  const selectedCerts = certificates
    .map((cert, index) => ({ cert, selection: selections[index] }))
    .filter(({ selection }) => selection.selected && selection.name.trim().length > 0);

  const canImport = selectedCerts.length > 0 && importStatus?.status !== 'pending';

  const handleImport = useCallback(async () => {
    if (!canImport || !uiState.synced) {
      return;
    }

    setImportStatus({ status: 'pending' });

    let successCount = 0;
    let lastError: string | null = null;

    for (const { cert, selection } of selectedCerts) {
      const attributes = certificateToTemplateAttributes(cert);
      if (!attributes) {
        continue;
      }

      try {
        const response = await fetch(getApiUrl('/api/utils/certificates/templates'), {
          ...getApiRequestConfig('POST'),
          body: JSON.stringify({ templateName: selection.name.trim(), attributes }),
        });

        if (!response.ok) {
          throw await ResponseError.fromResponse(response);
        }

        successCount++;
      } catch (err: unknown) {
        lastError = getErrorMessage(err as Error);
        addToast({
          id: htmlIdGenerator('failed-import-template')(),
          iconType: 'warning',
          color: 'danger',
          title: isClientError(err as Error)
            ? lastError
            : `Unable to import "${selection.name}" template, please try again later`,
        });
      }
    }

    if (successCount > 0) {
      addToast({
        id: `success-import-templates`,
        iconType: 'check',
        color: 'success',
        title:
          successCount === 1
            ? `Successfully imported 1 certificate template`
            : `Successfully imported ${successCount} certificate templates`,
      });
      setImportStatus({ status: 'succeeded', data: undefined });
      onClose(true);
    } else {
      setImportStatus({ status: 'failed', error: lastError ?? 'Import failed' });
    }
  }, [canImport, uiState.synced, selectedCerts, addToast, onClose]);

  const canParse =
    parseStatus?.status !== 'pending' &&
    ((activeTab === 'pem' && pemContent.trim().length > 0) ||
      (activeTab === 'url' && isValidURL(urlValue) && urlValue.startsWith('https')));

  const pemInput = (
    <EuiFormRow label="PEM content" helpText="Paste one or more PEM-encoded certificates" fullWidth>
      <EuiTextArea
        fullWidth
        value={pemContent}
        rows={6}
        placeholder={'-----BEGIN CERTIFICATE-----\nMIID...base64 encoded...\n-----END CERTIFICATE-----'}
        onChange={(e) => {
          setPemContent(e.target.value);
          clearResults();
        }}
      />
    </EuiFormRow>
  );

  const fileInput = (
    <EuiFormRow label="Certificate file" helpText="Select a .pem, .crt, .cer, or .cert file" fullWidth>
      <EuiFilePicker
        fullWidth
        accept=".pem,.crt,.cer,.cert"
        onChange={handleFileUpload}
        display="default"
        initialPromptText="Select or drag a certificate file"
      />
    </EuiFormRow>
  );

  const urlInput = (
    <EuiFormRow
      label="URL"
      helpText="HTTPS URL to extract the TLS certificate chain from (e.g., https://example.com)"
      fullWidth
    >
      <EuiFieldText
        fullWidth
        placeholder="https://example.com"
        value={urlValue}
        type="url"
        onChange={(e) => {
          setUrlValue(e.target.value);
          clearResults();
        }}
      />
    </EuiFormRow>
  );

  return (
    <EuiModal onClose={() => onClose()} style={{ maxWidth: 650, width: '90vw' }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size="s">
            <span>Import certificate template</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm component="form" onSubmit={(e) => e.preventDefault()}>
          <EuiTabs size="s">
            <EuiTab
              isSelected={activeTab === 'pem'}
              onClick={() => {
                setActiveTab('pem');
                clearResults();
              }}
            >
              PEM content
            </EuiTab>
            <EuiTab
              isSelected={activeTab === 'file'}
              onClick={() => {
                setActiveTab('file');
                clearResults();
              }}
            >
              File
            </EuiTab>
            <EuiTab
              isSelected={activeTab === 'url'}
              onClick={() => {
                setActiveTab('url');
                clearResults();
              }}
            >
              URL
            </EuiTab>
          </EuiTabs>
          <EuiSpacer size="m" />
          {activeTab === 'pem' ? pemInput : null}
          {activeTab === 'file' ? fileInput : null}
          {activeTab === 'url' ? urlInput : null}

          {activeTab !== 'file' ? (
            <>
              <EuiSpacer size="s" />
              <EuiButton
                size="s"
                onClick={activeTab === 'pem' ? () => handleParsePem(pemContent) : handleFetchUrl}
                isLoading={parseStatus?.status === 'pending'}
                isDisabled={!canParse}
              >
                {activeTab === 'url' ? 'Fetch certificates' : 'Parse certificates'}
              </EuiButton>
            </>
          ) : null}
        </EuiForm>

        {parseError ? (
          <>
            <EuiSpacer size="m" />
            <EuiCallOut title="Error" color="danger" iconType="warning" size="s">
              <p>{parseError}</p>
            </EuiCallOut>
          </>
        ) : null}

        {certificates.length > 0 ? (
          <>
            <EuiSpacer size="m" />
            <EuiText size="s">
              <strong>
                {certificates.length} certificate{certificates.length !== 1 ? 's' : ''} found
              </strong>
            </EuiText>
            <EuiSpacer size="s" />
            <CertificateImportPreview
              certificates={certificates}
              selections={selections}
              onSelectionsChange={setSelections}
            />
          </>
        ) : null}
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={() => onClose()}>Cancel</EuiButtonEmpty>
        <EuiButton fill onClick={handleImport} isLoading={importStatus?.status === 'pending'} isDisabled={!canImport}>
          Import {selectedCerts.length > 0 ? `(${selectedCerts.length})` : ''}
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
