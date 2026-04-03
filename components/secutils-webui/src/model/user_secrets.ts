import { getApiRequestConfig, getApiUrl } from './urls';
import type { EntityTag } from './user_tags';

export interface UserSecret {
  id: string;
  name: string;
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}

export async function getUserSecrets(): Promise<UserSecret[]> {
  const response = await fetch(getApiUrl('/api/user/secrets'), getApiRequestConfig('GET'));
  if (!response.ok) {
    throw new Error('Failed to fetch secrets.');
  }
  return response.json();
}

export async function createUserSecret(name: string, value: string, tagIds?: string[]): Promise<UserSecret> {
  const response = await fetch(getApiUrl('/api/user/secrets'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify({ name, value, ...(tagIds !== undefined ? { tagIds } : {}) }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to create secret.');
  }
  return response.json();
}

export async function updateUserSecret(id: string, value?: string, tagIds?: string[]): Promise<UserSecret> {
  const body: Record<string, unknown> = {};
  if (value !== undefined) {
    body.value = value;
  }
  if (tagIds !== undefined) {
    body.tagIds = tagIds;
  }
  const response = await fetch(getApiUrl(`/api/user/secrets/${encodeURIComponent(id)}`), {
    ...getApiRequestConfig('PUT'),
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to update secret.');
  }
  return response.json();
}

export async function deleteUserSecret(id: string): Promise<void> {
  const response = await fetch(getApiUrl(`/api/user/secrets/${encodeURIComponent(id)}`), getApiRequestConfig('DELETE'));
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to delete secret.');
  }
}
