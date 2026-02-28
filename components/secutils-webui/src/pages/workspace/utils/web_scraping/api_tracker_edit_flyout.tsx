import type { EuiSwitchEvent } from '@elastic/eui';
import {
  EuiComboBox,
  EuiDescribedFormGroup,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiForm,
  EuiFormRow,
  EuiLink,
  EuiRange,
  EuiSelect,
  EuiSwitch,
  EuiText,
  EuiTitle,
} from '@elastic/eui';
import type { ChangeEvent } from 'react';
import { useCallback, useEffect, useState } from 'react';

import type { ApiTracker, ApiTrackerTarget } from './api_tracker';
import { ApiTrackerTestPanel } from './api_tracker_test_panel';
import type { RetryInterval } from './consts';
import {
  getDefaultRetryStrategy,
  getRetryIntervals,
  getScheduleMinInterval,
  PAGE_TRACKER_CUSTOM_SCHEDULE,
} from './consts';
import { areSchedulerJobsEqual } from './page_tracker';
import type { SchedulerJobConfig } from './page_tracker';
import { PageTrackerJobSchedule } from './page_tracker_job_schedule';
import { PageTrackerRetryStrategy } from './page_tracker_retry_strategy';
import { useFormChanges, useRangeTicks } from '../../../../hooks';
import {
  type AsyncData,
  getApiRequestConfig,
  getApiUrl,
  getErrorMessage,
  isClientError,
  ResponseError,
} from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { ScriptEditor } from '../../components/script_editor';
import { useWorkspaceContext } from '../../hooks';

export interface Props {
  onClose: (success?: boolean) => void;
  tracker?: Partial<ApiTracker>;
}

const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

const isHeaderValid = (header: string) => {
  return header.length >= 3 && header.includes(':') && !header.startsWith(':') && !header.endsWith(':');
};

function getBodyLanguage(headers: Array<{ label: string }>): string {
  const contentType = headers
    .find((h) => h.label.toLowerCase().startsWith('content-type:'))
    ?.label.split(':')[1]
    ?.trim()
    .toLowerCase();
  if (!contentType) return 'json';
  if (contentType.includes('html')) return 'html';
  if (contentType.includes('json')) return 'json';
  if (contentType.includes('javascript')) return 'javascript';
  if (contentType.includes('css')) return 'css';
  return 'plaintext';
}

const API_TRACKER_EXTRACTOR_TYPE_DEFS = `
interface ExtractorContext {
  tags: string[];
  previousContent?: { original: unknown };
  responses?: Array<{
    status: number;
    headers: Record<string, string>;
    body: number[];
  }>;
  params?: { secrets?: Record<string, string> };
}
interface ExtractorResult {
  body?: Uint8Array;
}
declare const context: ExtractorContext;
declare namespace Deno { namespace core { function encode(input: string): Uint8Array; function decode(input: Uint8Array): string; } }
`;

const API_TRACKER_CONFIGURATOR_TYPE_DEFS = `
interface ConfiguratorRequest {
  url: string;
  method?: string;
  headers?: Record<string, string>;
  body?: number[];
  mediaType?: string;
  acceptStatuses?: number[];
  acceptInvalidCertificates?: boolean;
}
interface ConfiguratorResponse {
  status: number;
  headers: Record<string, string>;
  body: number[];
}
interface ConfiguratorContext {
  tags: string[];
  previousContent?: { original: unknown };
  requests: ConfiguratorRequest[];
  params?: { secrets?: Record<string, string> };
}
type ConfiguratorResult = { requests: ConfiguratorRequest[] } | { responses: ConfiguratorResponse[] };
declare const context: ConfiguratorContext;
declare namespace Deno { namespace core { function encode(input: string): Uint8Array; function decode(input: Uint8Array): string; } }
`;

const DEFAULT_EXTRACTOR_SCRIPT = '';

