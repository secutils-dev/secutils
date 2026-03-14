import { describe, expect, it } from 'vitest';

import type { SerializedSearchItem } from './search_item';
import { deserializeSearchItem } from './search_item';

describe('deserializeSearchItem', () => {
  it('deserializes all fields', () => {
    const serialized: SerializedSearchItem = {
      l: 'My Label',
      c: 'Category',
      s: 'SubCategory',
      m: { key: 'value', another: 'data' },
      t: 1700000000,
    };

    expect(deserializeSearchItem(serialized)).toEqual({
      label: 'My Label',
      category: 'Category',
      subCategory: 'SubCategory',
      meta: { key: 'value', another: 'data' },
      timestamp: 1700000000,
    });
  });

  it('handles missing optional fields', () => {
    const serialized: SerializedSearchItem = {
      l: 'Label',
      c: 'Cat',
      t: 0,
    };

    const result = deserializeSearchItem(serialized);
    expect(result.label).toBe('Label');
    expect(result.category).toBe('Cat');
    expect(result.subCategory).toBeUndefined();
    expect(result.meta).toBeUndefined();
    expect(result.timestamp).toBe(0);
  });

  it('preserves empty meta object', () => {
    const serialized: SerializedSearchItem = {
      l: 'Label',
      c: 'Cat',
      m: {},
      t: 42,
    };

    expect(deserializeSearchItem(serialized).meta).toEqual({});
  });
});
