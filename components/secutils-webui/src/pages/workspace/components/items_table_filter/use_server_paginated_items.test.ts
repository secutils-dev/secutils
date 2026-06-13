import type { Criteria } from '@elastic/eui';
import { describe, expect, it } from 'vitest';

import type { TableState } from './use_server_paginated_items';
import { resolveTableChange } from './use_server_paginated_items';

interface Row {
  name: string;
  updatedAt: number;
}

const current: TableState = {
  pageIndex: 0,
  pageSize: 15,
  sortField: 'updatedAt',
  sortDirection: 'desc',
};

describe('resolveTableChange', () => {
  it('applies a page change while preserving the (echoed) current sort', () => {
    // EUI echoes the current sort back even for a pure page change.
    const criteria: Criteria<Row> = {
      page: { index: 1, size: 15 },
      sort: { field: 'updatedAt', direction: 'desc' },
    };

    expect(resolveTableChange(criteria, current)).toEqual({
      pageIndex: 1,
      pageSize: 15,
      sortField: 'updatedAt',
      sortDirection: 'desc',
    });
  });

  it('applies a page size change without resetting to the first page', () => {
    const criteria: Criteria<Row> = {
      page: { index: 2, size: 25 },
      sort: { field: 'updatedAt', direction: 'desc' },
    };

    expect(resolveTableChange(criteria, current)).toEqual({
      pageIndex: 2,
      pageSize: 25,
      sortField: 'updatedAt',
      sortDirection: 'desc',
    });
  });

  it('resets to the first page when the sort field changes', () => {
    const criteria: Criteria<Row> = {
      page: { index: 3, size: 15 },
      sort: { field: 'name', direction: 'asc' },
    };

    expect(resolveTableChange(criteria, { ...current, pageIndex: 3 })).toEqual({
      pageIndex: 0,
      pageSize: 15,
      sortField: 'name',
      sortDirection: 'asc',
    });
  });

  it('resets to the first page when only the sort direction changes', () => {
    const criteria: Criteria<Row> = {
      sort: { field: 'updatedAt', direction: 'asc' },
    };

    expect(resolveTableChange(criteria, { ...current, pageIndex: 4 })).toEqual({
      pageIndex: 0,
      pageSize: 15,
      sortField: 'updatedAt',
      sortDirection: 'asc',
    });
  });

  it('keeps the current state when criteria carries neither page nor a sort change', () => {
    const criteria: Criteria<Row> = { sort: { field: 'updatedAt', direction: 'desc' } };

    expect(resolveTableChange(criteria, { ...current, pageIndex: 2 })).toEqual({
      pageIndex: 2,
      pageSize: 15,
      sortField: 'updatedAt',
      sortDirection: 'desc',
    });
  });
});
