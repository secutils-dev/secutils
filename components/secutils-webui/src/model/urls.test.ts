// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./user_share', () => ({
  USER_SHARE_ID_HEADER_NAME: 'x-user-share-id',
  getUserShareId: vi.fn(),
}));

import { apiFetch, getApiRequestConfig, getApiUrl } from './urls';
import { getUserShareId, USER_SHARE_ID_HEADER_NAME } from './user_share';

let mockFetch: ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockFetch = vi.fn();
  vi.stubGlobal('fetch', mockFetch);
  vi.mocked(getUserShareId).mockReturnValue(null);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('getApiUrl', () => {
  it('returns path unchanged', () => {
    expect(getApiUrl('/api/foo')).toBe('/api/foo');
    expect(getApiUrl('')).toBe('');
  });
});

describe('getApiRequestConfig', () => {
  it('defaults to GET, includes Content-Type header', () => {
    const config = getApiRequestConfig();
    expect(config.method).toBe('GET');
    expect(config.headers).toEqual({ 'Content-Type': 'application/json' });
  });

  it('respects explicit method', () => {
    expect(getApiRequestConfig('POST').method).toBe('POST');
    expect(getApiRequestConfig('PUT').method).toBe('PUT');
    expect(getApiRequestConfig('DELETE').method).toBe('DELETE');
  });

  it('includes share ID header when getUserShareId returns a value', () => {
    vi.mocked(getUserShareId).mockReturnValue('share-abc');
    const config = getApiRequestConfig('GET');
    expect(config.headers).toEqual({
      [USER_SHARE_ID_HEADER_NAME]: 'share-abc',
      'Content-Type': 'application/json',
    });
  });
});

describe('apiFetch', () => {
  let replaceMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    replaceMock = vi.fn();
    vi.stubGlobal('location', { ...window.location, replace: replaceMock });
  });

  it('calls fetch with correct URL and merged config', async () => {
    mockFetch.mockResolvedValueOnce(new Response(null, { status: 200 }));

    await apiFetch('/api/foo');

    expect(mockFetch).toHaveBeenCalledTimes(1);
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe('/api/foo');
    expect(init).toEqual(
      expect.objectContaining({
        method: 'GET',
        headers: { 'Content-Type': 'application/json' },
      }),
    );
  });

  it('defaults to GET method', async () => {
    mockFetch.mockResolvedValueOnce(new Response(null, { status: 200 }));

    await apiFetch('/api/items');

    const [, init] = mockFetch.mock.calls[0];
    expect(init.method).toBe('GET');
  });

  it('passes custom init (method, body) through', async () => {
    mockFetch.mockResolvedValueOnce(new Response(null, { status: 200 }));

    await apiFetch('/api/items', { method: 'POST', body: '{"a":1}' });

    const [, init] = mockFetch.mock.calls[0];
    expect(init.method).toBe('POST');
    expect(init.body).toBe('{"a":1}');
  });

  it('on 401, calls window.location.replace("/signin") and returns a never-resolving promise', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ message: 'Unauthorized' }), { status: 401 }));

    const fetchPromise = apiFetch('/api/secret');

    const result = await Promise.race([fetchPromise, new Promise((r) => setTimeout(() => r('timeout'), 100))]);
    expect(result).toBe('timeout');
    expect(replaceMock).toHaveBeenCalledWith('/signin');
  });

  it('on non-401 error (e.g. 500), returns the response normally (does not redirect)', async () => {
    const errorResponse = new Response(JSON.stringify({ message: 'Server error' }), {
      status: 500,
      statusText: 'Internal Server Error',
    });
    mockFetch.mockResolvedValueOnce(errorResponse);

    const response = await apiFetch('/api/broken');

    expect(response.status).toBe(500);
    expect(replaceMock).not.toHaveBeenCalled();
  });
});
