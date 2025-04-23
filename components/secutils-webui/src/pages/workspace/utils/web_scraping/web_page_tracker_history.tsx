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
import axios from 'axios';
import { unix } from 'moment';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useState } from 'react';

import type { WebPageContentRevision, WebPageDataRevision } from './web_page_data_revision';
import type { WebPageTracker } from './web_page_tracker';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface WebPageTrackerHistoryProps {
  tracker: WebPageTracker;
  kind: 'content' | 'resources';
  children: (revision: WebPageDataRevision, mode: WebPageTrackerHistoryMode) => ReactNode;
}

export type WebPageTrackerHistoryMode = 'default' | 'diff' | 'source';

export function WebPageTrackerHistory({ kind, tracker, children }: WebPageTrackerHistoryProps) {
  const { uiState, addToast } = useWorkspaceContext();

  const [revisions, setRevisions] = useState<AsyncData<WebPageContentRevision[], WebPageContentRevision[] | null>>({
    status: 'pending',
    state: null,
  });
  const [revisionIndex, setRevisionIndex] = useState<number | null>(null);

  const modes = [
    { id: 'default' as const, label: 'Default', isDisabled: revisions.status !== 'succeeded' },
    { id: 'diff' as const, label: 'Diff', isDisabled: revisionIndex === 0 || revisions.status !== 'succeeded' },
    ...(kind === 'content'
      ? [{ id: 'source' as const, label: 'Source', isDisabled: revisions.status !== 'succeeded' }]
      : []),
  ];
  const [mode, setMode] = useState<WebPageTrackerHistoryMode>('default');

  const fetchHistory = useCallback(
    (
      { refresh, forceMode }: { refresh: boolean; forceMode?: WebPageTrackerHistoryMode } = {
        refresh: false,
      },
    ) => {
      setRevisions((currentRevisions) =>
        currentRevisions.status === 'succeeded'
          ? { status: 'pending', state: currentRevisions.data }
          : { status: 'pending', state: currentRevisions.state },
      );

      const historyUrl = getApiUrl(`/api/utils/web_scraping/${kind}/${encodeURIComponent(tracker.id)}/history`);
      axios
        .post<
          WebPageContentRevision[]
        >(historyUrl, { refresh, calculateDiff: (forceMode ?? mode) === 'diff' }, getApiRequestConfig())
        .then(
          (response) => {
            setRevisions({ status: 'succeeded', data: response.data });

            // Reset a revision index only if it's not set or doesn't exist in the new data.
            if (refresh || revisionIndex === null || revisionIndex >= response.data.length) {
              setRevisionIndex(response.data.length > 0 ? response.data.length - 1 : null);
            }

            if (response.data.length < 2) {
              setMode('default');
            } else if (forceMode && forceMode !== mode) {
              setMode(forceMode);
            }
          },
          (err: Error) => {
            setRevisions((currentRevisions) => ({
              status: 'failed',
              error: getErrorMessage(err),
              state: currentRevisions.state,
            }));
            setRevisionIndex(null);
          },
        );
    },
    [revisionIndex, mode, kind, tracker.id],
  );

  useEffect(() => {
    if (!uiState.synced || !uiState.user) {
      return;
    }

    fetchHistory();
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
      title={`Clear web page tracker history?`}
      onCancel={() => setClearHistoryStatus({ isModalVisible: false, isInProgress: false })}
      isLoading={clearHistoryStatus.isInProgress}
      onConfirm={() => {
        setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: true }));

        axios
          .post(
            getApiUrl(`/api/utils/web_scraping/${kind}/${encodeURIComponent(tracker.id)}/clear`),
            undefined,
            getApiRequestConfig(),
          )
          .then(
            () => {
              setRevisions({ status: 'succeeded', data: [] });
              setRevisionIndex(null);

              addToast({
                id: `success-clear-tracker-history-${tracker.name}`,
                iconType: 'check',
                color: 'success',
                title: `Successfully cleared web page tracker history for ${tracker.url}`,
              });

              setClearHistoryStatus({ isModalVisible: false, isInProgress: false });
            },
            () => {
              addToast({
                id: `failed-clear-tracker-history-${tracker.name}`,
                iconType: 'warning',
                color: 'danger',
                title: `Unable to clear web page tracker history for ${tracker.url}, please try again later`,
              });
              setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: false }));
            },
          );
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Clear"
      buttonColor="danger"
    >
      The web page tracker history for{' '}
      <b>
        {tracker.url} ({tracker.name})
      </b>{' '}
      will be cleared. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  let history;
  if (revisions.status === 'pending') {
    history = <PageLoadingState title={`Loading…`} />;
  } else if (revisions.status === 'failed') {
    history = (
      <PageErrorState
        title="Cannot load web page tracker history"
        content={
          <p>
            Cannot load web page tracker history for {tracker.url}
            <br />
            <br />
            <strong>{revisions.error}</strong>.
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
        title="Fetch content for a web page"
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
                  Go ahead and fetch {kind} for <b>{tracker.url}</b>
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
            isDisabled={
              revisions.status !== 'succeeded' ||
              (kind === 'resources' && revisions.status === 'succeeded' && revisions.data.length < 2)
            }
            onChange={(id) => {
              const newMode = id as WebPageTrackerHistoryMode;
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
            onClick={() => fetchHistory({ refresh: true })}
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
