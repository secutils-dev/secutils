// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';

import { getApiRequestConfig, getApiUrl } from './urls';
import { USER_SHARE_ID_HEADER_NAME } from './user_share';

function setSearch(search: string) {
  window.history.replaceState(null, '', `${window.location.pathname}${search}`);
}

afterEach(() => {
  setSearch('');
});

describe('getApiUrl', () => {
  it('returns the path unchanged', () => {
    expect(getApiUrl('/api/test')).toBe('/api/test');
    expect(getApiUrl('/api/user/data?namespace=settings')).toBe('/api/user/data?namespace=settings');
  });
});

describe('getApiRequestConfig', () => {
  it('defaults to GET method', () => {
    const config = getApiRequestConfig();
    expect(config.method).toBe('GET');
  });

  it('uses the specified method', () => {
    expect(getApiRequestConfig('POST').method).toBe('POST');
    expect(getApiRequestConfig('PUT').method).toBe('PUT');
    expect(getApiRequestConfig('DELETE').method).toBe('DELETE');
  });

  it('always includes Content-Type header', () => {
    const config = getApiRequestConfig();
    expect((config.headers as Record<string, string>)['Content-Type']).toBe('application/json');
  });

  it('does not include share header when no share id in URL', () => {
    setSearch('');
    const config = getApiRequestConfig();
    expect((config.headers as Record<string, string>)[USER_SHARE_ID_HEADER_NAME]).toBeUndefined();
  });

  it('includes share header when share id is in URL', () => {
    setSearch(`?${USER_SHARE_ID_HEADER_NAME}=share-42`);
    const config = getApiRequestConfig('GET');
    expect((config.headers as Record<string, string>)[USER_SHARE_ID_HEADER_NAME]).toBe('share-42');
  });
});
