import type { EuiDataGridCellValueElementProps, EuiDataGridColumn, Pagination } from '@elastic/eui';
import {
  EuiButtonIcon,
  EuiCodeBlock,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiPanel,
  EuiToolTip,
} from '@elastic/eui';
import { unix } from 'moment';
import { useCallback, useEffect, useState } from 'react';

import type { Responder } from './responder';
import type { ResponderRequest } from './responder_request';
import { DataGrid, PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface ResponderRequestsTableProps {
  responder: Responder;
}

const TEXT_DECODER = new TextDecoder();
function binaryToText(binary: number[]) {
  return TEXT_DECODER.decode(new Uint8Array(binary));
}

function guessBodyContentType(request: ResponderRequest) {
  for (const [headerName, headerValue] of request.headers ?? []) {
    if (headerName.toLowerCase() === 'content-type') {
      const headerTextValue = binaryToText(headerValue).toLowerCase();
      if (headerTextValue.includes('json') || headerTextValue.includes('csp-report')) {
        return 'json';
      }

      break;
    }
  }
  return 'http';
}

export function ResponderRequestsTable({ responder }: ResponderRequestsTableProps) {
  const { uiState, addToast } = useWorkspaceContext();

  const [requests, setRequests] = useState<AsyncData<ResponderRequest[]>>(
    responder.settings.requestsToTrack > 0 ? { status: 'pending' } : { status: 'succeeded', data: [] },
  );

  const loadRequests = useCallback(() => {
    setRequests({ status: 'pending' });
    fetch(
      getApiUrl(`/api/utils/webhooks/responders/${encodeURIComponent(responder.id)}/history`),
      getApiRequestConfig('POST'),
    )
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }
        setRequests({ status: 'succeeded', data: (await res.json()) as ResponderRequest[] });
      })
      .catch((err: Error) => {
        setRequests((currentRevisions) => ({
          status: 'failed',
          error: getErrorMessage(err),
          state: currentRevisions.state,
        }));
      });
  }, [responder.id]);

  useEffect(() => {
    if (!uiState.synced || !uiState.user) {
      return;
    }

    if (responder.settings.requestsToTrack === 0) {
      setRequests({ status: 'succeeded', data: [] });
      return;
    }

    loadRequests();
  }, [uiState, responder, loadRequests]);

  const columns: EuiDataGridColumn[] = [
    {
      id: 'timestamp',
      display: 'Timestamp',
      displayAsText: 'Timestamp',
      initialWidth: 170,
      isSortable: true,
      isExpandable: false,
      isResizable: false,
    },
    { id: 'address', display: 'Client address', displayAsText: 'Client address', isExpandable: false },
    { id: 'method', display: 'Method', displayAsText: 'Method', initialWidth: 80, isExpandable: false },
    { id: 'url', display: 'URL', displayAsText: 'URL', isSortable: true },
    { id: 'headers', display: 'Headers', displayAsText: 'Body' },
    { id: 'body', display: 'Body', displayAsText: 'Body' },
  ];
  const [visibleColumns, setVisibleColumns] = useState(() => columns.map(({ id }) => id));
  const [sortingColumns, setSortingColumns] = useState<Array<{ id: string; direction: 'asc' | 'desc' }>>([]);

  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 10,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });
  const onChangeItemsPerPage = useCallback(
    (pageSize: number) => setPagination({ ...pagination, pageSize }),
    [setPagination, pagination],
  );
  const onChangePage = useCallback(
    (pageIndex: number) => setPagination({ ...pagination, pageIndex }),
    [setPagination, pagination],
  );

  const [clearHistoryStatus, setClearHistoryStatus] = useState<{ isModalVisible: boolean; isInProgress: boolean }>({
    isInProgress: false,
    isModalVisible: false,
  });

  const renderCellValue = useCallback(
    ({ rowIndex, columnId, isDetails }: EuiDataGridCellValueElementProps) => {
      if (requests.status !== 'succeeded' || rowIndex >= requests.data.length) {
        return null;
      }

      const request = requests.data[rowIndex];
      if (columnId === 'timestamp') {
        return unix(request.createdAt).format('L HH:mm:ss');
      }

      if (columnId === 'address') {
        return request.clientAddress ?? '-';
      }

      if (columnId === 'method') {
        return request.method;
      }

      if (columnId === 'url') {
        return request.url;
      }

      if (columnId === 'headers') {
        if (!request.headers || request.headers.length === 0) {
          return '-';
        }

        if (isDetails) {
          return (
            <EuiCodeBlock language="http" fontSize="m" isCopyable overflowHeight={'100%'}>
              {request.headers.map(([name, value]) => `${name}: ${binaryToText(value)}`).join('\n')}
            </EuiCodeBlock>
          );
        }

        return `${request.headers.length} headers`;
      }

      if (columnId === 'body') {
        if (!request.body || request.body.length === 0) {
          return '-';
        }

        if (isDetails) {
          return (
            <EuiCodeBlock language={guessBodyContentType(request)} fontSize="m" isCopyable overflowHeight={'100%'}>
              {binaryToText(request.body)}
            </EuiCodeBlock>
          );
        }

        return `${request.body.length} bytes`;
      }

      return null;
    },
    [requests],
  );

  if (responder.settings.requestsToTrack == 0) {
    return (
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
            title={<h2>Responder doesn&apos;t track requests</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
          />
        </EuiFlexItem>
      </EuiFlexGroup>
    );
  }

  const clearConfirmModal = clearHistoryStatus.isModalVisible ? (
    <EuiConfirmModal
      title={`Clear responder history?`}
      onCancel={() => setClearHistoryStatus({ isModalVisible: false, isInProgress: false })}
      isLoading={clearHistoryStatus.isInProgress}
      onConfirm={() => {
        setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: true }));

        fetch(
          getApiUrl(`/api/utils/webhooks/responders/${encodeURIComponent(responder.id)}/clear`),
          getApiRequestConfig('POST'),
        )
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            setRequests({ status: 'succeeded', data: [] });

            addToast({
              id: `success-clear-responder-history-${responder.name}`,
              iconType: 'check',
              color: 'success',
              title: `Successfully cleared request history for "${responder.name}" responder`,
            });

            setClearHistoryStatus({ isModalVisible: false, isInProgress: false });
          })
          .catch(() => {
            addToast({
              id: `failed-clear-responder-history-${responder.name}`,
              iconType: 'warning',
              color: 'danger',
              title: `Unable to clear request history for "${responder.name}" responder, please try again later`,
            });
            setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: false }));
          });
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Clear"
      buttonColor="danger"
    >
      The request history for <b>{responder.name}</b> will be cleared. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const controlPanel = (
    <EuiFlexItem grow={false}>
      <EuiFlexGroup alignItems={'center'} justifyContent={'flexEnd'} responsive={false}>
        <EuiFlexItem grow={false}>
          <EuiToolTip content="Update">
            <EuiButtonIcon
              iconType="refresh"
              aria-label="Update"
              isDisabled={requests.status === 'pending' || !responder.enabled}
              onClick={() => loadRequests()}
            />
          </EuiToolTip>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiToolTip content="Clear request history">
            <EuiButtonIcon
              iconType="cross"
              color="danger"
              aria-label="Clear request history"
              isDisabled={
                requests.status === 'pending' || (requests.status === 'succeeded' && requests.data.length === 0)
              }
              onClick={() => setClearHistoryStatus({ isModalVisible: true, isInProgress: false })}
            />
          </EuiToolTip>
        </EuiFlexItem>
      </EuiFlexGroup>
    </EuiFlexItem>
  );

  let content;
  if (requests.status === 'pending') {
    content = <PageLoadingState />;
  } else if (requests.status === 'failed') {
    content = (
      <PageErrorState
        title="Cannot load requests"
        content={
          <p>
            Cannot load recorded requests for <strong>{responder.name}</strong> responder.
          </p>
        }
      />
    );
  } else if (requests.data.length === 0) {
    content = (
      <EuiEmptyPrompt
        icon={<EuiIcon type={'radar'} size={'xl'} />}
        title={<h2>Still waiting for the first request to arrive</h2>}
        titleSize="s"
      />
    );
  } else {
    content = (
      <DataGrid
        width="100%"
        aria-label="Requests"
        columns={columns}
        columnVisibility={{ visibleColumns, setVisibleColumns }}
        rowCount={requests.data.length}
        renderCellValue={renderCellValue}
        inMemory={{ level: 'sorting' }}
        sorting={{ columns: sortingColumns, onSort: setSortingColumns }}
        pagination={{
          ...pagination,
          onChangeItemsPerPage: onChangeItemsPerPage,
          onChangePage: onChangePage,
        }}
        gridStyle={{ border: 'all', fontSize: 's', stripes: true }}
      />
    );
  }

  return (
    <EuiFlexGroup direction={'column'} style={{ height: '100%' }} gutterSize={'s'}>
      {controlPanel}
      <EuiFlexItem>
        <EuiPanel hasShadow={false} hasBorder={true}>
          {content}
        </EuiPanel>
        {clearConfirmModal}
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
