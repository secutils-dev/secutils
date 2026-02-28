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
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface TrackerRevisionsProps {
  tracker: ApiTracker | PageTracker;
  kind: 'page' | 'api';
  children: (
    revision: TrackerDataRevision,
    mode: TrackerRevisionsViewMode,
    previousRevision?: TrackerDataRevision,
  ) => ReactNode;
}

export type TrackerRevisionsViewMode = 'default' | 'diff' | 'source' | 'chart';

export function TrackerRevisions({ kind, tracker, children }: TrackerRevisionsProps) {
  const { uiState, addToast } = useWorkspaceContext();
  const euiThemeContext = useEuiTheme();

  const [revisions, setRevisions] = useState<AsyncData<Array<TrackerDataRevision>, Array<TrackerDataRevision> | null>>({
    status: 'pending',
    state: null,
  });
  const [revisionIndex, setRevisionIndex] = useState<number | null>(null);

  const isDataChartable = revisions.status === 'succeeded' && isChartableData(revisions.data);
  const modes = [
    { id: 'default' as const, label: 'Default', isDisabled: revisions.status !== 'succeeded' },
    {
      id: 'diff' as const,
      label: 'Diff',
      isDisabled: revisions.status !== 'succeeded' || revisionIndex === revisions.data.length - 1,
    },
    { id: 'source' as const, label: 'Source', isDisabled: revisions.status !== 'succeeded' },
    ...(isDataChartable
      ? [{ id: 'chart' as const, label: 'Chart', isDisabled: revisions.status !== 'succeeded' }]
      : []),
  ];
  const [mode, setMode] = useState<TrackerRevisionsViewMode>('default');

  const fetchHistory = useCallback(
    ({ refresh }: { refresh: boolean }) => {
      setRevisions((currentRevisions) =>
        currentRevisions.status === 'succeeded'
          ? { status: 'pending', state: currentRevisions.data }
          : { status: 'pending', state: currentRevisions.state },
      );

      const historyUrl = getApiUrl(`/api/utils/web_scraping/${kind}/${encodeURIComponent(tracker.id)}/history`);
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
            setMode('default');
          }
        })
        .catch((err: Error) => {
          setRevisions((currentRevisions) => ({
            status: 'failed',
            error: getErrorMessage(err),
            state: currentRevisions.state,
          }));
          setRevisionIndex(null);
        });
    },
    [kind, tracker.id],
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
          getApiUrl(`/api/utils/web_scraping/${kind}/${encodeURIComponent(tracker.id)}/clear`),
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

  let history;
  if (revisions.status === 'pending') {
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
  const isLoading = revisions.status === 'pending';
  const totalRevisions = revisionsToSelect.length;
  const canGoNewer = revisionIndex !== null && revisionIndex > 0;
  const canGoOlder = revisionIndex !== null && revisionIndex < totalRevisions - 1;
  const shouldDisplayControlPanel =
    (revisions.status === 'succeeded' && revisions.data.length > 0) || (revisions.state?.length ?? 0 > 0);
  const controlPanel = shouldDisplayControlPanel ? (
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
        {totalRevisions > 1 && revisionIndex !== null && (
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
                isDisabled={revisions.status !== 'succeeded'}
                onChange={(id) => setMode(id as TrackerRevisionsViewMode)}
              />
            </EuiFlexItem>
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
            <EuiFlexItem grow={false}>
              <EuiToolTip content="Clear history">
                <EuiButtonIcon
                  iconType="cross"
                  color="danger"
                  aria-label="Clear history"
                  isDisabled={isLoading}
                  onClick={() => setClearHistoryStatus({ isModalVisible: true, isInProgress: false })}
                />
              </EuiToolTip>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiFlexItem>
      </EuiFlexGroup>
    </EuiFlexItem>
  ) : null;
  return (
    <EuiFlexGroup direction={'column'} style={{ height: '100%' }} gutterSize={'s'}>
      {controlPanel}
      <EuiFlexItem>
        <EuiPanel hasShadow={false} hasBorder={true}>
          {history}
        </EuiPanel>
        {clearConfirmModal}
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
