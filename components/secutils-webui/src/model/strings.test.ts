import { describe, expect, it } from 'vitest';

import { formatBytes, getCopyName } from './strings';

describe('getCopyName', () => {
  it('appends "(Copy 1)" to a plain name', () => {
    expect(getCopyName('My Item')).toBe('My Item (Copy 1)');
  });

  it('increments existing copy number', () => {
    expect(getCopyName('My Item (Copy 1)')).toBe('My Item (Copy 2)');
    expect(getCopyName('My Item (Copy 9)')).toBe('My Item (Copy 10)');
    expect(getCopyName('My Item (Copy 99)')).toBe('My Item (Copy 100)');
  });

  it('handles empty string', () => {
    expect(getCopyName('')).toBe(' (Copy 1)');
  });

  it('handles names that contain parentheses but not the copy pattern', () => {
    expect(getCopyName('Item (special)')).toBe('Item (special) (Copy 1)');
  });

  it('handles names with nested copy-like patterns', () => {
    expect(getCopyName('Item (Copy 3) extra')).toBe('Item (Copy 3) extra (Copy 1)');
  });
});

describe('formatBytes', () => {
  it('formats bytes under 1 KB', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1)).toBe('1 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1023)).toBe('1023 B');
  });

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(10240)).toBe('10.0 KB');
    expect(formatBytes(1024 * 1024 - 1)).toBe('1024.0 KB');
  });

  it('formats megabytes', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB');
    expect(formatBytes(1.5 * 1024 * 1024)).toBe('1.5 MB');
    expect(formatBytes(100 * 1024 * 1024)).toBe('100.0 MB');
  });
});
