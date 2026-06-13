import { describe, expect, it, vi } from 'vitest';

import { buildPaginationQuery, fetchAllItems } from './pagination';
import type { Page } from './pagination';

describe('buildPaginationQuery', () => {
  it('returns an empty string when no params are provided', () => {
    expect(buildPaginationQuery()).toBe('');
    expect(buildPaginationQuery({})).toBe('');
  });

  it('omits empty/undefined values', () => {
    expect(buildPaginationQuery({ q: '', tags: [], globalTags: [] })).toBe('');
  });

  it('serializes provided params and comma-joins tag lists', () => {
    expect(
      buildPaginationQuery({
        page: 0,
        pageSize: 15,
        sort: 'name',
        order: 'asc',
        q: 'alpha beta',
        tags: ['a', 'b'],
        globalTags: ['g'],
      }),
    ).toBe('?page=0&pageSize=15&sort=name&order=asc&q=alpha+beta&tags=a%2Cb&globalTags=g');
  });

  it('serializes page index 0 explicitly', () => {
    expect(buildPaginationQuery({ page: 0 })).toBe('?page=0');
  });
});

describe('fetchAllItems', () => {
  it('walks every page until total is reached', async () => {
    const fetchPage = vi.fn(async ({ page }: { page?: number }): Promise<Page<number>> => {
      const pages: Array<Page<number>> = [
        { items: [1, 2], total: 5 },
        { items: [3, 4], total: 5 },
        { items: [5], total: 5 },
      ];
      return pages[page ?? 0];
    });

    const items = await fetchAllItems(fetchPage);
    expect(items).toEqual([1, 2, 3, 4, 5]);
    expect(fetchPage).toHaveBeenCalledTimes(3);
  });

  it('stops when a page returns no items', async () => {
    const fetchPage = vi.fn(async (): Promise<Page<number>> => ({ items: [], total: 99 }));
    const items = await fetchAllItems(fetchPage);
    expect(items).toEqual([]);
    expect(fetchPage).toHaveBeenCalledTimes(1);
  });
});
