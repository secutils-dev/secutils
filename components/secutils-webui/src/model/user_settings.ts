import { ResponseError } from './errors';
import { getApiRequestConfig, getApiUrl } from './urls';

export const USER_SETTINGS_KEY_COMMON_SHOW_ONLY_FAVORITES = 'common.showOnlyFavorites';
export const USER_SETTINGS_KEY_COMMON_FAVORITES = 'common.favorites';
export const USER_SETTINGS_KEY_COMMON_UI_THEME = 'common.uiTheme';
export const USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED = 'common.sidebarCollapsed';
export const USER_SETTINGS_KEY_COMMON_GLOBAL_SCOPE_TAG_IDS = 'common.globalScopeTagIds';

export type UserSettings = Record<string, unknown>;

export async function getUserSettings(): Promise<UserSettings | null> {
  const response = await fetch(getApiUrl('/api/user/settings'), getApiRequestConfig());
  if (!response.ok) {
    throw await ResponseError.fromResponse(response);
  }
  return (await response.json()) as UserSettings | null;
}

export async function setUserSettings(dataValue: unknown): Promise<UserSettings | null> {
  const response = await fetch(getApiUrl('/api/user/settings'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify(dataValue),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response);
  }
  return (await response.json()) as UserSettings | null;
}
