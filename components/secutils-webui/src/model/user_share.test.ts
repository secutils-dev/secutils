// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';

import { getUserShareId, removeUserShareId, USER_SHARE_ID_HEADER_NAME } from './user_share';

function setSearch(search: string) {
  window.history.replaceState(null, '', `${window.location.pathname}${search}`);
}

afterEach(() => {
  setSearch('');
});

describe('getUserShareId', () => {
  it('returns null when the parameter is absent', () => {
    setSearch('');
    expect(getUserShareId()).toBeNull();
  });

  it('returns the share id when the parameter is present', () => {
    setSearch(`?${USER_SHARE_ID_HEADER_NAME}=abc-123`);
    expect(getUserShareId()).toBe('abc-123');
  });

  it('returns the share id among other parameters', () => {
    setSearch(`?foo=bar&${USER_SHARE_ID_HEADER_NAME}=xyz&baz=1`);
    expect(getUserShareId()).toBe('xyz');
  });
});

describe('removeUserShareId', () => {
  it('does nothing when the parameter is absent', () => {
    setSearch('?other=value');
    removeUserShareId();
    expect(window.location.search).toBe('?other=value');
  });

  it('removes the parameter and keeps other parameters', () => {
    setSearch(`?foo=bar&${USER_SHARE_ID_HEADER_NAME}=abc&baz=1`);
    removeUserShareId();
    expect(window.location.search).toBe('?foo=bar&baz=1');
    expect(getUserShareId()).toBeNull();
  });

  it('removes the parameter and clears the query string when it is the only one', () => {
    setSearch(`?${USER_SHARE_ID_HEADER_NAME}=abc`);
    removeUserShareId();
    expect(window.location.search).toBe('');
  });

  it('does nothing when called twice', () => {
    setSearch(`?${USER_SHARE_ID_HEADER_NAME}=abc`);
    removeUserShareId();
    removeUserShareId();
    expect(window.location.search).toBe('');
  });
});
