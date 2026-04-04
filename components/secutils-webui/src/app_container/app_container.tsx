import type { EuiGlobalToastListToast, EuiThemeColorMode } from '@elastic/eui';
import { EuiGlobalToastList, EuiProvider } from '@elastic/eui';
import { useCallback, useEffect, useState } from 'react';
import { Outlet, useLocation } from 'react-router';

import { AppContext } from './app_context';
import { useLocalStorage } from '../hooks';
import type { UiState, UserSettings } from '../model';
import {
  getApiRequestConfig,
  getApiUrl,
  getErrorMessage,
  getUserShareId,
  parseUserUiThemePreference,
  removeUserShareId,
  ResponseError,
  setUserSettings,
  USER_SETTINGS_KEY_COMMON_UI_THEME,
} from '../model';
import type { PageToast } from '../pages/page';

export function AppContainer() {
  const location = useLocation();

  // Settings aren't sensitive data, so we can duplicate them in the local storage to improve overall responsiveness.
  const [localSettings, setLocalSettings] = useLocalStorage<UserSettings | undefined>('settings', undefined);

  const [settings, setSettings] = useState<UserSettings | undefined>(localSettings);

  const [uiState, setUiState] = useState<UiState>({
    synced: false,
    status: { level: 'available' },
    license: { maxEndpoints: Infinity },
    utils: [],
    webhookUrlType: 'path',
    platform: { maxImportFileSize: 10 * 1024 * 1024 },
  });
  const refreshUiState = useCallback(() => {
    fetch(getApiUrl('/api/ui/state'), getApiRequestConfig())
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        const data = (await res.json()) as UiState;
        setUiState({ ...data, synced: true });

        if (data.settings) {
          setSettings(data.settings);
          setLocalSettings(data.settings);
        }

        // Remove user share ID from URL if it's not valid anymore.
        if (!data.userShare) {
          removeUserShareId();
        }
      })
      .catch(() =>
        setUiState((currentUiState) => ({ ...currentUiState, status: { level: 'unavailable' }, synced: true })),
      );
  }, [setLocalSettings]);
  useEffect(refreshUiState, [refreshUiState]);

  // Track share context and refresh UI state if it changes.
  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    const shareId = getUserShareId();
    const shareContextHasChanged =
      (uiState.userShare && uiState.userShare.id !== shareId) || (!uiState.userShare && shareId);
    if (shareContextHasChanged) {
      refreshUiState();
    }
  }, [location.search, uiState, refreshUiState]);

  const updateSettings = useCallback(
    (settingsToUpdate: Record<string, unknown>) => {
      setSettings((currentSettings) => ({ ...currentSettings, ...settingsToUpdate }));
      setLocalSettings((currentSettings) => ({ ...currentSettings, ...settingsToUpdate }));

      setUserSettings(settingsToUpdate)
        .then((settings) => {
          setSettings(settings ?? undefined);
          setLocalSettings(settings ?? undefined);
        })
        .catch((err: Error) => {
          console.error(`Failed update user settings: ${getErrorMessage(err)}`);
        });
    },
    [setLocalSettings],
  );

  const uiThemePreference = parseUserUiThemePreference(settings?.[USER_SETTINGS_KEY_COMMON_UI_THEME]);
  const euiColorMode: EuiThemeColorMode | undefined = uiThemePreference === 'system' ? undefined : uiThemePreference;
  const [toasts, setToasts] = useState<PageToast[]>([]);
  const addToast = useCallback((toast: PageToast) => {
    setToasts((currentToasts) => [...currentToasts, toast]);
  }, []);
  const removeToast = useCallback((removedToast: EuiGlobalToastListToast) => {
    setToasts((currentToasts) => currentToasts.filter((toast) => toast.id !== removedToast.id));
  }, []);

  return (
    <EuiProvider colorMode={euiColorMode}>
      <AppContext.Provider value={{ uiState, refreshUiState, settings, setSettings: updateSettings, addToast }}>
        <Outlet />
      </AppContext.Provider>
      <EuiGlobalToastList toasts={toasts} dismissToast={removeToast} toastLifeTimeMs={5000} />
    </EuiProvider>
  );
}
