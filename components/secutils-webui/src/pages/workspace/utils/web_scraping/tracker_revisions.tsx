import {
  EuiButton,
  EuiButtonGroup,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiPanel,
  EuiSelect,
} from '@elastic/eui';
import { css } from '@emotion/react';
import { unix } from 'moment';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useState } from 'react';

import type { PageTracker } from './page_tracker';
import type { TrackerDataRevision } from './tracker_data_revision';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface TrackerRevisionsProps {
  tracker: PageTracker;
  kind: 'page';
  children: (revision: TrackerDataRevision, mode: TrackerRevisionsViewMode) => ReactNode;
}

export type TrackerRevisionsViewMode = 'default' | 'diff' | 'source';

export function TrackerRevisions({ kind, tracker, children }: TrackerRevisionsProps) {
  const { uiState, addToast } = useWorkspaceContext();

  const [revisions, setRevisions] = useState<AsyncData<Array<TrackerDataRevision>, Array<TrackerDataRevision> | null>>({
    status: 'pending',
    state: null,
  });
  const [revisionIndex, setRevisionIndex] = useState<number | null>(null);

  const modes = [
    { id: 'default' as const, label: 'Default', isDisabled: revisions.status !== 'succeeded' },
    {
      id: 'diff' as const,
      label: 'Diff',
      isDisabled: revisions.status !== 'succeeded' || revisionIndex === revisions.data.length - 1,
    },
    { id: 'source' as const, label: 'Source', isDisabled: revisions.status !== 'succeeded' },
  ];
  const [mode, setMode] = useState<TrackerRevisionsViewMode>('default');

  const fetchHistory = useCallback(
    ({ refresh, forceMode }: { refresh: boolean; forceMode: TrackerRevisionsViewMode }) => {
      setRevisions((currentRevisions) =>
        currentRevisions.status === 'succeeded'
          ? { status: 'pending', state: currentRevisions.data }
          : { status: 'pending', state: currentRevisions.state },
      );

      const historyUrl = getApiUrl(`/api/utils/web_scraping/${kind}/${encodeURIComponent(tracker.id)}/history`);
      fetch(historyUrl, {
        ...getApiRequestConfig('POST'),
        body: JSON.stringify({ refresh, calculateDiff: forceMode === 'diff' }),
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

    fetchHistory({ forceMode: 'default', refresh: false });
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

  const clearConfirmModal = clearHistoryStatus.isModalVisible ? (
    <EuiConfirmModal
      title={`Clear page tracker history?`}
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
              title: `Successfully cleared page tracker history`,
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
      The history for the page tracker <b>{tracker.name}</b> will be cleared. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  let history;
  if (revisions.status === 'pending') {
    history = <PageLoadingState title={`Loading…`} />;
  } else if (revisions.status === 'failed') {
    history = (
      <PageErrorState
        title="Cannot load page tracker history"
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
              onClick={() => fetchHistory({ forceMode: mode, refresh: true })}
            >
              Try again
            </EuiButton>
          </p>
        }
      />
    );
  } else if (revisionIndex !== null) {
    history = children(revisions.data[revisionIndex], mode);
  } else {
    const updateButton = (
      <EuiButton
        iconType={'refresh'}
        fill
        title="Fetch data for a web page"
        onClick={() => fetchHistory({ forceMode: mode, refresh: true })}
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
  const shouldDisplayControlPanel =
    (revisions.status === 'succeeded' && revisions.data.length > 0) || (revisions.state?.length ?? 0 > 0);
  const controlPanel = shouldDisplayControlPanel ? (
    <EuiFlexItem>
      <EuiFlexGroup alignItems={'center'}>
        <EuiFlexItem
          css={css`
            min-width: 200px;
          `}
        >
          <EuiSelect
            options={revisionsToSelect.map((rev) => ({
              value: rev.id,
              text: unix(rev.createdAt).format('ll HH:mm:ss'),
            }))}
            disabled={revisions.status === 'pending'}
            value={
              revisionsToSelect.length > 0 && revisionIndex !== null ? revisionsToSelect[revisionIndex].id : undefined
            }
            onChange={(e) => onRevisionChange(e.target.value)}
          />
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButtonGroup
            legend="View mode"
            options={modes}
            idSelected={mode}
            isDisabled={revisions.status !== 'succeeded'}
            onChange={(id) => {
              const newMode = id as TrackerRevisionsViewMode;
              setMode(newMode);

              const shouldFetch = (mode === 'diff' && id !== 'diff') || (mode !== 'diff' && id === 'diff');
              if (shouldFetch) {
                fetchHistory({ forceMode: newMode, refresh: false });
              }
            }}
          />
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButton
            iconType="refresh"
            isDisabled={revisions.status === 'pending'}
            onClick={() => fetchHistory({ forceMode: mode, refresh: true })}
          >
            Update
          </EuiButton>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButton
            iconType="cross"
            color={'danger'}
            isDisabled={revisions.status === 'pending'}
            onClick={() => setClearHistoryStatus({ isModalVisible: true, isInProgress: false })}
          >
            Clear
          </EuiButton>
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
