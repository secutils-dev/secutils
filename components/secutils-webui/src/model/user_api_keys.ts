import { getApiRequestConfig, getApiUrl } from './urls';

export interface UserApiKey {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
  expiresAt: number | null;
  lastUsedAt: number | null;
}

export interface ApiKeyCreateResponse extends UserApiKey {
  token: string;
}

export async function getUserApiKeys(): Promise<UserApiKey[]> {
  const response = await fetch(getApiUrl('/api/user/api_keys'), getApiRequestConfig('GET'));
  if (!response.ok) {
    throw new Error('Failed to fetch API keys.');
  }
  return response.json();
}

export async function createUserApiKey(name: string, expiresAt?: number): Promise<ApiKeyCreateResponse> {
  const response = await fetch(getApiUrl('/api/user/api_keys'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify({ name, ...(expiresAt !== undefined ? { expiresAt } : {}) }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to create API key.');
  }
  return response.json();
}

export async function updateUserApiKey(id: string, name: string): Promise<UserApiKey> {
  const response = await fetch(getApiUrl(`/api/user/api_keys/${encodeURIComponent(id)}`), {
    ...getApiRequestConfig('PUT'),
    body: JSON.stringify({ name }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to update API key.');
  }
  return response.json();
}

export async function deleteUserApiKey(id: string): Promise<void> {
  const response = await fetch(
    getApiUrl(`/api/user/api_keys/${encodeURIComponent(id)}`),
    getApiRequestConfig('DELETE'),
  );
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to delete API key.');
  }
}

export async function regenerateUserApiKey(id: string, expiresAt?: number): Promise<ApiKeyCreateResponse> {
  const response = await fetch(getApiUrl(`/api/user/api_keys/${encodeURIComponent(id)}/_regenerate`), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify(expiresAt !== undefined ? { expiresAt } : {}),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to regenerate API key.');
  }
  return response.json();
}
