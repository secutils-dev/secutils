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
