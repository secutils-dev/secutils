import { ResponseError } from './errors';
import { apiFetch } from './urls';
import type { EntityTag } from './user_tags';

export type UserScriptType = 'responder' | 'api_configurator' | 'api_extractor' | 'page_extractor' | 'universal';

export interface UserScript {
  id: string;
  name: string;
  scriptType: UserScriptType;
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}

export interface UserScriptWithContent extends UserScript {
  content: string;
}

export async function getUserScripts(context?: 'responder' | 'api_tracker' | 'page_tracker'): Promise<UserScript[]> {
  const path = context ? `/api/user/scripts?context=${context}` : '/api/user/scripts';
  const response = await apiFetch(path);
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to fetch scripts.');
  }
  return response.json();
}

export async function getUserScript(id: string): Promise<UserScriptWithContent> {
  const response = await apiFetch(`/api/user/scripts/${encodeURIComponent(id)}`);
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to fetch script.');
  }
  return response.json();
}

export async function createUserScript(
  name: string,
  scriptType: UserScriptType,
  content: string,
  tagIds?: string[],
): Promise<UserScript> {
  const response = await apiFetch('/api/user/scripts', {
    method: 'POST',
    body: JSON.stringify({ name, scriptType, content, ...(tagIds !== undefined ? { tagIds } : {}) }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to create script.');
  }
  return response.json();
}

export async function updateUserScript(id: string, content: string, tagIds?: string[]): Promise<UserScript> {
  const response = await apiFetch(`/api/user/scripts/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify({ content, ...(tagIds !== undefined ? { tagIds } : {}) }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to update script.');
  }
  return response.json();
}

export async function deleteUserScript(id: string): Promise<void> {
  const response = await apiFetch(`/api/user/scripts/${encodeURIComponent(id)}`, { method: 'DELETE' });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to delete script.');
  }
}

export const USER_SCRIPT_TYPE_LABELS: Record<UserScriptType, string> = {
  responder: 'Responder',
  api_configurator: 'API Configurator',
  api_extractor: 'API Extractor',
  page_extractor: 'Page Extractor',
  universal: 'Universal',
};

export const USER_SCRIPT_TYPE_OPTIONS: { value: UserScriptType; text: string }[] = [
  { value: 'responder', text: USER_SCRIPT_TYPE_LABELS.responder },
  { value: 'api_configurator', text: USER_SCRIPT_TYPE_LABELS.api_configurator },
  { value: 'api_extractor', text: USER_SCRIPT_TYPE_LABELS.api_extractor },
  { value: 'page_extractor', text: USER_SCRIPT_TYPE_LABELS.page_extractor },
  { value: 'universal', text: USER_SCRIPT_TYPE_LABELS.universal },
];
