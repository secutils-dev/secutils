import { EuiDataGrid, type EuiDataGridCellValueElementProps, EuiText, type Pagination } from '@elastic/eui';
import { useCallback, useState } from 'react';

import type { TrackerRevisionsViewMode } from '../tracker_revisions';

export interface PageTrackerRevisionTableViewProps {
  data: PageTrackerRevisionTableViewData;
  mode: TrackerRevisionsViewMode;
}

export interface PageTrackerRevisionTableViewData {
  '@secutils.data.view': 'table';
  columns: Array<{ id: string; label: string; sortable?: boolean }>;
  rows: Array<Record<string, unknown>>;
}

interface ComplexValue {
  value: unknown;
  color?: string;
}

const IS_NUMBER_REGEX = /^[0-9,]*$/g;

export function isPageTrackerRevisionTableViewData(data: unknown): data is PageTrackerRevisionTableViewData {
  return (
    typeof data === 'object' &&
    data !== null &&
    '@secutils.data.view' in data &&
    (data as Record<string, unknown>)['@secutils.data.view'] === 'table' &&
    Array.isArray((data as PageTrackerRevisionTableViewData).columns) &&
    Array.isArray((data as PageTrackerRevisionTableViewData).rows)
  );
}

export function isComplexValue(value: unknown): value is ComplexValue {
  return !!value && typeof value === 'object' && 'value' in value;
}

export function PageTrackerRevisionTableView({ data }: PageTrackerRevisionTableViewProps) {
  const [visibleColumns, setVisibleColumns] = useState(() => data.columns.map((column) => column.id));
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

  return (
    <EuiDataGrid
      aria-label="Page tracker data table"
      columns={data.columns.map((column) => ({
        id: column.id,
        display: column.label,
        displayAsText: column.label,
        isSortable: !!column.sortable,
      }))}
      columnVisibility={{ visibleColumns, setVisibleColumns }}
      rowCount={data.rows.length}
      renderCellValue={({ rowIndex, columnId }: EuiDataGridCellValueElementProps) => {
        const value = data.rows[rowIndex]?.[columnId];
        if (value === undefined || value === null) {
          return '-';
        }

        if (isComplexValue(value)) {
          return (
            <EuiText size={'xs'} color={value.color}>
              {typeof value.value === 'string' ? value.value : JSON.stringify(value.value, null, 2)}
            </EuiText>
          );
        }

        return typeof value === 'string' ? value : JSON.stringify(value, null, 2);
      }}
      pagination={{
        ...pagination,
        onChangeItemsPerPage: onChangeItemsPerPage,
        onChangePage: onChangePage,
      }}
      inMemory={{ level: 'sorting' }}
      sorting={{ columns: sortingColumns, onSort: setSortingColumns }}
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
  );
}
