import { getApiRequestConfig, getApiUrl } from './urls';

export interface UserSecret {
  name: string;
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

export async function createUserSecret(name: string, value: string): Promise<UserSecret> {
  const response = await fetch(getApiUrl('/api/user/secrets'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify({ name, value }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to create secret.');
  }
  return response.json();
}

export async function updateUserSecret(name: string, value: string): Promise<UserSecret> {
  const response = await fetch(getApiUrl(`/api/user/secrets/${encodeURIComponent(name)}`), {
    ...getApiRequestConfig('PUT'),
    body: JSON.stringify({ value }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to update secret.');
  }
  return response.json();
}

export async function deleteUserSecret(name: string): Promise<void> {
  const response = await fetch(
    getApiUrl(`/api/user/secrets/${encodeURIComponent(name)}`),
    getApiRequestConfig('DELETE'),
  );
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to delete secret.');
  }
}
