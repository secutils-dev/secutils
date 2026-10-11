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
import { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import { UTIL_HANDLES } from '..';
import type { ResponderSummary } from './responder';
import { getResponder } from './responder';
import type { ResponderEditFlyoutProps } from './responder_edit_flyout';
import { ResponderName } from './responder_name';
import { ResponderRequestsTable } from './responder_requests_table';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { useUserTags } from '../../../../hooks';
import type { Page, PaginationRequest } from '../../../../model';
import {
  apiFetch,
  buildPaginationQuery,
  formatBytes,
  getCopyName,
  getErrorMessage,
  ResponseError,
} from '../../../../model';
import {
  FilteredEmptyState,
  ItemsTableFilter,
  TagsFilter,
  useServerPaginatedItems,
} from '../../components/items_table_filter';
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useEntityLoader, useWorkspaceContext } from '../../hooks';
import { getWorkspaceEntityAbsoluteLink, getWorkspaceEntityLink } from '../workspace_links';

const ResponderEditFlyout = lazy(() => import('./responder_edit_flyout'));

export default function Responders() {
  const theme = useEuiTheme();
  const { uiState, setTitleActions, addToast } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);
  const [responderToRemove, setResponderToRemove] = useState<ResponderSummary | null>(null);
  const [responderToEdit, setResponderToEdit] = useState<ResponderEditFlyoutProps['responder'] | null>(null);
  const { allTags } = useUserTags();
  const { load: loadResponder, cancel: cancelResponderLoad, loading: responderLoading } = useEntityLoader(getResponder);

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusCircle'}
        title="Create new responder"
        fill
        onClick={() => {
          cancelResponderLoad();
          setResponderToEdit(undefined);
        }}
      >
        Create responder
      </EuiButton>
    ),
    [cancelResponderLoad],
  );

  const onLoadResponderError = useCallback(
    (name: string, err: unknown) =>
      addToast({
        id: 'load-responder-error',
        iconType: 'warning',
        color: 'danger',
        title: `Failed to load responder "${name}": ${getErrorMessage(err)}`,
      }),
    [addToast],
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

  const fetcher = useCallback(async (request: PaginationRequest): Promise<Page<ResponderSummary>> => {
    const res = await apiFetch(`/api/webhooks/responders${buildPaginationQuery(request)}`);
    if (!res.ok) {
      throw await ResponseError.fromResponse(res);
    }
    return (await res.json()) as Page<ResponderSummary>;
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
  } = useServerPaginatedItems<ResponderSummary>({
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

  const getResponderUrl = useCallback(
    (responder: ResponderSummary) => {
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

  const toggleResponderRequests = (responder: ResponderSummary) => {
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
            icon={<EuiIcon type={'vectorTriangle'} size={'xl'} aria-hidden={true} />}
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
          loading={loading || responderLoading}
          pagination={pagination}
          noItemsMessage={<FilteredEmptyState hasPageFilters={hasPageFilters} onClearFilters={clearPageFilters} />}
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
                    Name{' '}
                    <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" aria-hidden={true} />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              sortable: true,
              render: (_, responder: ResponderSummary) => (
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
                    URL <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" aria-hidden={true} />
                  </span>
                </EuiToolTip>
              ),
              field: 'path',
              render: (_, responder: ResponderSummary) => {
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
                  <EuiIcon
                    type="minus"
                    color={responder.enabled ? undefined : theme.euiTheme.colors.textDisabled}
                    aria-hidden={true}
                  />
                );
              },
            },
            {
              name: 'Method',
              field: 'method',
              width: '100px',
              render: (_, { enabled, method }: ResponderSummary) => (
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
              render: (_, responder: ResponderSummary) => (
                <TimestampTableCell
                  timestamp={responder.stats.lastRequestedAt}
                  highlightRecent
                  disabled={!responder.enabled}
                />
              ),
            },
            {
              name: (
                <EuiToolTip content="Serialized responder settings size, including body, headers, script, and other settings.">
                  <span>
                    Size{' '}
                    <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" aria-hidden={true} />
                  </span>
                </EuiToolTip>
              ),
              field: 'settingsSize',
              width: '90px',
              align: 'right',
              mobileOptions: { width: 'unset' },
              sortable: true,
              render: (_, { enabled, settingsSize }: ResponderSummary) => (
                <EuiText size={'s'} color={enabled ? undefined : theme.euiTheme.colors.textDisabled}>
                  {formatBytes(settingsSize)}
                </EuiText>
              ),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: true,
              render: (_, responder: ResponderSummary) => (
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
                  onClick: ({ id }: ResponderSummary) => void navigator.clipboard.writeText(id),
                },
                {
                  name: 'Copy link',
                  description: 'Copy link to responder in grid',
                  icon: 'link',
                  type: 'icon',
                  onClick: ({ id }: ResponderSummary) =>
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
                  onClick: ({ id, name }: ResponderSummary) =>
                    loadResponder(id, setResponderToEdit, (err) => onLoadResponderError(name, err)),
                },
                {
                  name: 'Duplicate',
                  description: 'Duplicate responder',
                  icon: 'copy',
                  type: 'icon',
                  onClick: ({ id, name }: ResponderSummary) =>
                    loadResponder(
                      id,
                      // eslint-disable-next-line @typescript-eslint/no-unused-vars
                      ({ id: _id, createdAt, updatedAt, ...source }) =>
                        setResponderToEdit({
                          ...source,
                          name: getCopyName(
                            source.name,
                            responders.map((r) => r.name),
                          ),
                        }),
                      (err) => onLoadResponderError(name, err),
                    ),
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
              render: (item: ResponderSummary) => {
                return (
                  <EuiButtonIcon
                    onClick={() => toggleResponderRequests(item)}
                    aria-label={itemIdToExpandedRowMap[item.id] ? 'Hide requests' : 'Show requests'}
                    iconType={itemIdToExpandedRowMap[item.id] ? 'chevronSingleDown' : 'chevronSingleRight'}
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
