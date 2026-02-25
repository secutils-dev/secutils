import { EuiDescribedFormGroup, EuiFieldText, EuiForm, EuiFormRow, EuiLink, EuiRange, EuiSwitch } from '@elastic/eui';
import type { ChangeEvent } from 'react';
import { useCallback, useState } from 'react';

import type { RetryInterval } from './consts';
import {
  getDefaultRetryStrategy,
  getRetryIntervals,
  getScheduleMinInterval,
  PAGE_TRACKER_CUSTOM_SCHEDULE,
} from './consts';
import type { PageTracker, SchedulerJobConfig } from './page_tracker';
import { areSchedulerJobsEqual } from './page_tracker';
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
  tracker?: Partial<PageTracker>;
}

export function PageTrackerEditFlyout({ onClose, tracker }: Props) {
  const { addToast, uiState } = useWorkspaceContext();
  const maxTicks = useRangeTicks();

  const newTracker = !tracker?.id;

  const [name, setName] = useState<string>(tracker?.name ?? '');
  const onNameChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
  }, []);
  const needsToSaveName = !!name && (newTracker || tracker?.name !== name);

  const [jobConfig, setJobConfig] = useState<SchedulerJobConfig | null>(tracker?.retrack?.config?.job ?? null);
  const [retryIntervals, setRetryIntervals] = useState<RetryInterval[]>(
    jobConfig?.schedule ? getRetryIntervals(getScheduleMinInterval(jobConfig.schedule)) : [],
  );
  const needsToSaveJobConfig =
    !tracker || newTracker || !areSchedulerJobsEqual(tracker?.retrack?.config?.job, jobConfig);

  const [extractorScript, setExtractorScript] = useState<string>(
    tracker?.retrack?.target?.extractor ??
      "export async function execute(page) {\n    // Load the page.\n    await page.goto('https://secutils.dev');\n\n    // Get the page title.\n    const title = await page.title();\n\n    // See what's possible at:\n    // https://playwright.dev/docs/api/class-page\n\n    // Return result as Markdown.\n    return `## ${title}`;\n};",
  );
  const onExtractContentScriptChange = useCallback((value?: string) => {
    setExtractorScript(value ?? '');
  }, []);
  const needsToSaveExtractorScript =
    !!extractorScript && (newTracker || tracker?.retrack?.target?.extractor !== extractorScript);

  const [revisions, setRevisions] = useState<number>(tracker ? (tracker.retrack?.config?.revisions ?? 0) : 3);
  const needsToSaveRevisions = newTracker || tracker?.retrack?.config?.revisions !== revisions;

  const [notifications, setNotifications] = useState<boolean>(tracker ? !!tracker.retrack?.notifications : false);
  const needsToSaveNotifications = newTracker || tracker?.retrack?.notifications !== notifications;

  const isDuplicate = !!tracker && !tracker.id;
  const hasFormChanges = useFormChanges({ name, jobConfig, extractorScript, revisions, notifications });
  const hasChanges = isDuplicate || hasFormChanges;

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();
  const onSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

    const trackerToUpdate = {
      name: !!name && (newTracker || tracker?.name !== name) ? name : null,
      config:
        newTracker ||
        revisions !== tracker?.retrack?.config?.revisions ||
        !areSchedulerJobsEqual(tracker?.retrack.config.job, jobConfig)
          ? { revisions, job: jobConfig }
          : null,
      target:
        !!extractorScript && (newTracker || tracker?.retrack?.target?.extractor !== extractorScript)
          ? { extractor: extractorScript }
          : null,
      notifications,
    };

    const requestInit = { ...getApiRequestConfig(), body: JSON.stringify(trackerToUpdate) };
    const [requestPromise, successMessage, errorMessage] = tracker?.id
      ? [
          fetch(getApiUrl(`/api/utils/web_scraping/page/${tracker.id}`), { ...requestInit, method: 'PUT' }),
          `Successfully updated "${name}" page tracker`,
          `Unable to update "${name}" page tracker, please try again later`,
        ]
      : [
          fetch(getApiUrl('/api/utils/web_scraping/page'), { ...requestInit, method: 'POST' }),
          `Successfully saved "${name}" page tracker`,
          `Unable to save "${name}" page tracker, please try again later`,
        ];
    requestPromise
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        setUpdatingStatus({ status: 'succeeded', data: undefined });

        addToast({
          id: `success-save-tracker-${name}`,
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
          id: `failed-save-tracker-${name}`,
          iconType: 'warning',
          color: 'danger',
          title: isClientError(err) ? remoteErrorMessage : errorMessage,
        });
      });
  }, [
    name,
    revisions,
    extractorScript,
    jobConfig,
    tracker,
    updatingStatus,
    addToast,
    notifications,
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

  // Link to the cron expression documentation only if it's allowed by the subscription.
  const supportsCustomSchedule =
    !uiState.subscription?.features?.webScraping.trackerSchedules ||
    uiState.subscription.features.webScraping.trackerSchedules.includes(PAGE_TRACKER_CUSTOM_SCHEDULE);
  const scheduleHelpText = supportsCustomSchedule ? (
    <span>
      How often the page should be checked for changes. By default, automatic checks are disabled and can be initiated
      manually. Custom schedules can be set using a cron expression. Refer to the{' '}
      <EuiLink target="_blank" href="/docs/guides/web_scraping/page#annex-custom-cron-schedules">
        <b>documentation</b>
      </EuiLink>{' '}
      for supported cron expression formats and examples
    </span>
  ) : (
    <>
      How often the page should be checked for changes. By default, automatic checks are disabled and can be initiated
      manually
    </>
  );

  const maxTrackerRevisions = uiState.subscription?.features?.webScraping.trackerRevisions ?? 0;
  const tickInterval = Math.ceil(maxTrackerRevisions / maxTicks);
  return (
    <EditorFlyout
      title={`${tracker ? 'Edit' : 'Add'} tracker`}
      onClose={() => onClose()}
      onSave={onSave}
      hasChanges={hasChanges}
      canSave={
        !!name &&
        !!extractorScript &&
        (needsToSaveName ||
          needsToSaveRevisions ||
          needsToSaveJobConfig ||
          needsToSaveExtractorScript ||
          needsToSaveNotifications)
      }
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description={'General properties of the page tracker'}>
          <EuiFormRow label="Name" helpText="Arbitrary page tracker name." fullWidth>
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
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup
          title={<h3>Change tracking</h3>}
          description={
            'Properties defining how frequently the page should be checked for changes and how those changes should be reported'
          }
        >
          <EuiFormRow label="Frequency" helpText={scheduleHelpText}>
            <PageTrackerJobSchedule
              schedule={jobConfig?.schedule}
              onChange={(schedule, retryIntervals) => {
                // If the schedule is invalid, update only the schedule.
                if (schedule === '' && jobConfig) {
                  setJobConfig({ ...jobConfig, schedule });
                  return;
                }

                if (schedule === null) {
                  setJobConfig(null);
                } else if (schedule !== jobConfig?.schedule) {
                  setJobConfig({
                    retryStrategy: retryIntervals.length > 0 ? getDefaultRetryStrategy(retryIntervals) : undefined,
                    schedule,
                  });
                }

                setRetryIntervals(retryIntervals);
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
          description={'Custom JavaScript scripts that will be injected into the page before content is extracted'}
        >
          <EuiFormRow
            label="Content extractor"
            helpText={
              <span>
                The script accepts a single &quot;context&quot; object argument, and should return the content intended
                for tracking. The returned value can be anything as long as it can be serialized to a{' '}
                <EuiLink
                  target="_blank"
                  href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/JSON/stringify#description"
                >
                  <b>JSON string</b>
                </EuiLink>
                {', '}
                including any{' '}
                <EuiLink target="_blank" href="https://eui.elastic.co/#/editors-syntax/markdown-format#kitchen-sink">
                  <b>valid markdown-style content</b>
                </EuiLink>
                . Refer to the{' '}
                <EuiLink target="_blank" href="/docs/guides/web_scraping/page#annex-content-extractor-script-examples">
                  <b>documentation</b>
                </EuiLink>{' '}
                for a list of script examples and properties available in the &quot;context&quot; object argument.
              </span>
            }
          >
            <ScriptEditor onChange={onExtractContentScriptChange} defaultValue={extractorScript} />
          </EuiFormRow>
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
