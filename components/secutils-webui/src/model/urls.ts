import type { AxiosRequestConfig } from 'axios';

import { getUserShareId, USER_SHARE_ID_HEADER_NAME } from './user_share';

/**
 * Takes API URL path and returns it back with any environment modifications if needed.
 * @param path API endpoint relative path.
 */
export function getApiUrl(path: string) {
  return path;
}

export function getApiRequestConfig(): AxiosRequestConfig | undefined {
  const shareId = getUserShareId();
  return shareId ? { headers: { [USER_SHARE_ID_HEADER_NAME]: shareId } } : undefined;
}
