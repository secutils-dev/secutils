import {
  EuiBasicTable,
  EuiButton,
  EuiButtonEmpty,
  EuiButtonIcon,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiScreenReaderOnly,
  EuiSpacer,
  EuiToolTip,
} from '@elastic/eui';
import { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useMemo, useState } from 'react';
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
import { useUserTags } from '../../../../hooks';
import type { Page, PaginationRequest } from '../../../../model';
import { apiFetch, buildPaginationQuery, getCopyName, ResponseError } from '../../../../model';
import {
  FilteredEmptyState,
  ItemsTableFilter,
  TagsFilter,
  useServerPaginatedItems,
} from '../../components/items_table_filter';
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../hooks';
import { getWorkspaceEntityAbsoluteLink, getWorkspaceEntityLink } from '../workspace_links';

const PageTrackerEditFlyout = lazy(() => import('./page_tracker_edit_flyout'));

export default function PageTrackers() {
  const { setTitleActions } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);

  const [trackerToRemove, setTrackerToRemove] = useState<PageTracker | null>(null);
  const [trackerToEdit, setTrackerToEdit] = useState<Partial<PageTracker> | null | undefined>(null);
  const { allTags } = useUserTags();

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

  const fetcher = useCallback(async (request: PaginationRequest): Promise<Page<PageTracker>> => {
    const res = await apiFetch(`/api/web_scraping/page_trackers${buildPaginationQuery(request)}`);
    if (!res.ok) {
      throw await ResponseError.fromResponse(res);
    }
    return (await res.json()) as Page<PageTracker>;
  }, []);

  const {
    items: trackers,
    total,
    loading,
    error,
    pagination,
    sorting,
    onTableChange,
    query,
    setQuery,
    selectedTagIds,
    setSelectedTagIds,
    hasPageFilters,
    hasActiveFilters,
    clearPageFilters,
    refresh,
  } = useServerPaginatedItems<PageTracker>({
    fetcher,
    allTags,
    defaultSortField: 'updatedAt',
    defaultSortDirection: 'desc',
  });

  useEffect(() => {
    if (!loading) {
      setInitialized(true);
    }
  }, [loading]);

  const isEmpty = initialized && total === 0 && !hasActiveFilters;

  // Layout effect (not a passive effect) so the title-bar create button is
  // added/removed in the same commit as the empty-state prompt that renders its
  // own copy of the button. A passive effect lags one paint behind, briefly
  // showing two identical "create" buttons (and tripping Playwright strict mode).
  useLayoutEffect(() => {
    setTitleActions(isEmpty ? null : createButton);
  }, [isEmpty, createButton, setTitleActions]);

  const { data: healthData, refetch: refetchHealth } = useTrackerHealth(
    'page',
    trackers.map((t) => t.id),
  );

  const editFlyout =
    trackerToEdit !== null ? (
      <Suspense fallback={null}>
        <PageTrackerEditFlyout
          onClose={(success) => {
            if (success) {
              refresh();
              refetchHealth();
            }
            setTrackerToEdit(null);
          }}
          tracker={trackerToEdit}
        />
      </Suspense>
    ) : null;

  const [itemIdToExpandedRowMap, setItemIdToExpandedRowMap] = useState<Record<string, ReactNode>>({});

  const removeConfirmModal = trackerToRemove ? (
    <EuiConfirmModal
      title={`Remove "${trackerToRemove.name}"?`}
      onCancel={() => setTrackerToRemove(null)}
      onConfirm={() => {
        setTrackerToRemove(null);

        apiFetch(`/api/web_scraping/page_trackers/${encodeURIComponent(trackerToRemove?.id)}`, { method: 'DELETE' })
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            refresh();
            refetchHealth();
          })
          .catch((err: Error) => {
            console.error(`Failed to remove the page tracker: ${err.message}`);
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

  if (!initialized && loading) {
    return <PageLoadingState />;
  }

  if (error && trackers.length === 0) {
    return <PageErrorState title="Cannot load page trackers" content={<p>{error}</p>} />;
  }

  let content;
  if (isEmpty) {
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
            refresh();
            refetchHealth();
          }}
          placeholder="Search by name or ID..."
        >
          <TagsFilter tags={allTags} selectedTagIds={selectedTagIds} onSelectedTagIdsChange={setSelectedTagIds} />
        </ItemsTableFilter>
        <EuiSpacer size="m" />
        <EuiBasicTable
          loading={loading}
          pagination={pagination}
          noItemsMessage={
            <FilteredEmptyState totalItems={total} hasPageFilters={hasPageFilters} onClearFilters={clearPageFilters} />
          }
          sorting={sorting}
          onChange={onTableChange}
          items={trackers}
          itemId={(item) => item.id}
          itemIdToExpandedRowMap={itemIdToExpandedRowMap}
          tableLayout={'auto'}
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
              render: (_: string, tracker: PageTracker) => (
                <TrackerHealthDots
                  logs={healthData.status === 'succeeded' ? healthData.data[tracker.id] : undefined}
                  disabled={tracker.retrack.enabled === false}
                />
              ),
            },
            {
              name: 'Next run',
              field: 'retrack.scheduledAt',
              render: (_, tracker: PageTracker) => (
                <TimestampTableCell
                  timestamp={tracker.retrack.scheduledAt}
                  disabled={tracker.retrack.enabled === false}
                />
              ),
            },
            {
              name: 'Last ran',
              field: 'retrack.lastRanAt',
              render: (_, tracker: PageTracker) => (
                <TimestampTableCell
                  timestamp={tracker.retrack.lastRanAt}
                  disabled={tracker.retrack.enabled === false}
                />
              ),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              sortable: true,
              render: (_, tracker: PageTracker) => (
                <TimestampTableCell timestamp={tracker.updatedAt} disabled={tracker.retrack.enabled === false} />
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
                    setTrackerToEdit({
                      ...rest,
                      name: getCopyName(
                        name,
                        trackers.map((t) => t.name),
                      ),
                    }),
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
