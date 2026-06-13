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
  EuiLink,
  EuiScreenReaderOnly,
  EuiSpacer,
  EuiText,
  EuiToolTip,
  useEuiTheme,
} from '@elastic/eui';
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import { UTIL_HANDLES } from '..';
import type { Responder } from './responder';
import { ResponderName } from './responder_name';
import { ResponderRequestsTable } from './responder_requests_table';
import type { ResponderStats } from './responder_stats';
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

const ResponderEditFlyout = lazy(() => import('./responder_edit_flyout'));

export default function Responders() {
  const theme = useEuiTheme();
  const { uiState, setTitleActions } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);
  const [stats, setStats] = useState<Map<string, ResponderStats>>(new Map());
  const [responderToRemove, setResponderToRemove] = useState<Responder | null>(null);
  const [responderToEdit, setResponderToEdit] = useState<Partial<Responder> | null | undefined>(null);
  const { allTags } = useUserTags();

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

  const fetcher = useCallback(async (request: PaginationRequest): Promise<Page<Responder>> => {
    const [respondersRes, respondersStatsRes] = await Promise.all([
      apiFetch(`/api/webhooks/responders${buildPaginationQuery(request)}`),
      apiFetch('/api/webhooks/responders/_stats'),
    ]);

    if (!respondersRes.ok) {
      throw await ResponseError.fromResponse(respondersRes);
    }
    if (!respondersStatsRes.ok) {
      throw await ResponseError.fromResponse(respondersStatsRes);
    }

    const page = (await respondersRes.json()) as Page<Responder>;
    const respondersStat = (await respondersStatsRes.json()) as ResponderStats[];
    setStats(new Map(respondersStat.map((stat) => [stat.responderId, stat])));
    return page;
  }, []);

  const {
    items: responders,
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
  } = useServerPaginatedItems<Responder>({
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

  useEffect(() => {
    setTitleActions(isEmpty ? null : createButton);
  }, [isEmpty, createButton, setTitleActions]);

  const getResponderUrl = useCallback(
    (responder: Responder) => {
      if (!uiState.user) {
        return '-';
      }
      const subdomain = responder.location.subdomainPrefix
        ? `${responder.location.subdomainPrefix}-${uiState.user.handle}`
        : uiState.user.handle;
      return `${location.protocol}//${subdomain}.webhooks.${location.host}${responder.location.path}`;
    },
    [uiState],
  );

  const [itemIdToExpandedRowMap, setItemIdToExpandedRowMap] = useState<Record<string, ReactNode>>({});
  const editFlyout =
    responderToEdit !== null ? (
      <Suspense fallback={null}>
        <ResponderEditFlyout
          onClose={(success) => {
            if (success) {
              refresh();
            }
            setResponderToEdit(null);
          }}
          responder={responderToEdit}
        />
      </Suspense>
    ) : null;

  const removeConfirmModal = responderToRemove ? (
    <EuiConfirmModal
      title={`Remove "${responderToRemove.name}"?`}
      onCancel={() => setResponderToRemove(null)}
      onConfirm={() => {
        setResponderToRemove(null);

        apiFetch(`/api/webhooks/responders/${encodeURIComponent(responderToRemove?.id)}`, { method: 'DELETE' })
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }
            refresh();
          })
          .catch((err: Error) => console.error(`Failed to remove responder: ${err.message}`));
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The responder endpoint will be deactivated, and the request history will be cleared. Are you sure you want to
      proceed?
    </EuiConfirmModal>
  ) : null;

  const toggleResponderRequests = (responder: Responder) => {
    const itemIdToExpandedRowMapValues = { ...itemIdToExpandedRowMap };
    if (itemIdToExpandedRowMapValues[responder.id]) {
      delete itemIdToExpandedRowMapValues[responder.id];
    } else {
      itemIdToExpandedRowMapValues[responder.id] = <ResponderRequestsTable responder={responder} />;
    }
    setItemIdToExpandedRowMap(itemIdToExpandedRowMapValues);
  };

  if (!initialized && loading) {
    return <PageLoadingState />;
  }

  if (error && responders.length === 0) {
    return <PageErrorState title="Cannot load responders" content={<p>{error}</p>} />;
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
          onRefresh={refresh}
          placeholder="Search by name, path, or ID..."
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
          items={responders}
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
              render: (_, responder: Responder) => (
                <ResponderName
                  responder={responder}
                  href={getWorkspaceEntityLink(UTIL_HANDLES.webhooksResponders, responder.id)}
                />
              ),
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
              render: (_, responder: Responder) => {
                const url = getResponderUrl(responder);
                return responder.enabled && url ? (
                  <EuiLink href={url} target="_blank" style={{ minWidth: '200px', display: 'inline-block' }}>
                    {url}
                  </EuiLink>
                ) : url ? (
                  <EuiText size={'s'} color={theme.euiTheme.colors.textDisabled} style={{ minWidth: '200px' }}>
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
            },
            {
              name: 'Last requested',
              field: 'lastRequestedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: true,
              render: (_, responder: Responder) => (
                <TimestampTableCell
                  timestamp={stats.get(responder.id)?.lastRequestedAt}
                  highlightRecent
                  disabled={!responder.enabled}
                />
              ),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: true,
              render: (_, responder: Responder) => (
                <TimestampTableCell timestamp={responder.updatedAt} disabled={!responder.enabled} />
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
                  name: 'Copy link',
                  description: 'Copy link to responder in grid',
                  icon: 'link',
                  type: 'icon',
                  onClick: ({ id }: Responder) =>
                    void navigator.clipboard.writeText(
                      getWorkspaceEntityAbsoluteLink(UTIL_HANDLES.webhooksResponders, id),
                    ),
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
                    setResponderToEdit({
                      ...rest,
                      name: getCopyName(
                        name,
                        responders.map((r) => r.name),
                      ),
                    }),
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
