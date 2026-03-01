import {
  EuiBadge,
  EuiCallOut,
  EuiCodeBlock,
  EuiFlexGroup,
  EuiFlexItem,
  EuiLoadingSpinner,
  EuiModal,
  EuiModalBody,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSpacer,
  EuiStepsHorizontal,
  EuiTabbedContent,
  EuiText,
} from '@elastic/eui';
import { css } from '@emotion/react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type { ApiTrackerTarget } from './api_tracker';
import type { ApiRequestDebugInfo, DebugResult, PipelineStage, ScriptDebugInfo } from './api_tracker_debug_types';
import { buildPipelineStages } from './api_tracker_debug_types';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';

export interface ApiTrackerDebugPanelProps {
  isOpen: boolean;
  onClose: () => void;
  url: string;
  method: string;
  headers: Array<{ label: string }>;
  body: string;
  mediaType: string;
  acceptInvalidCertificates: boolean;
  configurator?: string;
  extractor?: string;
  secrets: { type: string; secrets?: string[] };
  onStatusChange?: (status: 'idle' | 'pending' | 'done') => void;
}

function statusColor(status: number): 'success' | 'warning' | 'danger' | 'default' {
  if (status >= 200 && status < 300) return 'success';
  if (status >= 300 && status < 400) return 'warning';
  return 'danger';
}

function formatJson(value: unknown): string {
  if (typeof value === 'string') {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }
  return JSON.stringify(value, null, 2);
}

function detectLanguage(raw?: string, headers?: Record<string, string>): string {
  const ct = headers?.['content-type'] ?? '';
  if (ct.includes('html')) return 'html';
  if (ct.includes('xml')) return 'xml';
  if (ct.includes('css')) return 'css';
  if (ct.includes('javascript')) return 'javascript';
  if (raw) {
    try {
      JSON.parse(raw);
      return 'json';
    } catch {
      /* not json */
    }
  }
  return 'text';
}

function stageTitle(stage: PipelineStage, totalRequests: number): string {
  switch (stage.kind) {
    case 'configurator':
      return 'Configurator';
    case 'request':
      return totalRequests > 1 ? `Request #${stage.data.index + 1}` : 'Request';
    case 'extractor':
      return 'Extractor';
    case 'result':
      return 'Result';
  }
}

function stageStatus(stage: PipelineStage, debugResult: DebugResult): 'complete' | 'danger' | 'incomplete' {
  switch (stage.kind) {
    case 'configurator':
    case 'extractor':
      return stage.data.error ? 'danger' : 'complete';
    case 'request':
      return stage.data.error ? 'danger' : 'complete';
    case 'result':
      return debugResult.error ? 'danger' : debugResult.result != null ? 'complete' : 'incomplete';
  }
}

// ---------------------------------------------------------------------------
// Detail sub-components
// ---------------------------------------------------------------------------

function ScriptDetail({ label, data, params }: { label: string; data: ScriptDebugInfo; params?: unknown }) {
  const tabs = useMemo(() => {
    const result: Array<{ id: string; name: string; content: React.ReactNode }> = [];

    result.push({
      id: 'result',
      name: 'Result',
      content: (
        <>
          <EuiSpacer size="s" />
          {data.error ? (
            <EuiCallOut title={`${label} script failed`} color="danger" iconType="error" size="s">
              <p>{data.error}</p>
            </EuiCallOut>
          ) : data.result != null ? (
            <EuiCodeBlock language="json" fontSize="s" paddingSize="s" overflowHeight={300} isCopyable>
              {formatJson(data.result)}
            </EuiCodeBlock>
          ) : (
            <EuiText size="s" color="subdued">
              No result produced.
            </EuiText>
          )}
        </>
      ),
    });

    if (params != null) {
      result.push({
        id: 'params',
        name: 'Params',
        content: (
          <>
            <EuiSpacer size="s" />
            <EuiCodeBlock language="json" fontSize="s" paddingSize="s" overflowHeight={300} isCopyable>
              {formatJson(params)}
            </EuiCodeBlock>
          </>
        ),
      });
    }

    return result;
  }, [data, label, params]);

  return (
    <>
      <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
        <EuiFlexItem grow={false}>
          <EuiBadge color="hollow">{data.durationMs}ms</EuiBadge>
        </EuiFlexItem>
      </EuiFlexGroup>
      <EuiSpacer size="s" />
      <EuiTabbedContent tabs={tabs} size="s" autoFocus="selected" />
    </>
  );
}

