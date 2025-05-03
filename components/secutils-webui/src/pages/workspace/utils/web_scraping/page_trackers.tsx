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
} from '@elastic/eui';
import axios from 'axios';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import type { PageTracker } from './page_tracker';
import { PageTrackerEditFlyout } from './page_tracker_edit_flyout';
import { PageTrackerRevision } from './page_tracker_revision';
import type { TrackerDataRevision } from './tracker_data_revision';
import { TrackerName } from './tracker_name';
import { TrackerRevisions } from './tracker_revisions';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../hooks';

export default function PageTrackers() {
  const { uiState, setTitleActions } = useWorkspaceContext();

  const [trackers, setTrackers] = useState<AsyncData<PageTracker[]>>({ status: 'pending' });

  const [trackerToRemove, setTrackerToRemove] = useState<PageTracker | null>(null);
  const [trackerToEdit, setTrackerToEdit] = useState<PageTracker | null | undefined>(null);

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusInCircle'}
        fill
        title="Track content for a web page"
        onClick={() => setTrackerToEdit(undefined)}
      >
        Track content
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
    axios.get<PageTracker[]>(getApiUrl('/api/utils/web_scraping/page'), getApiRequestConfig()).then(
      (response) => {
        setTrackers({ status: 'succeeded', data: response.data });
        setTitleActions(response.data.length === 0 ? null : createButton);
      },
      (err: Error) => {
        setTrackers({ status: 'failed', error: getErrorMessage(err) });
      },
    );
  }, [createButton, setTitleActions]);

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadTrackers();
  }, [uiState, loadTrackers]);

  const editFlyout =
    trackerToEdit !== null ? (
      <PageTrackerEditFlyout
        onClose={(success) => {
          if (success) {
            loadTrackers();
          }
          setTrackerToEdit(null);
        }}
        tracker={trackerToEdit as PageTracker}
      />
    ) : null;

  const [itemIdToExpandedRowMap, setItemIdToExpandedRowMap] = useState<Record<string, ReactNode>>({});

  const removeConfirmModal = trackerToRemove ? (
    <EuiConfirmModal
      title={`Remove "${trackerToRemove.name}"?`}
      onCancel={() => setTrackerToRemove(null)}
      onConfirm={() => {
        setTrackerToRemove(null);

        axios
          .delete(
            getApiUrl(`/api/utils/web_scraping/page/${encodeURIComponent(trackerToRemove?.id)}`),
            getApiRequestConfig(),
          )
          .then(
            () => loadTrackers(),
            (err: Error) => {
              console.error(`Failed to remove the page tracker: ${getErrorMessage(err)}`);
            },
          );
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
  const [sorting, setSorting] = useState<{ sort: PropertySort }>({ sort: { field: 'name', direction: 'asc' } });
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
        <TrackerRevisions kind={'page'} tracker={tracker}>
          {(revision, mode) => <PageTrackerRevision revision={revision as TrackerDataRevision<string>} mode={mode} />}
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
      <EuiInMemoryTable
        pagination={pagination}
        allowNeutralSort={false}
        sorting={sorting}
        onTableChange={onTableChange}
        items={trackers.data}
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
            textOnly: true,
            render: (_, tracker: PageTracker) => <TrackerName tracker={tracker} />,
          },
          {
            name: 'Last updated',
            field: 'updatedAt',
            width: '160px',
            mobileOptions: { width: 'unset' },
            sortable: (tracker) => tracker.updatedAt,
            render: (_, tracker: PageTracker) => <TimestampTableCell timestamp={tracker.updatedAt} />,
          },
          {
            name: 'Actions',
            field: 'headers',
            width: '75px',
            actions: [
              {
                name: 'Edit tracker',
                description: 'Edit tracker',
                icon: 'pencil',
                type: 'icon',
                onClick: setTrackerToEdit,
              },
              {
                name: 'Remove tracker',
                description: 'Remove tracker',
                icon: 'minusInCircle',
                type: 'icon',
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
