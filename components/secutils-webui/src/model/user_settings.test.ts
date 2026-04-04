// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ResponseError } from './errors';
import {
  getUserSettings,
  parseUserUiThemePreference,
  setUserSettings,
  USER_SETTINGS_DEFAULT_UI_THEME,
} from './user_settings';

let mockFetch: ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockFetch = vi.fn();
  vi.stubGlobal('fetch', mockFetch);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('getUserSettings', () => {
  it('returns settings directly from the response', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ theme: 'dark' }), { status: 200 }));

    const result = await getUserSettings();
    expect(result).toEqual({ theme: 'dark' });

    expect(mockFetch).toHaveBeenCalledWith('/api/user/settings', expect.objectContaining({ method: 'GET' }));
  });

  it('returns null when the response body is null', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify(null), { status: 200 }));

    const result = await getUserSettings();
    expect(result).toBeNull();
  });

  it('throws ResponseError when the response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ message: 'Forbidden' }), { status: 403, statusText: 'Forbidden' }),
    );

    await expect(getUserSettings()).rejects.toThrow(ResponseError);
  });

  it('redirects to sign-in on 401', async () => {
    const replaceMock = vi.fn();
    vi.stubGlobal('location', { ...window.location, replace: replaceMock });

    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ message: 'Unauthorized' }), { status: 401, statusText: 'Unauthorized' }),
    );

    const settingsPromise = getUserSettings();

    // The promise should never resolve (redirect happens instead).
    const result = await Promise.race([settingsPromise, new Promise((r) => setTimeout(() => r('timeout'), 100))]);
    expect(result).toBe('timeout');
    expect(replaceMock).toHaveBeenCalledWith('/signin');
  });
});

describe('parseUserUiThemePreference', () => {
  it('returns system for missing or invalid values', () => {
    expect(parseUserUiThemePreference(undefined)).toBe(USER_SETTINGS_DEFAULT_UI_THEME);
    expect(parseUserUiThemePreference(null)).toBe(USER_SETTINGS_DEFAULT_UI_THEME);
    expect(parseUserUiThemePreference('')).toBe(USER_SETTINGS_DEFAULT_UI_THEME);
    expect(parseUserUiThemePreference('auto')).toBe(USER_SETTINGS_DEFAULT_UI_THEME);
  });

  it('returns light, dark, and system when valid', () => {
    expect(parseUserUiThemePreference('light')).toBe('light');
    expect(parseUserUiThemePreference('dark')).toBe('dark');
    expect(parseUserUiThemePreference('system')).toBe('system');
  });
});

describe('setUserSettings', () => {
  it('sends data directly and returns settings from the response', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({ theme: 'light' }), { status: 200 }));

    const result = await setUserSettings({ theme: 'light' });
    expect(result).toEqual({ theme: 'light' });

    const [url, config] = mockFetch.mock.calls[0];
    expect(url).toBe('/api/user/settings');
    expect(config.method).toBe('POST');
    expect(JSON.parse(config.body)).toEqual({ theme: 'light' });
  });

  it('returns null when the response body is null', async () => {
    mockFetch.mockResolvedValueOnce(new Response(JSON.stringify(null), { status: 200 }));

    const result = await setUserSettings({ key: 'value' });
    expect(result).toBeNull();
  });

  it('throws ResponseError when the response is not ok', async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ message: 'Server error' }), { status: 500, statusText: 'Internal Server Error' }),
    );

    await expect(setUserSettings({})).rejects.toThrow(ResponseError);
  });
});