function RequestDetail({ data }: { data: ApiRequestDebugInfo }) {
  const lang = detectLanguage(data.responseBodyRaw, data.responseHeaders);

  const tabs = useMemo(() => {
    const result: Array<{ id: string; name: string; content: React.ReactNode }> = [];

    result.push({
      id: 'response-body',
      name: 'Response Body',
      content: (
        <>
          <EuiSpacer size="s" />
          {data.autoParse ? (
            <>
              <EuiBadge color={data.autoParse.success ? 'success' : 'warning'}>
                Parsed as {data.autoParse.mediaType}
                {data.autoParse.error ? ` - ${data.autoParse.error}` : ''}
              </EuiBadge>
              <EuiSpacer size="s" />
            </>
          ) : null}
          {data.responseBodyRaw != null ? (
            <>
              <EuiCodeBlock language={lang} fontSize="s" paddingSize="s" overflowHeight={300} isCopyable>
                {lang === 'json' ? formatJson(data.responseBodyRaw) : data.responseBodyRaw}
              </EuiCodeBlock>
              {data.responseBodyRawSize != null && data.responseBodyRaw.length < data.responseBodyRawSize ? (
                <EuiText size="xs" color="subdued">
                  Showing {data.responseBodyRaw.length} of {data.responseBodyRawSize} bytes
                </EuiText>
              ) : null}
            </>
          ) : (
            <EuiText size="s" color="subdued">
              No response body.
            </EuiText>
          )}
        </>
      ),
    });

    if (data.responseHeaders) {
      result.push({
        id: 'response-headers',
        name: 'Response Headers',
        content: (
          <>
            <EuiSpacer size="s" />
            <EuiCodeBlock
              language="http"
              fontSize="s"
              paddingSize="s"
              overflowHeight={300}
              whiteSpace="pre-wrap"
              isCopyable
              css={css`
                & code {
                  white-space: pre-wrap !important;
                  word-break: break-word;
                }
              `}
            >
              {Object.entries(data.responseHeaders)
                .map(([k, v]) => `${k}: ${v}`)
                .join('\n')}
            </EuiCodeBlock>
          </>
        ),
      });
    }

    if (data.requestHeaders) {
      result.push({
        id: 'request-headers',
        name: 'Request Headers',
        content: (
          <>
            <EuiSpacer size="s" />
            <EuiCodeBlock
              language="http"
              fontSize="s"
              paddingSize="s"
              overflowHeight={300}
              whiteSpace="pre-wrap"
              isCopyable
            >
              {Object.entries(data.requestHeaders)
                .map(([k, v]) => `${k}: ${v}`)
                .join('\n')}
            </EuiCodeBlock>
          </>
        ),
      });
    }

    if (data.requestBody != null) {
      result.push({
        id: 'request-body',
        name: 'Request Body',
        content: (
          <>
            <EuiSpacer size="s" />
            <EuiCodeBlock language="json" fontSize="s" paddingSize="s" overflowHeight={300} isCopyable>
              {formatJson(data.requestBody)}
            </EuiCodeBlock>
          </>
        ),
      });
    }

    return result;
  }, [data, lang]);

  return (
    <>
      <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false} wrap>
        {data.statusCode != null ? (
          <EuiFlexItem grow={false}>
            <EuiBadge color={statusColor(data.statusCode)}>{data.statusCode}</EuiBadge>
          </EuiFlexItem>
        ) : null}
        <EuiFlexItem grow={false}>
          <EuiText size="s">
            <strong>{data.method ?? 'GET'}</strong> {data.url ?? ''}
          </EuiText>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiBadge color="hollow">{data.durationMs}ms</EuiBadge>
        </EuiFlexItem>
        {data.source !== 'original' ? (
          <EuiFlexItem grow={false}>
            <EuiBadge color="accent">{data.source}</EuiBadge>
          </EuiFlexItem>
        ) : null}
      </EuiFlexGroup>

      {data.error ? (
        <>
          <EuiSpacer size="s" />
          <EuiCallOut title="Request failed" color="danger" iconType="error" size="s">
            <p>{data.error}</p>
          </EuiCallOut>
        </>
      ) : null}

      <EuiSpacer size="s" />
      <EuiTabbedContent tabs={tabs} size="s" autoFocus="selected" />
    </>
  );
}