export function ApiTrackerEditFlyout({ onClose, tracker }: Props) {
  const { addToast, uiState } = useWorkspaceContext();
  const maxTicks = useRangeTicks();

  const newTracker = !tracker?.id;
  const target = tracker?.retrack?.target;

  const [isAdvancedMode, setIsAdvancedMode] = useState(
    !newTracker &&
      (!!target?.acceptInvalidCertificates ||
        !!target?.mediaType ||
        !!target?.configurator ||
        (!!tracker?.secrets && tracker.secrets.type !== 'none')),
  );

  const [name, setName] = useState<string>(tracker?.name ?? '');
  const onNameChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
  }, []);

  const [url, setUrl] = useState<string>(target?.url ?? '');
  const onUrlChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setUrl(e.target.value);
  }, []);

  const [method, setMethod] = useState<string>(target?.method ?? 'GET');
  const onMethodChange = useCallback((e: ChangeEvent<HTMLSelectElement>) => {
    setMethod(e.target.value);
  }, []);

  const [headers, setHeaders] = useState<Array<{ label: string }>>(
    target?.headers
      ? Object.entries(target.headers).map(([k, v]) => ({ label: `${k}: ${v}` }))
      : [{ label: 'Content-Type: application/json' }],
  );
  const [areHeadersInvalid, setAreHeadersInvalid] = useState(false);

  const onCreateHeader = useCallback((headerValue: string) => {
    if (!isHeaderValid(headerValue)) {
      return false;
    }
    setHeaders((prev) => [...prev, { label: headerValue }]);
  }, []);

  const onHeadersSearchChange = useCallback((headerValue: string) => {
    if (!headerValue) {
      setAreHeadersInvalid(false);
      return;
    }
    setAreHeadersInvalid(!isHeaderValid(headerValue));
  }, []);

  const onHeadersChange = useCallback((selectedHeaders: Array<{ label: string }>) => {
    setHeaders(selectedHeaders);
    setAreHeadersInvalid(false);
  }, []);

  const [body, setBody] = useState<string>(() => {
    if (target?.body === undefined) return '';
    return typeof target.body === 'string' ? target.body : JSON.stringify(target.body, null, 2);
  });
  const onBodyChange = useCallback((value?: string) => {
    setBody(value ?? '');
  }, []);

  const [acceptInvalidCerts, setAcceptInvalidCerts] = useState<boolean>(!!target?.acceptInvalidCertificates);
  const [mediaType, setMediaType] = useState<string>(target?.mediaType ?? '');

  const [extractor, setExtractor] = useState<string>(target?.extractor ?? DEFAULT_EXTRACTOR_SCRIPT);
  const onExtractorChange = useCallback((value?: string) => {
    setExtractor(value ?? '');
  }, []);

  const [configurator, setConfigurator] = useState<string>(target?.configurator ?? '');
  const onConfiguratorChange = useCallback((value?: string) => {
    setConfigurator(value ?? '');
  }, []);

  const [revisions, setRevisions] = useState<number>(tracker ? (tracker.retrack?.config?.revisions ?? 0) : 3);
  const [enabled, setEnabled] = useState<boolean>(tracker?.retrack?.enabled ?? true);

  const [jobConfig, setJobConfig] = useState<SchedulerJobConfig | null>(tracker?.retrack?.config?.job ?? null);
  const [retryIntervals, setRetryIntervals] = useState<RetryInterval[]>(
    jobConfig?.schedule ? getRetryIntervals(getScheduleMinInterval(jobConfig.schedule)) : [],
  );

  const [notifications, setNotifications] = useState<boolean>(tracker ? !!tracker.retrack?.notifications : false);

  const existingSecrets = tracker?.secrets;
  const [secretsMode, setSecretsMode] = useState<'none' | 'all' | 'selected'>(existingSecrets?.type ?? 'none');
  const [selectedSecretNames, setSelectedSecretNames] = useState<Array<{ label: string }>>(
    existingSecrets?.type === 'selected' ? (existingSecrets.secrets ?? []).map((s) => ({ label: s })) : [],
  );
  const [availableSecrets, setAvailableSecrets] = useState<Array<{ label: string }>>([]);
  const [secretsLoaded, setSecretsLoaded] = useState(false);

  useEffect(() => {
    if (secretsMode !== 'selected' || secretsLoaded) return;
    fetch(getApiUrl('/api/user/secrets'), getApiRequestConfig())
      .then(async (res) => {
        if (res.ok) {
          const data: Array<{ name: string }> = await res.json();
          setAvailableSecrets(data.map((s) => ({ label: s.name })));
        }
      })
      .catch(() => {})
      .finally(() => setSecretsLoaded(true));
  }, [secretsMode, secretsLoaded]);

  const isDuplicate = !!tracker && !tracker.id;
  const hasFormChanges = useFormChanges({
    name,
    url,
    method,
    headers,
    body,
    acceptInvalidCerts,
    mediaType,
    extractor,
    configurator,
    revisions,
    enabled,
    jobConfig,
    notifications,
    secretsMode,
    selectedSecretNames,
  });
  const hasChanges = isDuplicate || hasFormChanges;

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();
  const onSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

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

    const trackerToUpdate = {
      name: !!name && (newTracker || tracker?.name !== name) ? name : null,
      enabled: newTracker || tracker?.retrack?.enabled !== enabled ? enabled : null,
      config:
        newTracker ||
        revisions !== tracker?.retrack?.config?.revisions ||
        !areSchedulerJobsEqual(tracker?.retrack?.config?.job, jobConfig)
          ? { revisions, job: jobConfig }
          : null,
      target: {
        url: url || undefined,
        method: method !== 'GET' ? method : undefined,
        headers: headersObj,
        body: parsedBody,
        mediaType: mediaType || undefined,
        acceptInvalidCertificates: acceptInvalidCerts || undefined,
        configurator: configurator || undefined,
        extractor: extractor || undefined,
      } as ApiTrackerTarget,
      notifications,
      secrets:
        secretsMode === 'none'
          ? { type: 'none' as const }
          : secretsMode === 'all'
            ? { type: 'all' as const }
            : { type: 'selected' as const, secrets: selectedSecretNames.map((s) => s.label) },
    };

    const requestInit = { ...getApiRequestConfig(), body: JSON.stringify(trackerToUpdate) };
    const [requestPromise, successMessage, errorMessage] = tracker?.id
      ? [
          fetch(getApiUrl(`/api/utils/web_scraping/api/${tracker.id}`), { ...requestInit, method: 'PUT' }),
          `Successfully updated "${name}" API tracker`,
          `Unable to update "${name}" API tracker, please try again later`,
        ]
      : [
          fetch(getApiUrl('/api/utils/web_scraping/api'), { ...requestInit, method: 'POST' }),
          `Successfully saved "${name}" API tracker`,
          `Unable to save "${name}" API tracker, please try again later`,
        ];
    requestPromise
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        setUpdatingStatus({ status: 'succeeded', data: undefined });

        addToast({
          id: `success-save-api-tracker-${name}`,
          iconType: 'check',
          color: 'success',
          title: successMessage,
        });

        onClose(true);
      })
      .catch((err: Error) => {
        const remoteErrorMessage = getErrorMessage(err);
        setUpdatingStatus({ status: 'failed', error: remoteErrorMessage });

        addToast({
          id: `failed-save-api-tracker-${name}`,
          iconType: 'warning',
          color: 'danger',
          title: isClientError(err) ? remoteErrorMessage : errorMessage,
        });
      });
  }, [
    name,
    url,
    method,
    headers,
    body,
    acceptInvalidCerts,
    mediaType,
    extractor,
    configurator,
    revisions,
    enabled,
    jobConfig,
    notifications,
    secretsMode,
    selectedSecretNames,
    tracker,
    updatingStatus,
    addToast,
    onClose,
    newTracker,
  ]);

  const notificationsRow = jobConfig ? (
    <EuiFormRow
      label={'Notifications'}
      helpText={'Send an email notification when a change is detected or a check fails.'}
    >
      <EuiSwitch
        showLabel={false}
        label="Notification on change"
        checked={notifications}
        onChange={(e) => setNotifications(e.target.checked)}
      />
    </EuiFormRow>
  ) : null;

  const supportsCustomSchedule =
    !uiState.subscription?.features?.webScraping.trackerSchedules ||
    uiState.subscription.features.webScraping.trackerSchedules.includes(PAGE_TRACKER_CUSTOM_SCHEDULE);
  const scheduleHelpText = supportsCustomSchedule ? (
    <span>
      How often the API should be checked for changes. By default, automatic checks are disabled and can be initiated
      manually. Custom schedules can be set using a cron expression. Refer to the{' '}
      <EuiLink target="_blank" href="/docs/guides/web_scraping/api#annex-custom-cron-schedules">
        <b>documentation</b>
      </EuiLink>{' '}
      for supported cron expression formats and examples
    </span>
  ) : (
    <>
      How often the API should be checked for changes. By default, automatic checks are disabled and can be initiated
      manually
    </>
  );

  const maxTrackerRevisions = uiState.subscription?.features?.webScraping.trackerRevisions ?? 0;
  const tickInterval = Math.ceil(maxTrackerRevisions / maxTicks);

  const headerOptions = [
    { label: 'Content-Type: application/json' },
    { label: 'Content-Type: text/plain; charset=utf-8' },
    { label: 'Content-Type: text/html; charset=utf-8' },
    { label: 'Accept: application/json' },
  ];

  return (
    <EditorFlyout
      title={
        <EuiFlexGroup>
          <EuiFlexItem>
            <EuiTitle size="s">
              <h1>{`${tracker ? 'Edit' : 'Add'} API tracker`}</h1>
            </EuiTitle>
          </EuiFlexItem>
          <EuiFlexItem grow={false}>
            <EuiSwitch
              label={
                <EuiText color={'subdued'} size={'s'}>
                  Advanced mode
                </EuiText>
              }
              checked={isAdvancedMode}
              onChange={(e) => setIsAdvancedMode(e.target.checked)}
            />
          </EuiFlexItem>
        </EuiFlexGroup>
      }
      onClose={() => onClose()}
      onSave={onSave}
      hasChanges={hasChanges}
      canSave={!!name && !!url && !areHeadersInvalid && (hasFormChanges || isDuplicate)}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description={'General properties of the API tracker'}>
          <EuiFormRow label="Name" helpText="Arbitrary API tracker name." fullWidth>
            <EuiFieldText value={name} required type={'text'} onChange={onNameChange} />
          </EuiFormRow>
          <EuiFormRow label="Revisions" helpText="Tracker will persist only specified number of revisions">
            <EuiRange
              min={0}
              max={maxTrackerRevisions}
              value={revisions}
              fullWidth
              onChange={(e) => setRevisions(+e.currentTarget.value)}
              showRange
              showTicks
              tickInterval={tickInterval > 1 ? Math.ceil(tickInterval / 5) * 5 : tickInterval}
              showValue={maxTrackerRevisions > maxTicks}
            />
          </EuiFormRow>
          <EuiFormRow label="Enabled" helpText="Disable the tracker to pause scheduled checks.">
            <EuiSwitch
              showLabel={false}
              label="Enable tracker"
              checked={enabled}
              onChange={(e: EuiSwitchEvent) => setEnabled(e.target.checked)}
            />
          </EuiFormRow>
        </EuiDescribedFormGroup>

        <EuiDescribedFormGroup title={<h3>Request</h3>} description={'Properties of the HTTP request to track'}>
          <EuiFormRow label="URL" helpText="The API endpoint URL to track" fullWidth>
            <EuiFieldText
              value={url}
              required
              type={'text'}
              onChange={onUrlChange}
              placeholder="https://api.example.com/data"
            />
          </EuiFormRow>
          <EuiFormRow label="Method" helpText="HTTP method for the request">
            <EuiSelect
              options={HTTP_METHODS.map((m) => ({ value: m, text: m }))}
              value={method}
              onChange={onMethodChange}
            />
          </EuiFormRow>
          <EuiFormRow label="Headers" helpText="Optional HTTP headers, e.g. Content-Type: application/json" fullWidth>
            <EuiComboBox
              fullWidth
              options={headerOptions}
              selectedOptions={headers}
              onCreateOption={onCreateHeader}
              onChange={onHeadersChange}
              onSearchChange={onHeadersSearchChange}
              isInvalid={areHeadersInvalid}
            />
          </EuiFormRow>
          {method !== 'GET' && method !== 'HEAD' ? (
            <EuiFormRow label="Body" helpText="Request body (JSON or plain text depending on Content-Type header)">
              <ScriptEditor onChange={onBodyChange} defaultValue={body} language={getBodyLanguage(headers)} />
            </EuiFormRow>
          ) : null}
          {isAdvancedMode ? (
            <>
              <EuiFormRow
                label="Accept invalid certificates"
                helpText="Allow connections to servers with invalid or self-signed SSL certificates"
              >
                <EuiSwitch
                  showLabel={false}
                  label="Accept invalid certificates"
                  checked={acceptInvalidCerts}
                  onChange={(e: EuiSwitchEvent) => setAcceptInvalidCerts(e.target.checked)}
                />
              </EuiFormRow>
              <EuiFormRow label="Media type" helpText="Override the request Content-Type">
                <EuiFieldText
                  value={mediaType}
                  type={'text'}
                  onChange={(e) => setMediaType(e.target.value)}
                  placeholder="application/json"
                />
              </EuiFormRow>
            </>
          ) : null}
          <EuiFormRow
            label="Test"
            helpText="Send the configured HTTP request and inspect the response before saving."
            fullWidth
          >
            <ApiTrackerTestPanel
              url={url}
              method={method}
              headers={headers}
              body={body}
              mediaType={mediaType}
              acceptInvalidCertificates={acceptInvalidCerts}
            />
          </EuiFormRow>
        </EuiDescribedFormGroup>

        <EuiDescribedFormGroup
          title={<h3>Change tracking</h3>}
          description={
            'Properties defining how frequently the API should be checked for changes and how those changes should be reported'
          }
        >
          <EuiFormRow label="Frequency" helpText={scheduleHelpText}>
            <PageTrackerJobSchedule
              schedule={jobConfig?.schedule}
              onChange={(schedule, retryIntervalsFromSchedule) => {
                if (schedule === '' && jobConfig) {
                  setJobConfig({ ...jobConfig, schedule });
                  return;
                }
                if (schedule === null) {
                  setJobConfig(null);
                } else if (schedule !== jobConfig?.schedule) {
                  setJobConfig({
                    retryStrategy:
                      retryIntervalsFromSchedule.length > 0
                        ? getDefaultRetryStrategy(retryIntervalsFromSchedule)
                        : undefined,
                    schedule,
                  });
                }
                setRetryIntervals(retryIntervalsFromSchedule);
              }}
            />
          </EuiFormRow>
          {notificationsRow}
        </EuiDescribedFormGroup>
        {jobConfig ? (
          <EuiDescribedFormGroup
            title={<h3>Retries</h3>}
            description={'Properties defining how failed automatic checks should be retried'}
          >
            <PageTrackerRetryStrategy
              strategy={jobConfig.retryStrategy}
              intervals={retryIntervals}
              onChange={(newStrategy) => {
                if (jobConfig) {
                  setJobConfig({ ...jobConfig, retryStrategy: newStrategy ?? undefined });
                }
              }}
            />
          </EuiDescribedFormGroup>
        ) : null}

        <EuiDescribedFormGroup
          title={<h3>Scripts</h3>}
          description={'Custom JavaScript scripts for extracting and configuring API requests'}
        >
          <EuiFormRow
            label="Data extractor"
            helpText={
              <span>
                An IIFE script that receives API responses via the global &quot;context&quot; object and returns{' '}
                {'{ body: Deno.core.encode(...) }'}. No <code>fetch</code> is available. Refer to the{' '}
                <EuiLink target="_blank" href="/docs/guides/web_scraping/api#annex-extractor-script">
                  <b>documentation</b>
                </EuiLink>{' '}
                for examples.
              </span>
            }
          >
            <ScriptEditor
              onChange={onExtractorChange}
              defaultValue={extractor}
              extraLibs={[{ content: API_TRACKER_EXTRACTOR_TYPE_DEFS, filePath: 'ts:api-tracker-extractor.d.ts' }]}
            />
          </EuiFormRow>
          {isAdvancedMode ? (
            <EuiFormRow
              label="Request configurator"
              helpText={
                <span>
                  Optional IIFE script to dynamically configure requests before they are sent. Refer to the{' '}
                  <EuiLink target="_blank" href="/docs/guides/web_scraping/api#annex-configurator-script">
                    <b>documentation</b>
                  </EuiLink>{' '}
                  for details.
                </span>
              }
            >
              <ScriptEditor
                onChange={onConfiguratorChange}
                defaultValue={configurator}
                extraLibs={[
                  { content: API_TRACKER_CONFIGURATOR_TYPE_DEFS, filePath: 'ts:api-tracker-configurator.d.ts' },
                ]}
              />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>

        <EuiDescribedFormGroup
          title={<h3>Secrets</h3>}
          description="Control which user secrets are available to this tracker's scripts."
        >
          <EuiFormRow label="Access mode" helpText="Choose which secrets to expose to this tracker." fullWidth>
            <EuiSelect
              fullWidth
              options={[
                { value: 'none', text: 'No secrets' },
                { value: 'all', text: 'All secrets' },
                { value: 'selected', text: 'Selected secrets' },
              ]}
              value={secretsMode}
              onChange={(e) => setSecretsMode(e.target.value as 'none' | 'all' | 'selected')}
            />
          </EuiFormRow>
          {secretsMode === 'selected' ? (
            <EuiFormRow label="Secrets" helpText="Select the secrets to expose." fullWidth>
              <EuiComboBox
                fullWidth
                options={availableSecrets}
                selectedOptions={selectedSecretNames}
                onChange={setSelectedSecretNames}
                isLoading={!secretsLoaded}
              />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
