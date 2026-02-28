import {
  EuiBadge,
  EuiButtonEmpty,
  EuiCodeBlock,
  EuiFlexGroup,
  EuiFlexItem,
  EuiLoadingSpinner,
  EuiPanel,
  EuiSpacer,
  EuiTab,
  EuiTabs,
  EuiText,
} from '@elastic/eui';
import { css } from '@emotion/react';
import { useCallback, useState } from 'react';

import type { ApiTrackerTarget } from './api_tracker';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';

interface TestResult {
  status: number;
  headers: Record<string, string>;
  body: string;
  latencyMs: number;
}

export interface ApiTrackerTestPanelProps {
  url: string;
  method: string;
  headers: Array<{ label: string }>;
  body: string;
  mediaType: string;
  acceptInvalidCertificates: boolean;
}

function statusColor(status: number): 'success' | 'warning' | 'danger' | 'default' {
  if (status >= 200 && status < 300) return 'success';
  if (status >= 300 && status < 400) return 'warning';
  return 'danger';
}

function formatBody(body: string): string {
  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}

export function ApiTrackerTestPanel({
  url,
  method,
  headers,
  body,
  mediaType,
  acceptInvalidCertificates,
}: ApiTrackerTestPanelProps) {
  const [result, setResult] = useState<AsyncData<TestResult>>();
  const [selectedTab, setSelectedTab] = useState<'body' | 'headers'>('body');

  const onTest = useCallback(() => {
    if (result?.status === 'pending') return;
    setSelectedTab('body');
    setResult({ status: 'pending' });

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
    };

    fetch(getApiUrl('/api/utils/web_scraping/api/test'), {
      ...getApiRequestConfig(),
      method: 'POST',
      body: JSON.stringify({ target }),
    })
      .then(async (res) => {
        if (!res.ok) throw await ResponseError.fromResponse(res);
        const data: TestResult = await res.json();
        setResult({ status: 'succeeded', data });
      })
      .catch((err: Error) => {
        setResult({ status: 'failed', error: getErrorMessage(err) });
      });
  }, [url, method, headers, body, mediaType, acceptInvalidCertificates, result?.status]);

  return (
    <EuiPanel paddingSize="s" hasBorder>
      <EuiFlexGroup alignItems="center" gutterSize="s" responsive={false}>
        <EuiFlexItem grow={false}>
          <EuiButtonEmpty
            iconType="play"
            size="s"
            onClick={onTest}
            disabled={!url || result?.status === 'pending'}
            data-test-subj="test-request-button"
          >
            Test request
          </EuiButtonEmpty>
        </EuiFlexItem>
        {result?.status === 'pending' ? (
          <EuiFlexItem grow={false}>
            <EuiLoadingSpinner size="s" />
          </EuiFlexItem>
        ) : null}
        {result?.status === 'succeeded' ? (
          <>
            <EuiFlexItem grow={false}>
              <EuiBadge color={statusColor(result.data.status)}>{result.data.status}</EuiBadge>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiText size="xs" color="subdued">
                {result.data.latencyMs}ms
              </EuiText>
            </EuiFlexItem>
          </>
        ) : null}
        {result?.status === 'failed' ? (
          <EuiFlexItem grow={false}>
            <EuiText size="xs" color="danger">
              {result.error}
            </EuiText>
          </EuiFlexItem>
        ) : null}
      </EuiFlexGroup>

      {result?.status === 'succeeded' ? (
        <>
          <EuiSpacer size="s" />
          <EuiTabs size="s" bottomBorder={false}>
            <EuiTab isSelected={selectedTab === 'body'} onClick={() => setSelectedTab('body')}>
              Body
            </EuiTab>
            <EuiTab isSelected={selectedTab === 'headers'} onClick={() => setSelectedTab('headers')}>
              Headers ({Object.keys(result.data.headers).length})
            </EuiTab>
          </EuiTabs>
          <EuiSpacer size="s" />
          {selectedTab === 'body' ? (
            <EuiCodeBlock language="json" fontSize="s" paddingSize="s" overflowHeight={300} isCopyable>
              {formatBody(result.data.body)}
            </EuiCodeBlock>
          ) : (
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
              {Object.entries(result.data.headers)
                .map(([key, value]) => `${key}: ${value}`)
                .join('\n')}
            </EuiCodeBlock>
          )}
        </>
      ) : null}
    </EuiPanel>
  );
}