function ConfiguratorDetail({ data, params }: { data: ScriptDebugInfo; params?: unknown }) {
  return <ScriptDetail label="Configurator" data={data} params={params} />;
}

function ExtractorDetail({ data, params }: { data: ScriptDebugInfo; params?: unknown }) {
  return <ScriptDetail label="Extractor" data={data} params={params} />;
}

function ResultDetail({ debugResult }: { debugResult: DebugResult }) {
  return (
    <>
      <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
        <EuiFlexItem grow={false}>
          <EuiBadge color="hollow">{debugResult.durationMs}ms total</EuiBadge>
        </EuiFlexItem>
      </EuiFlexGroup>
      {debugResult.error ? (
        <>
          <EuiSpacer size="s" />
          <EuiCallOut title="Pipeline failed" color="danger" iconType="error" size="s">
            <p>{debugResult.error}</p>
          </EuiCallOut>
        </>
      ) : null}
      {debugResult.result != null ? (
        <>
          <EuiSpacer size="s" />
          <EuiCodeBlock language="json" fontSize="s" paddingSize="s" overflowHeight={300} isCopyable>
            {formatJson(debugResult.result)}
          </EuiCodeBlock>
        </>
      ) : !debugResult.error ? (
        <>
          <EuiSpacer size="s" />
          <EuiText size="s" color="subdued">
            No result produced.
          </EuiText>
        </>
      ) : null}
    </>
  );
}

