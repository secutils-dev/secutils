// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  createUserSecret,
  deleteUserSecret,
  getUserSecrets,
  getUserSecretsPage,
  updateUserSecret,
} from './user_secrets';

let mockFetch: ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockFetch = vi.fn();
  vi.stubGlobal('fetch', mockFetch);
});

afterEach(() => {
  vi.restoreAllMocks();
});

const SECRET: { id: string; name: string; createdAt: number; updatedAt: number } = {
  id: '00000000-0000-0000-0000-000000000001',
  name: 'my-secret',
  createdAt: 1700000000,
  updatedAt: 1700000001,
};

describe('getUserSecretsPage', () => {
  it('returns a page and requests the default endpoint without params', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ items: [SECRET], total: 1 }), { status: 200 }));

    const page = await getUserSecretsPage();
    expect(page).toEqual({ items: [SECRET], total: 1 });
    expect(mockFetch).toHaveBeenCalledWith('/api/user/secrets', expect.objectContaining({ method: 'GET' }));
  });

  it('serializes pagination, sort, search, and tag filters into the query string', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ items: [], total: 0 }), { status: 200 }));

    await getUserSecretsPage({
      page: 2,
      pageSize: 25,
      sort: 'updatedAt',
      order: 'desc',
      q: 'token',
      tags: ['t1', 't2'],
      globalTags: ['g1'],
    });

    const [url] = mockFetch.mock.calls[0];
    expect(url).toBe(
      '/api/user/secrets?page=2&pageSize=25&sort=updatedAt&order=desc&q=token&tags=t1%2Ct2&globalTags=g1',
    );
  });

  it('throws when response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(new Response(null, { status: 500 }));
    await expect(getUserSecretsPage()).rejects.toThrow('Failed to fetch secrets.');
  });
});

describe('getUserSecrets', () => {
  it('aggregates every page into a flat list', async () => {
    const second = { ...SECRET, id: '00000000-0000-0000-0000-000000000002', name: 'other' };
    mockFetch
      .mockResolvedValueOnce(new Response(JSON.stringify({ items: [SECRET], total: 2 }), { status: 200 }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ items: [second], total: 2 }), { status: 200 }));

    const secrets = await getUserSecrets();
    expect(secrets).toEqual([SECRET, second]);
    expect(mockFetch).toHaveBeenCalledTimes(2);
    expect(mockFetch.mock.calls[0][0]).toBe('/api/user/secrets?page=0&pageSize=100');
    expect(mockFetch.mock.calls[1][0]).toBe('/api/user/secrets?page=1&pageSize=100');
  });

  it('throws when a page response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(new Response(null, { status: 500 }));
    await expect(getUserSecrets()).rejects.toThrow('Failed to fetch secrets.');
  });
});

describe('createUserSecret', () => {
  it('creates a secret and returns it', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify(SECRET), { status: 200 }));

    const result = await createUserSecret('my-secret', 's3cret');
    expect(result).toEqual(SECRET);

    const [url, config] = mockFetch.mock.calls[0];
    expect(url).toBe('/api/user/secrets');
    expect(config.method).toBe('POST');
    expect(JSON.parse(config.body)).toEqual({ name: 'my-secret', value: 's3cret' });
  });

  it('throws with server message when response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ message: 'Name already exists' }), { status: 409 }));
    await expect(createUserSecret('dup', 'val')).rejects.toThrow('Name already exists');
  });

  it('throws fallback message when response body is not JSON', async () => {
    mockFetch.mockResolvedValueOnce(new Response('not json', { status: 500 }));
    await expect(createUserSecret('x', 'y')).rejects.toThrow('Failed to create secret.');
  });
});

describe('updateUserSecret', () => {
  it('updates a secret and returns it', async () => {
    const updated = { ...SECRET, updatedAt: 1700000099 };
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify(updated), { status: 200 }));

    const result = await updateUserSecret('00000000-0000-0000-0000-000000000001', 'new-value');
    expect(result).toEqual(updated);

    const [url, config] = mockFetch.mock.calls[0];
    expect(url).toBe('/api/user/secrets/00000000-0000-0000-0000-000000000001');
    expect(config.method).toBe('PUT');
    expect(JSON.parse(config.body)).toEqual({ value: 'new-value' });
  });

  it('throws with server message when response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ message: 'Not found' }), { status: 404 }));
    await expect(updateUserSecret('00000000-0000-0000-0000-000000000001', 'y')).rejects.toThrow('Not found');
  });

  it('throws fallback message when response body is not JSON', async () => {
    mockFetch.mockResolvedValueOnce(new Response('bad', { status: 500 }));
    await expect(updateUserSecret('00000000-0000-0000-0000-000000000001', 'y')).rejects.toThrow(
      'Failed to update secret.',
    );
  });
});

describe('deleteUserSecret', () => {
  it('deletes a secret successfully', async () => {
    mockFetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
    await expect(deleteUserSecret('00000000-0000-0000-0000-000000000001')).resolves.toBeUndefined();

    const [url, config] = mockFetch.mock.calls[0];
    expect(url).toBe('/api/user/secrets/00000000-0000-0000-0000-000000000001');
    expect(config.method).toBe('DELETE');
  });

  it('throws with server message when response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ message: 'Forbidden' }), { status: 403 }));
    await expect(deleteUserSecret('00000000-0000-0000-0000-000000000001')).rejects.toThrow('Forbidden');
  });

  it('throws fallback message when response body is not JSON', async () => {
    mockFetch.mockResolvedValueOnce(new Response('nope', { status: 500 }));
    await expect(deleteUserSecret('00000000-0000-0000-0000-000000000001')).rejects.toThrow('Failed to delete secret.');
  });
});
