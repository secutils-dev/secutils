import {
  EuiButton,
  EuiButtonGroup,
  EuiButtonIcon,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  euiMaxBreakpoint,
  EuiPanel,
  EuiSelect,
  EuiText,
  EuiToolTip,
  useEuiTheme,
} from '@elastic/eui';
import { css } from '@emotion/react';
import { unix } from 'moment';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useState } from 'react';

import type { ApiTracker } from './api_tracker';
import type { PageTracker } from './page_tracker';
import { isChartableData } from './revision_views/page_tracker_revision_chart_utils';
import { PageTrackerRevisionChartView } from './revision_views/page_tracker_revision_chart_view';
import type { TrackerDataRevision } from './tracker_data_revision';
import { TrackerExecutionLogs } from './tracker_execution_logs';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface TrackerRevisionsProps {
  tracker: ApiTracker | PageTracker;
  kind: 'page' | 'api';
  onHealthRefreshNeeded?: () => void;
  children: (
    revision: TrackerDataRevision,
    mode: TrackerRevisionsViewMode,
    previousRevision?: TrackerDataRevision,
  ) => ReactNode;
}

export type TrackerRevisionsViewMode = 'default' | 'diff' | 'source' | 'chart' | 'logs';

export function TrackerRevisions({ kind, tracker, onHealthRefreshNeeded, children }: TrackerRevisionsProps) {
  const { uiState, addToast } = useWorkspaceContext();
  const euiThemeContext = useEuiTheme();

  const [revisions, setRevisions] = useState<AsyncData<Array<TrackerDataRevision>, Array<TrackerDataRevision> | null>>({
    status: 'pending',
    state: null,
  });
  const [revisionIndex, setRevisionIndex] = useState<number | null>(null);

  const hasRevisions = revisions.status === 'succeeded' && revisions.data.length > 0;
  const isDataChartable = hasRevisions && isChartableData(revisions.data);
  const modes = [
    { id: 'default' as const, label: 'Default', isDisabled: revisions.status !== 'succeeded' },
    ...(hasRevisions
      ? [
          {
            id: 'diff' as const,
            label: 'Diff',
            isDisabled: revisionIndex === revisions.data.length - 1,
          },
          { id: 'source' as const, label: 'Source' },
        ]
      : []),
    ...(isDataChartable ? [{ id: 'chart' as const, label: 'Chart' }] : []),
    { id: 'logs' as const, label: 'Logs' },
  ];
  const [mode, setMode] = useState<TrackerRevisionsViewMode>('default');

  const fetchHistory = useCallback(
    ({ refresh }: { refresh: boolean }) => {
      setRevisions((currentRevisions) =>
        currentRevisions.status === 'succeeded'
          ? { status: 'pending', state: currentRevisions.data }
          : { status: 'pending', state: currentRevisions.state },
      );

      const historyUrl = getApiUrl(`/api/web_scraping/${kind}_trackers/${encodeURIComponent(tracker.id)}/_history`);
      fetch(historyUrl, {
        ...getApiRequestConfig('POST'),
        body: JSON.stringify({ refresh }),
      })
        .then(async (res) => {
          if (!res.ok) {
            throw await ResponseError.fromResponse(res);
          }

          const revisions = (await res.json()) as TrackerDataRevision[];
          setRevisions({ status: 'succeeded', data: revisions });

          // Reset a revision index only if it's not set or doesn't exist in the new data.
          setRevisionIndex((prevRevisionIndex) =>
            refresh || prevRevisionIndex === null || prevRevisionIndex >= revisions.length
              ? revisions.length > 0
                ? 0
                : null
              : prevRevisionIndex,
          );

          if (revisions.length < 2) {
            setMode((m) => (m === 'logs' ? m : 'default'));
          }

          if (refresh) {
            onHealthRefreshNeeded?.();
          }
        })
        .catch((err: Error) => {
          setRevisions((currentRevisions) => ({
            status: 'failed',
            error: getErrorMessage(err),
            state: currentRevisions.state,
          }));
          setRevisionIndex(null);

          if (refresh) {
            onHealthRefreshNeeded?.();
          }
        });
    },
    [kind, tracker.id, onHealthRefreshNeeded],
  );

  useEffect(() => {
    if (!uiState.synced || !uiState.user) {
      return;
    }

    fetchHistory({ refresh: false });
  }, [uiState, tracker, fetchHistory]);

  const onRevisionChange = useCallback(
    (revisionId: string) => {
      if (revisions.status === 'succeeded') {
        setRevisionIndex(revisions.data?.findIndex((revision) => revision.id === revisionId) ?? null);
      }
    },
    [revisions],
  );

  const [clearHistoryStatus, setClearHistoryStatus] = useState<{ isModalVisible: boolean; isInProgress: boolean }>({
    isInProgress: false,
    isModalVisible: false,
  });

  const trackerLabel = kind === 'api' ? 'API tracker' : 'page tracker';
  const clearConfirmModal = clearHistoryStatus.isModalVisible ? (
    <EuiConfirmModal
      title={`Clear ${trackerLabel} history?`}
      onCancel={() => setClearHistoryStatus({ isModalVisible: false, isInProgress: false })}
      isLoading={clearHistoryStatus.isInProgress}
      onConfirm={() => {
        setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: true }));

        fetch(
          getApiUrl(`/api/web_scraping/${kind}_trackers/${encodeURIComponent(tracker.id)}/_clear`),
          getApiRequestConfig('POST'),
        )
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            setRevisions({ status: 'succeeded', data: [] });
            setRevisionIndex(null);

            addToast({
              id: `success-clear-tracker-history-${tracker.name}`,
              iconType: 'check',
              color: 'success',
              title: `Successfully cleared ${trackerLabel} history`,
            });

            setClearHistoryStatus({ isModalVisible: false, isInProgress: false });
          })
          .catch(() => {
            addToast({
              id: `failed-clear-tracker-history-${tracker.name}`,
              iconType: 'warning',
              color: 'danger',
              title: `Unable to clear page tracker history, please try again later`,
            });
            setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: false }));
          });
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Clear"
      buttonColor="danger"
    >
      The history for the {trackerLabel} <b>{tracker.name}</b> will be cleared. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const [logsRefreshKey, setLogsRefreshKey] = useState(0);
  const [clearLogsStatus, setClearLogsStatus] = useState<{ isModalVisible: boolean; isInProgress: boolean }>({
    isInProgress: false,
    isModalVisible: false,
  });

  const clearLogsConfirmModal = clearLogsStatus.isModalVisible ? (
    <EuiConfirmModal
      title={`Clear ${trackerLabel} execution logs?`}
      onCancel={() => setClearLogsStatus({ isModalVisible: false, isInProgress: false })}
      isLoading={clearLogsStatus.isInProgress}
      onConfirm={() => {
        setClearLogsStatus((s) => ({ ...s, isInProgress: true }));

        fetch(
          getApiUrl(`/api/web_scraping/${kind}_trackers/${encodeURIComponent(tracker.id)}/_clear_logs`),
          getApiRequestConfig('POST'),
        )
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            setLogsRefreshKey((k) => k + 1);
            onHealthRefreshNeeded?.();
            addToast({
              id: `success-clear-tracker-logs-${tracker.name}`,
              iconType: 'check',
              color: 'success',
              title: `Successfully cleared ${trackerLabel} execution logs`,
            });
            setClearLogsStatus({ isModalVisible: false, isInProgress: false });
          })
          .catch(() => {
            addToast({
              id: `failed-clear-tracker-logs-${tracker.name}`,
              iconType: 'warning',
              color: 'danger',
              title: `Unable to clear execution logs, please try again later`,
            });
            setClearLogsStatus((s) => ({ ...s, isInProgress: false }));
          });
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Clear"
      buttonColor="danger"
    >
      The execution logs for the {trackerLabel} <b>{tracker.name}</b> will be cleared. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const isLogsMode = mode === 'logs';
  const hideRevisionPicker = isLogsMode || mode === 'chart';

  let history;
  if (isLogsMode) {
    history = <TrackerExecutionLogs key={logsRefreshKey} kind={kind} tracker={tracker} />;
  } else if (revisions.status === 'pending') {
    history = <PageLoadingState title={`Loading…`} />;
  } else if (revisions.status === 'failed') {
    history = (
      <PageErrorState
        title={`Cannot load ${trackerLabel} history`}
        content={
          <p>
            <strong
              css={css`
                white-space: pre-line;
              `}
            >
              {revisions.error}
            </strong>
            .
            <br />
            <br />
            <EuiButton
              iconType={'refresh'}
              fill
              title="Fetch data for a web page"
              onClick={() => fetchHistory({ refresh: true })}
            >
              Try again
            </EuiButton>
          </p>
        }
      />
    );
  } else if (revisionIndex !== null) {
    history =
      mode === 'chart' ? (
        <PageTrackerRevisionChartView revisions={revisions.data} />
      ) : (
        children(revisions.data[revisionIndex], mode, revisions.data[revisionIndex + 1])
      );
  } else {
    const updateButton = (
      <EuiButton
        iconType={'refresh'}
        fill
        title="Fetch data for a web page"
        onClick={() => fetchHistory({ refresh: true })}
      >
        Update
      </EuiButton>
    );
    history = (
      <EuiFlexGroup
        direction={'column'}
        gutterSize={'s'}
        justifyContent="center"
        alignItems="center"
        style={{ height: '100%' }}
      >
        <EuiFlexItem>
          <EuiEmptyPrompt
            icon={<EuiIcon type={'securitySignalDetected'} size={'xl'} />}
            title={<h2>Nothing has been tracked yet</h2>}
            body={
              <div>
                <p>
                  Go ahead and fetch new data for <b>{tracker.name}</b>
                </p>
                {updateButton}
              </div>
            }
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
          />
        </EuiFlexItem>
      </EuiFlexGroup>
    );
  }

  const revisionsToSelect = revisions.status === 'succeeded' ? revisions.data : (revisions.state ?? []);
  const isLoading = revisions.status === 'pending' && !isLogsMode;
  const totalRevisions = revisionsToSelect.length;
  const canGoNewer = revisionIndex !== null && revisionIndex > 0;
  const canGoOlder = revisionIndex !== null && revisionIndex < totalRevisions - 1;
  const controlPanel = (
    <EuiFlexItem>
      <EuiFlexGroup
        alignItems={'center'}
        responsive={false}
        wrap
        gutterSize="s"
        css={css`
          row-gap: 4px;
          ${euiMaxBreakpoint(euiThemeContext, 'm')} {
            justify-content: center;
          }
        `}
      >
        {!hideRevisionPicker && totalRevisions > 0 && (
          <EuiFlexItem grow={false}>
            <EuiFlexGroup alignItems="center" responsive={false} gutterSize="xs">
              <EuiFlexItem grow={false}>
                <EuiToolTip content="Newer revision">
                  <EuiButtonIcon
                    iconType="arrowLeft"
                    aria-label="Newer revision"
                    isDisabled={isLoading || !canGoNewer}
                    onClick={() => setRevisionIndex((i) => (i !== null && i > 0 ? i - 1 : i))}
                    size="s"
                  />
                </EuiToolTip>
              </EuiFlexItem>
              <EuiFlexItem grow={false}>
                <EuiSelect
                  compressed
                  options={revisionsToSelect.map((rev) => ({
                    value: rev.id,
                    text: unix(rev.createdAt).format('ll HH:mm:ss'),
                  }))}
                  disabled={isLoading}
                  value={totalRevisions > 0 && revisionIndex !== null ? revisionsToSelect[revisionIndex].id : undefined}
                  onChange={(e) => onRevisionChange(e.target.value)}
                />
              </EuiFlexItem>
              <EuiFlexItem grow={false}>
                <EuiToolTip content="Older revision">
                  <EuiButtonIcon
                    iconType="arrowRight"
                    aria-label="Older revision"
                    isDisabled={isLoading || !canGoOlder}
                    onClick={() => setRevisionIndex((i) => (i !== null && i < totalRevisions - 1 ? i + 1 : i))}
                    size="s"
                  />
                </EuiToolTip>
              </EuiFlexItem>
            </EuiFlexGroup>
          </EuiFlexItem>
        )}
        {!hideRevisionPicker && totalRevisions > 1 && revisionIndex !== null && (
          <EuiFlexItem
            grow={false}
            css={css`
              ${euiMaxBreakpoint(euiThemeContext, 'm')} {
                flex-basis: 100%;
                text-align: center;
                order: -1;
              }
            `}
          >
            <EuiText size="xs" color="subdued">
              {revisionIndex + 1} / {totalRevisions}
            </EuiText>
          </EuiFlexItem>
        )}
        <EuiFlexItem
          grow
          css={css`
            ${euiMaxBreakpoint(euiThemeContext, 'm')} {
              flex-grow: 0 !important;
            }
          `}
        >
          <EuiFlexGroup
            alignItems="center"
            responsive={false}
            justifyContent="flexEnd"
            gutterSize="s"
            css={css`
              ${euiMaxBreakpoint(euiThemeContext, 'm')} {
                justify-content: center;
              }
            `}
          >
            <EuiFlexItem grow={false}>
              <EuiButtonGroup
                legend="View mode"
                options={modes}
                idSelected={mode}
                onChange={(id) => setMode(id as TrackerRevisionsViewMode)}
              />
            </EuiFlexItem>
            {!isLogsMode && totalRevisions > 0 && (
              <EuiFlexItem grow={false}>
                <EuiToolTip content="Update">
                  <EuiButtonIcon
                    iconType="refresh"
                    aria-label="Update"
                    isDisabled={isLoading}
                    onClick={() => fetchHistory({ refresh: true })}
                  />
                </EuiToolTip>
              </EuiFlexItem>
            )}
            {(isLogsMode || totalRevisions > 0) && (
              <EuiFlexItem grow={false}>
                <EuiToolTip content={isLogsMode ? 'Clear logs' : 'Clear history'}>
                  <EuiButtonIcon
                    iconType="cross"
                    color="danger"
                    aria-label={isLogsMode ? 'Clear logs' : 'Clear history'}
                    isDisabled={isLoading}
                    onClick={() =>
                      isLogsMode
                        ? setClearLogsStatus({ isModalVisible: true, isInProgress: false })
                        : setClearHistoryStatus({ isModalVisible: true, isInProgress: false })
                    }
                  />
                </EuiToolTip>
              </EuiFlexItem>
            )}
          </EuiFlexGroup>
        </EuiFlexItem>
      </EuiFlexGroup>
    </EuiFlexItem>
  );
  return (
    <div style={{ width: 0, minWidth: '100%', overflow: 'hidden' }}>
      <EuiFlexGroup direction={'column'} style={{ height: '100%' }} gutterSize={'s'}>
        {controlPanel}
        <EuiFlexItem>
          <EuiPanel hasShadow={false} hasBorder={true}>
            {history}
          </EuiPanel>
          {clearConfirmModal}
          {clearLogsConfirmModal}
        </EuiFlexItem>
      </EuiFlexGroup>
    </div>
  );
}