function StageDetailPanel({ stage, debugResult }: { stage: PipelineStage | null; debugResult: DebugResult }) {
  if (!stage) return null;
  const params = debugResult.target.params;
  switch (stage.kind) {
    case 'configurator':
      return <ConfiguratorDetail data={stage.data} params={params} />;
    case 'request':
      return <RequestDetail data={stage.data} />;
    case 'extractor':
      return <ExtractorDetail data={stage.data} params={params} />;
    case 'result':
      return <ResultDetail debugResult={debugResult} />;
  }
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function ApiTrackerDebugPanel({
  isOpen,
  onClose,
  url,
  method,
  headers,
  body,
  mediaType,
  acceptInvalidCertificates,
  configurator,
  extractor,
  secrets,
  onStatusChange,
}: ApiTrackerDebugPanelProps) {
  const [result, setResult] = useState<AsyncData<DebugResult>>();
  const [selectedStageIndex, setSelectedStageIndex] = useState<number>(-1);

  const stages = useMemo(() => (result?.status === 'succeeded' ? buildPipelineStages(result.data) : []), [result]);

  const runDebug = useCallback(() => {
    if (result?.status === 'pending') return;

    setResult({ status: 'pending' });
    onStatusChange?.('pending');

    const headersObj =
      headers.length > 0
        ? Object.fromEntries(
            headers.map((h) => {
              const [k, ...rest] = h.label.split(':');
              return [k.trim(), rest.join(':').trim()];
            }),
          )
        : undefined;

    let parsedBody: unknown = undefined;
    if (body && method !== 'GET' && method !== 'HEAD') {
      try {
        parsedBody = JSON.parse(body);
      } catch {
        parsedBody = body;
      }
    }

    const target: ApiTrackerTarget = {
      url,
      method: method !== 'GET' ? method : undefined,
      headers: headersObj,
      body: parsedBody,
      mediaType: mediaType || undefined,
      acceptInvalidCertificates: acceptInvalidCertificates || undefined,
      configurator: configurator || undefined,
      extractor: extractor || undefined,
    };

    fetch(getApiUrl('/api/utils/web_scraping/api/debug'), {
      ...getApiRequestConfig(),
      method: 'POST',
      body: JSON.stringify({ target, secrets }),
    })
      .then(async (res) => {
        if (!res.ok) throw await ResponseError.fromResponse(res);
        const data: DebugResult = await res.json();
        setResult({ status: 'succeeded', data });

        const newStages = buildPipelineStages(data);
        setSelectedStageIndex(newStages.length - 1);
        onStatusChange?.('done');
      })
      .catch((err: Error) => {
        setResult({ status: 'failed', error: getErrorMessage(err) });
        onStatusChange?.('done');
      });
  }, [
    url,
    method,
    headers,
    body,
    mediaType,
    acceptInvalidCertificates,
    configurator,
    extractor,
    secrets,
    result?.status,
    onStatusChange,
  ]);

  useEffect(() => {
    if (isOpen && !result) {
      runDebug();
    }
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  const debugData = result?.status === 'succeeded' ? result.data : null;
  const totalRequests = debugData?.target.requests.length ?? 0;

  const horizontalSteps = useMemo(
    () =>
      stages.map((stage, i) => ({
        title: stageTitle(stage, totalRequests),
        status:
          i === selectedStageIndex
            ? ('current' as const)
            : debugData
              ? stageStatus(stage, debugData)
              : ('incomplete' as const),
        onClick: () => {
          setSelectedStageIndex(i);
        },
      })),
    [stages, selectedStageIndex, debugData, totalRequests],
  );

  const selectedStage: PipelineStage | null = useMemo(() => {
    return selectedStageIndex >= 0 && selectedStageIndex < stages.length ? stages[selectedStageIndex] : null;
  }, [selectedStageIndex, stages]);

  const handleClose = useCallback(() => {
    setResult(undefined);
    setSelectedStageIndex(-1);
    onClose();
  }, [onClose]);

  if (!isOpen) return null;

  const modalCss = css`
    width: 75vw;
    max-width: 800px;
    min-height: min(60vh, 500px);
    display: flex;
    flex-direction: column;
  `;

  const bodyCss = css`
    flex: 1 1 auto;
    min-height: 0;
  `;

  return (
    <EuiModal onClose={handleClose} maxWidth={false} data-test-subj="debug-modal" css={modalCss}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>Debug</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody css={bodyCss}>
        {result?.status === 'pending' ? (
          <EuiFlexGroup
            alignItems="center"
            justifyContent="center"
            direction="column"
            gutterSize="m"
            responsive={false}
            css={css`
              flex: 1;
            `}
          >
            <EuiFlexItem grow={false}>
              <EuiLoadingSpinner size="xl" />
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiText size="s" color="subdued">
                Running debug pipeline…
              </EuiText>
            </EuiFlexItem>
          </EuiFlexGroup>
        ) : null}

        {result?.status === 'failed' ? (
          <EuiCallOut title="Debug request failed" color="danger" iconType="error" size="s">
            <p>{result.error}</p>
          </EuiCallOut>
        ) : null}

        {debugData ? (
          <>
            <EuiStepsHorizontal steps={horizontalSteps} size="xs" />
            <EuiSpacer size="m" />
            <StageDetailPanel stage={selectedStage} debugResult={debugData} />
          </>
        ) : null}
      </EuiModalBody>
    </EuiModal>
  );
}
