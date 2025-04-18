import { useCallback, useEffect, useState } from 'react';

import type { EuiDataGridCellValueElementProps, EuiDataGridColumn, Pagination } from '@elastic/eui';
import {
  EuiButton,
  EuiCodeBlock,
  EuiConfirmModal,
  EuiDataGrid,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormRow,
  EuiIcon,
  EuiPanel,
} from '@elastic/eui';
import axios from 'axios';
import { unix } from 'moment';

import type { Responder } from './responder';
import type { ResponderRequest } from './responder_request';
import { PageErrorState, PageLoadingState } from '../../../../components';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
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
  useEffect(() => {
    if (!uiState.synced || !uiState.user) {
      return;
    }

    if (responder.settings.requestsToTrack === 0) {
      setRequests({ status: 'succeeded', data: [] });
      return;
    }

    axios
      .post<
        ResponderRequest[]
      >(getApiUrl(`/api/utils/webhooks/responders/${encodeURIComponent(responder.id)}/history`), getApiRequestConfig())
      .then(
        (response) => {
          setRequests({ status: 'succeeded', data: response.data });
        },
        (err: Error) => {
          setRequests((currentRevisions) => ({
            status: 'failed',
            error: getErrorMessage(err),
            state: currentRevisions.state,
          }));
        },
      );
  }, [uiState, responder]);

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
  const onSort = useCallback(
    (sortingColumns: Array<{ id: string; direction: 'asc' | 'desc' }>) => {
      setSortingColumns(sortingColumns);
    },
    [sortingColumns],
  );

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

  if (requests.status === 'pending') {
    return <PageLoadingState title={`Loading requests for "${responder.name}" responder…`} />;
  }

  if (requests.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load requests"
        content={
          <p>
            Cannot load recorded requests for <strong>{responder.name}</strong> responder.
          </p>
        }
      />
    );
  }

  if (requests.data.length === 0) {
    const tracksRequests = responder.settings.requestsToTrack > 0;
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
            icon={<EuiIcon type={tracksRequests ? 'securitySignal' : 'securitySignalDetected'} size={'xl'} />}
            title={
              tracksRequests ? (
                <h2>Still waiting for the first request to arrive</h2>
              ) : (
                <h2>Responder doesn&apos;t track requests</h2>
              )
            }
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

        axios
          .post(
            getApiUrl(`/api/utils/webhooks/responders/${encodeURIComponent(responder.id)}/clear`),
            undefined,
            getApiRequestConfig(),
          )
          .then(
            () => {
              setRequests({ status: 'succeeded', data: [] });

              addToast({
                id: `success-clear-responder-history-${responder.name}`,
                iconType: 'check',
                color: 'success',
                title: `Successfully cleared request history for "${responder.name}" responder`,
              });

              setClearHistoryStatus({ isModalVisible: false, isInProgress: false });
            },
            () => {
              addToast({
                id: `failed-clear-responder-history-${responder.name}`,
                iconType: 'warning',
                color: 'danger',
                title: `Unable to clear request history for "${responder.name}" responder, please try again later`,
              });
              setClearHistoryStatus((currentStatus) => ({ ...currentStatus, isInProgress: false }));
            },
          );
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Clear"
      buttonColor="danger"
    >
      The request history for <b>{responder.name}</b> will be cleared. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const shouldDisplayControlPanel = requests.status === 'succeeded' && requests.data.length > 0;
  const controlPanel = shouldDisplayControlPanel ? (
    <EuiFlexItem>
      <EuiFlexGroup justifyContent={'flexEnd'}>
        <EuiFlexItem grow={false}>
          <EuiFormRow>
            <EuiButton
              iconType="cross"
              color={'danger'}
              onClick={() => setClearHistoryStatus({ isModalVisible: true, isInProgress: false })}
            >
              Clear
            </EuiButton>
          </EuiFormRow>
        </EuiFlexItem>
      </EuiFlexGroup>
    </EuiFlexItem>
  ) : null;

  return (
    <EuiFlexGroup direction={'column'} style={{ height: '100%' }} gutterSize={'s'}>
      {controlPanel}
      <EuiFlexItem>
        <EuiPanel hasShadow={false} hasBorder={true}>
          <EuiDataGrid
            width="100%"
            aria-label="Requests"
            columns={columns}
            columnVisibility={{ visibleColumns, setVisibleColumns }}
            rowCount={requests.data.length}
            renderCellValue={renderCellValue}
            inMemory={{ level: 'sorting' }}
            sorting={{ columns: sortingColumns, onSort }}
            pagination={{
              ...pagination,
              onChangeItemsPerPage: onChangeItemsPerPage,
              onChangePage: onChangePage,
            }}
            gridStyle={{ border: 'all', fontSize: 's', stripes: true }}
            toolbarVisibility
          />
        </EuiPanel>
        {clearConfirmModal}
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
