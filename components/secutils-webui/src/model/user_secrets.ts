import { ResponseError } from './errors';
import { buildPaginationQuery, fetchAllItems } from './pagination';
import type { Page, PaginationRequest } from './pagination';
import { apiFetch } from './urls';
import type { EntityTag } from './user_tags';

export interface UserSecret {
  id: string;
  name: string;
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}

/** Fetches a single page of secrets honoring search, tag, sort, and paging. */
export async function getUserSecretsPage(params: PaginationRequest = {}): Promise<Page<UserSecret>> {
  const response = await apiFetch(`/api/user/secrets${buildPaginationQuery(params)}`);
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to fetch secrets.');
  }
  return response.json();
}

/** Fetches every secret across all pages (used by export and other bulk flows). */
export async function getUserSecrets(): Promise<UserSecret[]> {
  return fetchAllItems(getUserSecretsPage);
}

export async function createUserSecret(name: string, value: string, tagIds?: string[]): Promise<UserSecret> {
  const response = await apiFetch('/api/user/secrets', {
    method: 'POST',
    body: JSON.stringify({ name, value, ...(tagIds !== undefined ? { tagIds } : {}) }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to create secret.');
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
  const response = await apiFetch(`/api/user/secrets/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to update secret.');
  }
  return response.json();
}

export async function deleteUserSecret(id: string): Promise<void> {
  const response = await apiFetch(`/api/user/secrets/${encodeURIComponent(id)}`, { method: 'DELETE' });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to delete secret.');
  }
}
