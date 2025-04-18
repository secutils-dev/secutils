import type { EuiDataGridCellValueElementProps, EuiDataGridColumn, Pagination } from '@elastic/eui';
import { EuiDataGrid, EuiFlexGroup, EuiFlexItem, EuiStat, EuiText } from '@elastic/eui';
import { unix } from 'moment';
import { useCallback, useState } from 'react';

import type { WebPageResourcesRevision } from './web_page_data_revision';
import type { WebPageResource } from './web_page_resource';

export interface WebPageResourcesTrackerRevisionProps {
  revision: WebPageResourcesRevision;
}

export interface ItemDetailsType {
  id: string;
  createdAt: number;
  combinedResources: Array<WebPageResource & { type: 'js' | 'css' }>;
  scriptsCount: number;
  scriptsTotalSize: number;
  stylesCount: number;
  stylesTotalSize: number;
}

const IS_NUMBER_REGEX = /^[0-9,]*$/g;
const COMMA_SEPARATE_NUMBER_REGEX = /\B(?=(\d{3})+(?!\d))/g;
const commaSeparateNumbers = (bytes: number) => {
  return bytes.toString().replace(COMMA_SEPARATE_NUMBER_REGEX, ',');
};

function formatBytes(bytes: number, decimals = 2) {
  if (bytes == 0) {
    return '0 B';
  }

  const k = 1024,
    sizes = ['B', 'KB', 'MB'],
    i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(decimals))} ${sizes[i]}`;
}

function transformRevision(revision: WebPageResourcesRevision) {
  const itemDetails: ItemDetailsType = {
    id: revision.id,
    createdAt: revision.createdAt,
    scriptsCount: revision.data.scripts?.length ?? 0,
    scriptsTotalSize: 0,
    stylesCount: revision.data.styles?.length ?? 0,
    stylesTotalSize: 0,
    combinedResources: [],
  };

  if (revision.data.scripts) {
    for (const resource of revision.data.scripts) {
      itemDetails.combinedResources.push({ ...resource, type: 'js' });
      itemDetails.scriptsTotalSize += resource.content?.size ?? 0;
    }
  }

  if (revision.data.styles) {
    for (const resource of revision.data.styles) {
      itemDetails.combinedResources.push({ ...resource, type: 'css' });
      itemDetails.stylesTotalSize += resource.content?.size ?? 0;
    }
  }

  return itemDetails;
}

const COLUMNS: EuiDataGridColumn[] = [
  { id: 'source', display: 'Source', displayAsText: 'Source', isExpandable: true, isSortable: true },
  { id: 'diff', display: 'Diff', displayAsText: 'Diff', isExpandable: false, isSortable: true, initialWidth: 75 },
  { id: 'type', display: 'Type', displayAsText: 'Type', initialWidth: 80, isExpandable: false, isSortable: true },
  {
    id: 'size',
    display: 'Size',
    displayAsText: 'Size',
    schema: 'commaNumber',
    initialWidth: 100,
    isExpandable: true,
    isSortable: true,
  },
];

export function WebPageResourcesTrackerRevision({ revision }: WebPageResourcesTrackerRevisionProps) {
  const transformedRevision = transformRevision(revision);

  const [visibleColumns, setVisibleColumns] = useState(() => COLUMNS.map(({ id }) => id));
  const [sortingColumns, setSortingColumns] = useState<Array<{ id: string; direction: 'asc' | 'desc' }>>([]);
  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 10,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });

  const onChangeItemsPerPage = useCallback(
    (pageSize: number) => setPagination({ ...pagination, pageSize }),
    [pagination],
  );
  const onChangePage = useCallback((pageIndex: number) => setPagination({ ...pagination, pageIndex }), [pagination]);

  const renderCellValue = useCallback(
    ({ rowIndex, columnId, isDetails }: EuiDataGridCellValueElementProps) => {
      if (!revision || rowIndex >= transformedRevision.combinedResources.length) {
        return null;
      }

      const detailsItem = transformedRevision.combinedResources[rowIndex];
      let diffStatus: { color?: string; label: string } | undefined;
      if (detailsItem.diffStatus === 'changed') {
        diffStatus = { color: '#79aad9', label: 'Changed' };
      } else if (detailsItem.diffStatus === 'added') {
        diffStatus = { color: '#6dccb1', label: 'Added' };
      } else if (detailsItem.diffStatus === 'removed') {
        diffStatus = { color: '#ff7e62', label: 'Removed' };
      } else {
        diffStatus = { label: '-' };
      }

      if (columnId === 'diff' && diffStatus) {
        return (
          <EuiText size={'xs'} color={diffStatus.color}>
            <b>{diffStatus.label}</b>
          </EuiText>
        );
      }
      if (columnId === 'source') {
        return (
          <EuiText size={'xs'} color={diffStatus?.color}>
            {detailsItem.url ?? '(inline)'}
          </EuiText>
        );
      }

      if (columnId === 'type') {
        return detailsItem.type === 'js' ? 'Script' : 'Stylesheet';
      }

      if (columnId === 'size') {
        return detailsItem.content?.size
          ? isDetails
            ? formatBytes(detailsItem.content.size)
            : commaSeparateNumbers(detailsItem.content.size)
          : '-';
      }

      return null;
    },
    [transformedRevision],
  );

  return (
    <EuiFlexGroup direction={'column'} style={{ height: '100%' }}>
      <EuiFlexItem>
        <EuiFlexGroup>
          <EuiFlexItem>
            <EuiStat
              title={<b>{unix(revision.createdAt).format('ll HH:mm:ss')}</b>}
              titleSize={'xs'}
              description={'Last updated'}
            />
          </EuiFlexItem>
          <EuiFlexItem>
            <EuiStat
              title={
                <b>
                  {transformedRevision.scriptsCount} ({formatBytes(transformedRevision.scriptsTotalSize)})
                </b>
              }
              titleSize={'xs'}
              description={'Scripts'}
            />
          </EuiFlexItem>
          <EuiFlexItem>
            <EuiStat
              title={
                <b>
                  {transformedRevision.stylesCount} ({formatBytes(transformedRevision.stylesTotalSize)})
                </b>
              }
              titleSize={'xs'}
              description={'Styles'}
            />
          </EuiFlexItem>
        </EuiFlexGroup>
      </EuiFlexItem>

      <EuiFlexItem>
        <EuiDataGrid
          width="100%"
          aria-label="Resources"
          columns={COLUMNS}
          columnVisibility={{ visibleColumns, setVisibleColumns }}
          rowCount={transformedRevision.combinedResources.length}
          renderCellValue={renderCellValue}
          inMemory={{ level: 'sorting' }}
          sorting={{ columns: sortingColumns, onSort: setSortingColumns }}
          pagination={{
            ...pagination,
            onChangeItemsPerPage: onChangeItemsPerPage,
            onChangePage: onChangePage,
          }}
          gridStyle={{ border: 'all', fontSize: 's', stripes: true }}
          schemaDetectors={[
            {
              type: 'commaNumber',
              detector: (value) => (IS_NUMBER_REGEX.test(value) ? 1 : 0),
              comparator(a, b, direction) {
                const aValue = a === '-' ? 0 : Number.parseInt(a.replace(/,/g, ''), 10);
                const bValue = b === '-' ? 0 : Number.parseInt(b.replace(/,/g, ''), 10);
                if (aValue > bValue) {
                  return direction === 'asc' ? 1 : -1;
                }
                if (aValue < bValue) {
                  return direction === 'asc' ? -1 : 1;
                }
                return 0;
              },
              sortTextAsc: 'Low-High',
              sortTextDesc: 'High-Low',
              icon: 'tokenNumber',
            },
          ]}
        />
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
