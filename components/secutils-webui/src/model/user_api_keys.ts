import { ResponseError } from './errors';
import { apiFetch } from './urls';

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
  const response = await apiFetch('/api/user/api_keys');
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to fetch API keys.');
  }
  return response.json();
}

export async function createUserApiKey(name: string, expiresAt?: number): Promise<ApiKeyCreateResponse> {
  const response = await apiFetch('/api/user/api_keys', {
    method: 'POST',
    body: JSON.stringify({ name, ...(expiresAt !== undefined ? { expiresAt } : {}) }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to create API key.');
  }
  return response.json();
}

export async function updateUserApiKey(id: string, name: string): Promise<UserApiKey> {
  const response = await apiFetch(`/api/user/api_keys/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify({ name }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to update API key.');
  }
  return response.json();
}

export async function deleteUserApiKey(id: string): Promise<void> {
  const response = await apiFetch(`/api/user/api_keys/${encodeURIComponent(id)}`, { method: 'DELETE' });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to delete API key.');
  }
}

export async function regenerateUserApiKey(id: string, expiresAt?: number): Promise<ApiKeyCreateResponse> {
  const response = await apiFetch(`/api/user/api_keys/${encodeURIComponent(id)}/_regenerate`, {
    method: 'POST',
    body: JSON.stringify(expiresAt !== undefined ? { expiresAt } : {}),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to regenerate API key.');
  }
  return response.json();
}
