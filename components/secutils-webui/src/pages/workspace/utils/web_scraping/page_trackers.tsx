import type { Criteria, Pagination, PropertySort } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiButtonIcon,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiInMemoryTable,
  EuiScreenReaderOnly,
  EuiSpacer,
  EuiToolTip,
  useEuiTheme,
} from '@elastic/eui';
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import { UTIL_HANDLES } from '..';
import type { PageTracker } from './page_tracker';
import { PageTrackerRevision } from './page_tracker_revision';
import type { TrackerDataRevision } from './tracker_data_revision';
import { TrackerHealthDots } from './tracker_health_dots';
import { TrackerName } from './tracker_name';
import { TrackerRevisions } from './tracker_revisions';
import { useTrackerHealth } from './use_tracker_health';
import { PageErrorState, PageLoadingState } from '../../../../components';
import {
  type AsyncData,
  getApiRequestConfig,
  getApiUrl,
  getCopyName,
  getErrorMessage,
  ResponseError,
} from '../../../../model';
import { ItemsTableFilter, useItemsTableFilter } from '../../components/items_table_filter';
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../hooks';
import { getWorkspaceEntityAbsoluteLink, getWorkspaceEntityLink } from '../workspace_links';

const PageTrackerEditFlyout = lazy(() => import('./page_tracker_edit_flyout'));

