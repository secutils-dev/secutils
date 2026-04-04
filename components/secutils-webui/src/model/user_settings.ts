import { ResponseError } from './errors';
import { apiFetch } from './urls';

export const USER_SETTINGS_KEY_COMMON_UI_THEME = 'common.uiTheme';
export const USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED = 'common.sidebarCollapsed';
export const USER_SETTINGS_KEY_COMMON_GLOBAL_SCOPE_TAG_IDS = 'common.globalScopeTagIds';

export type UserSettings = Record<string, unknown>;

export interface SidebarCollapsedState {
  nav: boolean;
  sections: string[];
}

/** Parse the `common.sidebarCollapsed` setting (`{ nav?: boolean, sections?: string[] }`). */
export function parseSidebarCollapsed(value: unknown): SidebarCollapsedState {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    const obj = value as Record<string, unknown>;
    return {
      nav: typeof obj.nav === 'boolean' ? obj.nav : false,
      sections: Array.isArray(obj.sections) ? (obj.sections as string[]) : [],
    };
  }
  return { nav: false, sections: [] };
}

export async function getUserSettings(): Promise<UserSettings | null> {
  const response = await apiFetch('/api/user/settings');
  if (!response.ok) {
    throw await ResponseError.fromResponse(response);
  }
  return (await response.json()) as UserSettings | null;
}

export async function setUserSettings(dataValue: unknown): Promise<UserSettings | null> {
  const response = await apiFetch('/api/user/settings', {
    method: 'POST',
    body: JSON.stringify(dataValue),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response);
  }
  return (await response.json()) as UserSettings | null;
}
