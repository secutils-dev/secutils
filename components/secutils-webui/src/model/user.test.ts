// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ResponseError } from './errors';
import { getUserData, setUserData } from './user';

let mockFetch: ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockFetch = vi.fn();
  vi.stubGlobal('fetch', mockFetch);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('getUserData', () => {
  it('returns the value for the requested namespace', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ settings: { theme: 'dark' } }), { status: 200 }));

    const result = await getUserData<{ theme: string }>('settings');
    expect(result).toEqual({ theme: 'dark' });

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/user/data?namespace=settings',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('returns null when the namespace key is absent in the response', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({}), { status: 200 }));

    const result = await getUserData('missing');
    expect(result).toBeUndefined();
  });

  it('throws ResponseError when the response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ message: 'Unauthorized' }), { status: 401, statusText: 'Unauthorized' }),
    );

    await expect(getUserData('settings')).rejects.toThrow(ResponseError);
  });
});

describe('setUserData', () => {
  it('sends data and returns the namespace value', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ settings: { theme: 'light' } }), { status: 200 }));

    const result = await setUserData<{ theme: string }>('settings', { theme: 'light' });
    expect(result).toEqual({ theme: 'light' });

    const [url, config] = mockFetch.mock.calls[0];
    expect(url).toBe('/api/user/data?namespace=settings');
    expect(config.method).toBe('POST');
    expect(JSON.parse(config.body)).toEqual({ dataValue: JSON.stringify({ theme: 'light' }) });
  });

  it('returns null when the namespace key is absent in the response', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({}), { status: 200 }));

    const result = await setUserData('missing', 'value');
    expect(result).toBeUndefined();
  });

  it('throws ResponseError when the response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ message: 'Server error' }), { status: 500, statusText: 'Internal Server Error' }),
    );

    await expect(setUserData('settings', {})).rejects.toThrow(ResponseError);
  });
});
