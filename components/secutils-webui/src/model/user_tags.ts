import { ResponseError } from './errors';
import { apiFetch } from './urls';

export interface EntityTag {
  id: string;
  name: string;
  color: string;
}

export interface UserTag extends EntityTag {
  createdAt: number;
  updatedAt: number;
}

export const TAG_COLOR_SWATCHES = [
  '#54B399',
  '#6092C0',
  '#D36086',
  '#9170B8',
  '#E7664C',
  '#DA8B45',
  '#D6BF57',
  '#B9A888',
  '#CA8EAE',
  '#AA6556',
] as const;

export async function getUserTags(): Promise<UserTag[]> {
  const response = await apiFetch('/api/user/tags');
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to fetch tags.');
  }
  return response.json();
}

export async function createUserTag(name: string, color: string): Promise<UserTag> {
  const response = await apiFetch('/api/user/tags', {
    method: 'POST',
    body: JSON.stringify({ name, color }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to create tag.');
  }
  return response.json();
}

export async function updateUserTag(id: string, params: { name?: string; color?: string }): Promise<UserTag> {
  const response = await apiFetch(`/api/user/tags/${id}`, {
    method: 'PUT',
    body: JSON.stringify(params),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to update tag.');
  }
  return response.json();
}

export async function deleteUserTag(id: string): Promise<void> {
  const response = await apiFetch(`/api/user/tags/${id}`, { method: 'DELETE' });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to delete tag.');
  }
}
