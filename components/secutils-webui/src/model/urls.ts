import { getUserShareId, USER_SHARE_ID_HEADER_NAME } from './user_share';

/**
 * Takes API URL path and returns it back with any environment modifications if needed.
 * @param path API endpoint relative path.
 */
export function getApiUrl(path: string) {
  return path;
}

export function getApiRequestConfig(method: 'GET' | 'POST' | 'PUT' | 'DELETE' = 'GET'): Partial<RequestInit> {
  const shareId = getUserShareId();
  return shareId
    ? { method, headers: { [USER_SHARE_ID_HEADER_NAME]: shareId, 'Content-Type': 'application/json' } }
    : { method, headers: { 'Content-Type': 'application/json' } };
}

/**
 * Fetch wrapper for API calls that redirects to the sign-in page on 401 (expired session).
 * Use this instead of raw `fetch(getApiUrl(...), getApiRequestConfig(...))` for all
 * authenticated API endpoints. The only exception is `/api/ui/state` which accepts
 * unauthenticated requests and never returns 401.
 */
export async function apiFetch(path: string, init?: Partial<RequestInit>): Promise<Response> {
  const method = (init?.method ?? 'GET') as 'GET' | 'POST' | 'PUT' | 'DELETE';
  const response = await fetch(getApiUrl(path), { ...getApiRequestConfig(method), ...init });
  if (response.status === 401) {
    window.location.replace('/signin');
    return new Promise(() => {});
  }
  return response;
}
