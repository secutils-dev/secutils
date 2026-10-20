import { describe, expect, it } from 'vitest';

import { fitTagCount } from './entity_name';

describe('fitTagCount', () => {
  it('fits as many tags as possible while reserving room for the "+N" chip', () => {
    // All tags fit: no chip needed, so the last tag can use the space reserved for it.
    expect(fitTagCount([50, 50, 50], 170, 10, 30)).toBe(3);
    // The third tag doesn't fit, and the second one only fits if the chip still does.
    expect(fitTagCount([50, 50, 50], 160, 10, 30)).toBe(2);
    expect(fitTagCount([50, 50, 50], 140, 10, 30)).toBe(1);
    // A single tag that is too wide is still shown, truncated.
    expect(fitTagCount([300], 100, 10, 30)).toBe(1);
    expect(fitTagCount([300, 50], 100, 10, 30)).toBe(1);
    expect(fitTagCount([], 100, 10, 30)).toBe(0);
  });
});
