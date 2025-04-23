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
  EuiLink,
  EuiScreenReaderOnly,
  EuiSpacer,
  EuiToolTip,
} from '@elastic/eui';
import axios from 'axios';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import { WebPageContentTrackerEditFlyout } from './web_page_content_tracker_edit_flyout';
import { WebPageContentTrackerRevision } from './web_page_content_tracker_revision';
import type { WebPageContentRevision } from './web_page_data_revision';
import type { WebPageContentTracker, WebPageTracker } from './web_page_tracker';
import { WebPageTrackerHistory } from './web_page_tracker_history';
import { WebPageTrackerName } from './web_page_tracker_name';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../hooks';

export default function WebPageContentTrackers() {
  const { uiState, setTitleActions } = useWorkspaceContext();

  const [trackers, setTrackers] = useState<AsyncData<WebPageTracker[]>>({ status: 'pending' });

  const [trackerToRemove, setTrackerToRemove] = useState<WebPageTracker | null>(null);
  const [trackerToEdit, setTrackerToEdit] = useState<WebPageTracker | null | undefined>(null);

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
      title="Learn how to create and use web page trackers"
      target={'_blank'}
      href={'/docs/guides/web_scraping/content'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const loadTrackers = useCallback(() => {
    axios.get<WebPageTracker[]>(getApiUrl('/api/utils/web_scraping/content'), getApiRequestConfig()).then(
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
      <WebPageContentTrackerEditFlyout
        onClose={(success) => {
          if (success) {
            loadTrackers();
          }
          setTrackerToEdit(null);
        }}
        tracker={trackerToEdit as WebPageContentTracker}
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
            getApiUrl(`/api/utils/web_scraping/content/${encodeURIComponent(trackerToRemove?.id)}`),
            getApiRequestConfig(),
          )
          .then(
            () => loadTrackers(),
            (err: Error) => {
              console.error(`Failed to remove web page tracker: ${getErrorMessage(err)}`);
            },
          );
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The web page tracker for{' '}
      <b>
        {trackerToRemove.url} ({trackerToRemove.name})
      </b>{' '}
      will be deactivated, and the tracked history will be cleared. Are you sure you want to proceed?
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
    ({ page, sort }: Criteria<WebPageTracker>) => {
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

  const toggleItemDetails = (tracker: WebPageTracker) => {
    const itemIdToExpandedRowMapValues = { ...itemIdToExpandedRowMap };
    if (itemIdToExpandedRowMapValues[tracker.id]) {
      delete itemIdToExpandedRowMapValues[tracker.id];
    } else {
      itemIdToExpandedRowMapValues[tracker.id] = (
        <WebPageTrackerHistory kind={'content'} tracker={tracker}>
          {(revision, mode) => (
            <WebPageContentTrackerRevision revision={revision as WebPageContentRevision} mode={mode} />
          )}
        </WebPageTrackerHistory>
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
        title="Cannot load web page trackers"
        content={
          <p>
            Cannot load web page trackers
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
            title={<h2>You don&apos;t have any web page trackers yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and track content for your web page.</p>
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
              <EuiToolTip content="Name of the web page tracker">
                <span>
                  Name <EuiIcon size="s" color="subdued" type="questionInCircle" className="eui-alignTop" />
                </span>
              </EuiToolTip>
            ),
            field: 'name',
            sortable: true,
            textOnly: true,
            render: (_, tracker: WebPageTracker) => <WebPageTrackerName tracker={tracker} />,
          },
          {
            name: (
              <EuiToolTip content="URL of the web page to track">
                <span>
                  URL <EuiIcon size="s" color="subdued" type="questionInCircle" className="eui-alignTop" />
                </span>
              </EuiToolTip>
            ),
            field: 'url',
            sortable: true,
            render: (_, tracker: WebPageTracker) => (
              <EuiLink href={tracker.url} target="_blank">
                {tracker.url}
              </EuiLink>
            ),
          },
          {
            name: 'Last updated',
            field: 'updatedAt',
            width: '160px',
            mobileOptions: { width: 'unset' },
            sortable: (tracker) => tracker.updatedAt,
            render: (_, tracker: WebPageTracker) => <TimestampTableCell timestamp={tracker.updatedAt} />,
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
            render: (tracker: WebPageTracker) => {
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
