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
  EuiText,
  EuiToolTip,
  useEuiTheme,
} from '@elastic/eui';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import type { Responder } from './responder';
import { ResponderEditFlyout } from './responder_edit_flyout';
import { ResponderName } from './responder_name';
import { ResponderRequestsTable } from './responder_requests_table';
import type { ResponderStats } from './responder_stats';
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

export default function Responders() {
  const theme = useEuiTheme();
  const { uiState, setTitleActions } = useWorkspaceContext();

  const [responders, setResponders] = useState<
    AsyncData<{ responders: Responder[]; stats: Map<string, ResponderStats> }>
  >({ status: 'pending' });
  const [responderToRemove, setResponderToRemove] = useState<Responder | null>(null);
  const [responderToEdit, setResponderToEdit] = useState<Partial<Responder> | null | undefined>(null);

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusInCircle'}
        title="Create new responder"
        fill
        onClick={() => setResponderToEdit(undefined)}
      >
        Create responder
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to create and use responders"
      target={'_blank'}
      href={'/docs/guides/webhooks'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const loadResponders = useCallback(() => {
    Promise.all([
      fetch(getApiUrl('/api/utils/webhooks/responders'), getApiRequestConfig()),
      fetch(getApiUrl('/api/utils/webhooks/responders/stats'), getApiRequestConfig()),
    ])
      .then(async ([respondersRes, respondersStatsRes]) => {
        if (!respondersRes.ok) {
          throw await ResponseError.fromResponse(respondersRes);
        }

        if (!respondersStatsRes.ok) {
          throw await ResponseError.fromResponse(respondersStatsRes);
        }

        const responders = (await respondersRes.json()) as Responder[];
        const respondersStat = (await respondersStatsRes.json()) as ResponderStats[];
        setResponders({
          status: 'succeeded',
          data: {
            responders,
            stats: new Map(respondersStat.map((stats) => [stats.responderId, stats])),
          },
        });
        setTitleActions(responders.length === 0 ? null : createButton);
      })
      .catch((err) => setResponders({ status: 'failed', error: getErrorMessage(err) }));
  }, [createButton, setTitleActions]);

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadResponders();
  }, [uiState, loadResponders]);

  const getResponderUrl = useCallback(
    (responder: Responder) => {
      if (!uiState.user) {
        return '-';
      }
      const subdomain = responder.location.subdomainPrefix
        ? `${responder.location.subdomainPrefix}-${uiState.user.handle}`
        : uiState.user.handle;
      return uiState.webhookUrlType === 'path'
        ? `${location.origin}/api/webhooks/${uiState.user.handle}${responder.location.path}`
        : `${location.protocol}//${subdomain}.webhooks.${location.host}${responder.location.path}`;
    },
    [uiState],
  );

  // Filter configuration: search by name, path, and ID
  const getSearchFields = useCallback(
    (responder: Responder) => [responder.name, responder.id, responder.location.path],
    [],
  );

  // Use the filter hook with URL sync
  const { filteredItems, query, setQuery } = useItemsTableFilter({
    items: responders.status === 'succeeded' ? responders.data.responders : [],
    getSearchFields,
  });

  const [itemIdToExpandedRowMap, setItemIdToExpandedRowMap] = useState<Record<string, ReactNode>>({});
  const editFlyout =
    responderToEdit !== null ? (
      <ResponderEditFlyout
        onClose={(success) => {
          if (success) {
            loadResponders();
          }
          setResponderToEdit(null);
        }}
        responder={responderToEdit}
      />
    ) : null;

  const removeConfirmModal = responderToRemove ? (
    <EuiConfirmModal
      title={`Remove "${responderToRemove.name}"?`}
      onCancel={() => setResponderToRemove(null)}
      onConfirm={() => {
        setResponderToRemove(null);

        fetch(
          getApiUrl(`/api/utils/webhooks/responders/${encodeURIComponent(responderToRemove?.id)}`),
          getApiRequestConfig('DELETE'),
        )
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }
            loadResponders();
          })
          .catch((err) => console.error(`Failed to remove responder: ${getErrorMessage(err)}`));
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The responder endpoint will be deactivated, and the request history will be cleared. Are you sure you want to
      proceed?
    </EuiConfirmModal>
  ) : null;

  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 15,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });
  const [sorting, setSorting] = useState<{ sort: PropertySort }>({ sort: { field: 'path', direction: 'asc' } });
  const onTableChange = useCallback(
    ({ page, sort }: Criteria<Responder>) => {
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

  const toggleResponderRequests = (responder: Responder) => {
    const itemIdToExpandedRowMapValues = { ...itemIdToExpandedRowMap };
    if (itemIdToExpandedRowMapValues[responder.id]) {
      delete itemIdToExpandedRowMapValues[responder.id];
    } else {
      itemIdToExpandedRowMapValues[responder.id] = <ResponderRequestsTable responder={responder} />;
    }
    setItemIdToExpandedRowMap(itemIdToExpandedRowMapValues);
  };

  if (responders.status === 'pending') {
    return <PageLoadingState />;
  }

  if (responders.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load responders"
        content={
          <p>
            Cannot load responders
            <br />
            <br />
            <strong>{responders.error}</strong>.
          </p>
        }
      />
    );
  }

  let content;
  if (responders.data.responders.length === 0) {
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
            icon={<EuiIcon type={'node'} size={'xl'} />}
            title={<h2>You don&apos;t have any responders yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and create your first HTTP responder.</p>
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
          onRefresh={loadResponders}
          placeholder="Search by name, path, or ID..."
        />
        <EuiSpacer size="m" />
        <EuiInMemoryTable
          pagination={pagination}
          allowNeutralSort={false}
          sorting={sorting}
          onTableChange={onTableChange}
          items={filteredItems}
          itemId={(responder) => responder.id}
          itemIdToExpandedRowMap={itemIdToExpandedRowMap}
          tableLayout={'auto'}
          columns={[
            {
              name: (
                <EuiToolTip content="Name of the responder">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              sortable: true,
              textOnly: true,
              render: (_, responder: Responder) => <ResponderName responder={responder} />,
            },
            {
              name: (
                <EuiToolTip content="A unique URL of the responder endpoint">
                  <span>
                    URL <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'path',
              sortable: true,
              render: (_, responder: Responder) => {
                const url = getResponderUrl(responder);
                return responder.enabled && url ? (
                  <EuiLink href={url} target="_blank">
                    {url}
                  </EuiLink>
                ) : url ? (
                  <EuiText size={'s'} color={theme.euiTheme.colors.textDisabled}>
                    {url}
                  </EuiText>
                ) : (
                  <EuiIcon type="minus" color={responder.enabled ? undefined : theme.euiTheme.colors.textDisabled} />
                );
              },
            },
            {
              name: 'Method',
              field: 'method',
              width: '100px',
              render: (_, { enabled, method }: Responder) => (
                <EuiText size={'s'} color={enabled ? undefined : theme.euiTheme.colors.textDisabled}>
                  <b>{method}</b>
                </EuiText>
              ),
              sortable: true,
            },
            {
              name: 'Last requested',
              field: 'createdAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: (responder) => responders.data.stats.get(responder.id)?.lastRequestedAt ?? 0,
              render: (_, responder: Responder) => {
                const stats = responders.data.stats.get(responder.id);
                return stats?.lastRequestedAt ? (
                  <TimestampTableCell
                    timestamp={stats.lastRequestedAt}
                    highlightRecent
                    color={responder.enabled ? undefined : theme.euiTheme.colors.textDisabled}
                  />
                ) : (
                  <EuiText size={'s'} color={responder.enabled ? undefined : theme.euiTheme.colors.textDisabled}>
                    <b>-</b>
                  </EuiText>
                );
              },
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: (responder) => responder.updatedAt,
              render: (_, responder: Responder) => (
                <TimestampTableCell
                  timestamp={responder.updatedAt}
                  color={responder.enabled ? undefined : theme.euiTheme.colors.textDisabled}
                />
              ),
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
                  onClick: ({ id }: Responder) => void navigator.clipboard.writeText(id),
                },
                {
                  name: 'Edit',
                  description: 'Edit responder',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setResponderToEdit,
                },
                {
                  name: 'Duplicate',
                  description: 'Duplicate responder',
                  icon: 'copy',
                  type: 'icon',
                  // eslint-disable-next-line @typescript-eslint/no-unused-vars
                  onClick: ({ id, createdAt, updatedAt, name, ...rest }: Responder) =>
                    setResponderToEdit({ ...rest, name: getCopyName(name) }),
                },
                {
                  name: 'Remove',
                  description: 'Remove responder',
                  icon: 'trash',
                  color: 'danger',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setResponderToRemove,
                },
              ],
            },
            {
              align: 'right',
              width: '40px',
              isExpander: true,
              name: (
                <EuiScreenReaderOnly>
                  <span>Show requests</span>
                </EuiScreenReaderOnly>
              ),
              render: (item: Responder) => {
                return (
                  <EuiButtonIcon
                    onClick={() => toggleResponderRequests(item)}
                    aria-label={itemIdToExpandedRowMap[item.id] ? 'Hide requests' : 'Show requests'}
                    iconType={itemIdToExpandedRowMap[item.id] ? 'arrowDown' : 'arrowRight'}
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