export default function PageTrackers() {
  const { uiState, setTitleActions } = useWorkspaceContext();
  const theme = useEuiTheme();

  const [trackers, setTrackers] = useState<AsyncData<PageTracker[]>>({ status: 'pending' });

  const [trackerToRemove, setTrackerToRemove] = useState<PageTracker | null>(null);
  const [trackerToEdit, setTrackerToEdit] = useState<Partial<PageTracker> | null | undefined>(null);

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusInCircle'}
        fill
        title="Track content for a web page"
        onClick={() => setTrackerToEdit(undefined)}
      >
        Track page
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to create and use page trackers"
      target={'_blank'}
      href={'/docs/guides/web_scraping/page'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const loadTrackers = useCallback(() => {
    fetch(getApiUrl('/api/utils/web_scraping/page'), getApiRequestConfig())
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        const trackers = (await res.json()) as PageTracker[];
        setTrackers({ status: 'succeeded', data: trackers });
        setTitleActions(trackers.length === 0 ? null : createButton);
      })
      .catch((err: Error) => setTrackers({ status: 'failed', error: getErrorMessage(err) }));
  }, [createButton, setTitleActions]);

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadTrackers();
  }, [uiState, loadTrackers]);

  const editFlyout =
    trackerToEdit !== null ? (
      <Suspense fallback={null}>
        <PageTrackerEditFlyout
          onClose={(success) => {
            if (success) {
              loadTrackers();
            }
            setTrackerToEdit(null);
          }}
          tracker={trackerToEdit}
        />
      </Suspense>
    ) : null;

  // Filter configuration: search by name and ID
  const getSearchFields = useCallback((tracker: PageTracker) => [tracker.name, tracker.id], []);

  // Use the filter hook with URL sync
  const { filteredItems, query, setQuery } = useItemsTableFilter({
    items: trackers.status === 'succeeded' ? trackers.data : [],
    getSearchFields,
  });

  const { data: healthData, refetch: refetchHealth } = useTrackerHealth(
    'page',
    trackers.status === 'succeeded' ? trackers.data.map((t) => t.id) : undefined,
  );

  const [itemIdToExpandedRowMap, setItemIdToExpandedRowMap] = useState<Record<string, ReactNode>>({});

  const removeConfirmModal = trackerToRemove ? (
    <EuiConfirmModal
      title={`Remove "${trackerToRemove.name}"?`}
      onCancel={() => setTrackerToRemove(null)}
      onConfirm={() => {
        setTrackerToRemove(null);

        fetch(
          getApiUrl(`/api/utils/web_scraping/page/${encodeURIComponent(trackerToRemove?.id)}`),
          getApiRequestConfig('DELETE'),
        )
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            loadTrackers();
          })
          .catch((err: Error) => {
            console.error(`Failed to remove the page tracker: ${getErrorMessage(err)}`);
          });
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The page tracker <b>{trackerToRemove.name}</b> will be deactivated, and the tracked history will be cleared. Are
      you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 15,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });
  const [sorting, setSorting] = useState<{ sort: PropertySort }>({ sort: { field: 'updatedAt', direction: 'desc' } });
  const onTableChange = useCallback(
    ({ page, sort }: Criteria<PageTracker>) => {
      setPagination({
        ...pagination,
        pageIndex: page?.index ?? 0,
        pageSize: page?.size ?? 15,
      });

      if (sort?.field) {
        setSorting({ sort });
      }
    },
    [pagination],
  );

  const toggleItemDetails = (tracker: PageTracker) => {
    const itemIdToExpandedRowMapValues = { ...itemIdToExpandedRowMap };
    if (itemIdToExpandedRowMapValues[tracker.id]) {
      delete itemIdToExpandedRowMapValues[tracker.id];
    } else {
      itemIdToExpandedRowMapValues[tracker.id] = (
        <TrackerRevisions kind={'page'} tracker={tracker} onHealthRefreshNeeded={refetchHealth}>
          {(revision, mode, previousRevision) => (
            <PageTrackerRevision
              revision={revision as TrackerDataRevision<string>}
              mode={mode}
              previousRevision={previousRevision as TrackerDataRevision<string> | undefined}
            />
          )}
        </TrackerRevisions>
      );
    }
    setItemIdToExpandedRowMap(itemIdToExpandedRowMapValues);
  };

  if (trackers.status === 'pending') {
    return <PageLoadingState />;
  }

  if (trackers.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load page trackers"
        content={
          <p>
            Cannot load page trackers
            <br />
            <br />
            <strong>{trackers.error}</strong>.
          </p>
        }
      />
    );
  }

  let content;
  if (trackers.data.length === 0) {
    content = (
      <EuiFlexGroup
        direction={'column'}
        gutterSize={'s'}
        justifyContent="center"
        alignItems="center"
        style={{ height: '100%' }}
      >
        <EuiFlexItem>
          <EuiEmptyPrompt
            icon={<EuiIcon type={'cut'} size={'xl'} />}
            title={<h2>You don&apos;t have any page trackers yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and track any web page.</p>
                {createButton}
                <EuiSpacer size={'s'} />
                {docsButton}
              </div>
            }
          />
        </EuiFlexItem>
      </EuiFlexGroup>
    );
  } else {
    content = (
      <>
        <ItemsTableFilter
          query={query}
          onQueryChange={setQuery}
          onRefresh={() => {
            loadTrackers();
            refetchHealth();
          }}
          placeholder="Search by name or ID..."
        />
        <EuiSpacer size="m" />
        <EuiInMemoryTable
          pagination={pagination}
          allowNeutralSort={false}
          sorting={sorting}
          onTableChange={onTableChange}
          items={filteredItems}
          itemId={(item) => item.id}
          itemIdToExpandedRowMap={itemIdToExpandedRowMap}
          tableLayout={'fixed'}
          columns={[
            {
              name: (
                <EuiToolTip content="Name of the page tracker">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              sortable: true,
              render: (_, tracker: PageTracker) => (
                <TrackerName
                  tracker={tracker}
                  href={getWorkspaceEntityLink(UTIL_HANDLES.webScrapingPage, tracker.id)}
                />
              ),
            },
            {
              name: (
                <EuiToolTip content="Recent execution status (oldest to newest)">
                  <span>
                    Health <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'id',
              width: '150px',
              render: (_: string, tracker: PageTracker) => (
                <TrackerHealthDots logs={healthData.status === 'succeeded' ? healthData.data[tracker.id] : undefined} />
              ),
            },
            {
              name: 'Next run',
              field: 'retrack.scheduledAt',
              width: '130px',
              sortable: (tracker) => tracker.retrack.scheduledAt ?? null,
              render: (_, tracker: PageTracker) =>
                tracker.retrack.scheduledAt != null ? (
                  <TimestampTableCell
                    timestamp={tracker.retrack.scheduledAt}
                    color={tracker.retrack.enabled === false ? theme.euiTheme.colors.textDisabled : undefined}
                  />
                ) : (
                  '—'
                ),
            },
            {
              name: 'Last ran',
              field: 'retrack.lastRanAt',
              width: '130px',
              sortable: (tracker) => tracker.retrack.lastRanAt ?? null,
              render: (_, tracker: PageTracker) =>
                tracker.retrack.lastRanAt != null ? (
                  <TimestampTableCell
                    timestamp={tracker.retrack.lastRanAt}
                    color={tracker.retrack.enabled === false ? theme.euiTheme.colors.textDisabled : undefined}
                  />
                ) : (
                  '—'
                ),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: (tracker) => tracker.updatedAt,
              render: (_, tracker: PageTracker) => (
                <TimestampTableCell
                  timestamp={tracker.updatedAt}
                  color={tracker.retrack.enabled === false ? theme.euiTheme.colors.textDisabled : undefined}
                />
              ),
            },
            {
              name: <></>,
              render: () => <EuiSpacer size={'xxl'} />,
              mobileOptions: { only: true },
            },
            {
              name: 'Actions',
              field: 'headers',
              width: '105px',
              actions: [
                {
                  name: 'Copy ID',
                  description: 'Copy ID to clipboard',
                  icon: 'tokenKey',
                  type: 'icon',
                  onClick: ({ id }: PageTracker) => void navigator.clipboard.writeText(id),
                },
                {
                  name: 'Copy link',
                  description: 'Copy link to tracker in grid',
                  icon: 'link',
                  type: 'icon',
                  onClick: ({ id }: PageTracker) =>
                    void navigator.clipboard.writeText(
                      getWorkspaceEntityAbsoluteLink(UTIL_HANDLES.webScrapingPage, id),
                    ),
                },
                {
                  name: 'Edit',
                  description: 'Edit tracker',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setTrackerToEdit,
                },
                {
                  name: 'Duplicate',
                  description: 'Duplicate tracker',
                  icon: 'copy',
                  type: 'icon',
                  // eslint-disable-next-line @typescript-eslint/no-unused-vars
                  onClick: ({ id, createdAt, updatedAt, name, ...rest }: PageTracker) =>
                    setTrackerToEdit({ ...rest, name: getCopyName(name) }),
                },
                {
                  name: 'Remove',
                  description: 'Remove tracker',
                  icon: 'trash',
                  color: 'danger',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setTrackerToRemove,
                },
              ],
            },
            {
              align: 'right',
              width: '40px',
              isExpander: true,
              name: (
                <EuiScreenReaderOnly>
                  <span>Show history</span>
                </EuiScreenReaderOnly>
              ),
              render: (tracker: PageTracker) => {
                return (
                  <EuiButtonIcon
                    onClick={() => toggleItemDetails(tracker)}
                    aria-label={itemIdToExpandedRowMap[tracker.id] ? 'Hide history' : 'Show history'}
                    iconType={itemIdToExpandedRowMap[tracker.id] ? 'arrowDown' : 'arrowRight'}
                  />
                );
              },
            },
          ]}
        />
      </>
    );
  }

  return (
    <>
      {content}
      {editFlyout}
      {removeConfirmModal}
    </>
  );
}
