// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { isSafeNextUrl, isValidURL } from './url';

describe('isSafeNextUrl', () => {
  it('accepts a relative path', () => {
    expect(isSafeNextUrl('/dashboard')).toBe(true);
    expect(isSafeNextUrl('/settings?tab=security')).toBe(true);
  });

  it('accepts the same origin as an absolute URL', () => {
    expect(isSafeNextUrl(`${window.location.origin}/page`)).toBe(true);
  });

  it('rejects a different origin', () => {
    expect(isSafeNextUrl('https://evil.com/phish')).toBe(false);
  });

  it('rejects protocol-relative URLs pointing elsewhere', () => {
    expect(isSafeNextUrl('//evil.com/phish')).toBe(false);
  });

  it('accepts an empty string (resolves to current origin)', () => {
    expect(isSafeNextUrl('')).toBe(true);
  });

  it('accepts a bare hash or query string', () => {
    expect(isSafeNextUrl('#section')).toBe(true);
    expect(isSafeNextUrl('?foo=bar')).toBe(true);
  });
});

describe('isValidURL', () => {
  it('accepts valid HTTP URLs', () => {
    expect(isValidURL('https://example.com')).toBe(true);
    expect(isValidURL('http://localhost:3000/path?q=1')).toBe(true);
  });

  it('accepts other schemes', () => {
    expect(isValidURL('ftp://files.example.com/doc')).toBe(true);
    expect(isValidURL('data:text/plain;base64,SGVsbG8=')).toBe(true);
  });

  it('rejects a bare hostname without scheme', () => {
    expect(isValidURL('example.com')).toBe(false);
  });

  it('rejects relative paths', () => {
    expect(isValidURL('/dashboard')).toBe(false);
    expect(isValidURL('dashboard')).toBe(false);
  });

  it('rejects empty string', () => {
    expect(isValidURL('')).toBe(false);
  });

  it('rejects malformed URLs', () => {
    expect(isValidURL('://')).toBe(false);
    expect(isValidURL('http://')).toBe(false);
  });
});
