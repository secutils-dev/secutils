import { getApiRequestConfig, getApiUrl } from './urls';
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
  const url = context ? getApiUrl(`/api/user/scripts?context=${context}`) : getApiUrl('/api/user/scripts');
  const response = await fetch(url, getApiRequestConfig('GET'));
  if (!response.ok) {
    throw new Error('Failed to fetch scripts.');
  }
  return response.json();
}

export async function getUserScript(id: string): Promise<UserScriptWithContent> {
  const response = await fetch(getApiUrl(`/api/user/scripts/${encodeURIComponent(id)}`), getApiRequestConfig('GET'));
  if (!response.ok) {
    throw new Error('Failed to fetch script.');
  }
  return response.json();
}

export async function createUserScript(
  name: string,
  scriptType: UserScriptType,
  content: string,
  tagIds?: string[],
): Promise<UserScript> {
  const response = await fetch(getApiUrl('/api/user/scripts'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify({ name, scriptType, content, ...(tagIds !== undefined ? { tagIds } : {}) }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to create script.');
  }
  return response.json();
}

export async function updateUserScript(id: string, content: string, tagIds?: string[]): Promise<UserScript> {
  const response = await fetch(getApiUrl(`/api/user/scripts/${encodeURIComponent(id)}`), {
    ...getApiRequestConfig('PUT'),
    body: JSON.stringify({ content, ...(tagIds !== undefined ? { tagIds } : {}) }),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to update script.');
  }
  return response.json();
}

export async function deleteUserScript(id: string): Promise<void> {
  const response = await fetch(getApiUrl(`/api/user/scripts/${encodeURIComponent(id)}`), getApiRequestConfig('DELETE'));
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to delete script.');
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
