import { useCallback, useEffect, useState } from 'react';
import { Outlet, useLocation } from 'react-router';

import type { EuiThemeColorMode } from '@elastic/eui';
import { EuiGlobalToastList, EuiProvider } from '@elastic/eui';
import type { Toast } from '@elastic/eui/src/components/toast/global_toast_list';
import axios from 'axios';

import { AppContext } from './app_context';
import { useLocalStorage } from '../hooks';
import type { UiState, UserSettings } from '../model';
import {
  getApiRequestConfig,
  getApiUrl,
  getErrorMessage,
  getUserShareId,
  removeUserShareId,
  setUserData,
  USER_SETTINGS_KEY_COMMON_UI_THEME,
  USER_SETTINGS_USER_DATA_TYPE,
} from '../model';
import type { PageToast } from '../pages/page';

export function AppContainer() {
  const location = useLocation();
  const [isUiStateRefreshInProgress, setIsUiStateRefreshInProgress] = useState(false);
  const [uiState, setUiState] = useState<UiState>({
    synced: false,
    status: { level: 'available' },
    license: { maxEndpoints: Infinity },
    utils: [],
    webhookUrlType: 'path',
  });
  const refreshUiState = useCallback(() => {
    if (isUiStateRefreshInProgress) {
      return;
    }

    setIsUiStateRefreshInProgress(true);

    axios.get(getApiUrl('/api/ui/state'), getApiRequestConfig()).then(
      ({ data }: { data: UiState }) => {
        setUiState({ ...data, synced: true });

        if (data.settings) {
          setSettings(data.settings);
          setLocalSettings(data.settings);
        }

        // Remove user share ID from URL if it's not valid anymore.
        if (!data.userShare) {
          removeUserShareId();
        }

        setIsUiStateRefreshInProgress(false);
      },
      () => {
        setUiState({ ...uiState, status: { level: 'unavailable' }, synced: true });
        setIsUiStateRefreshInProgress(false);
      },
    );
  }, [isUiStateRefreshInProgress]);
  useEffect(refreshUiState, []);

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
  }, [location.search, uiState]);

  // Settings aren't sensitive data, so we can duplicate them in the local storage to improve overall responsiveness.
  const [localSettings, setLocalSettings] = useLocalStorage<UserSettings | undefined>('settings', undefined);
  const [settings, setSettings] = useState<UserSettings | undefined>(localSettings);
  const updateSettings = useCallback((settingsToUpdate: Record<string, unknown>) => {
    setSettings((currentSettings) => ({ ...currentSettings, ...settingsToUpdate }));
    setLocalSettings((currentSettings) => ({ ...currentSettings, ...settingsToUpdate }));

    setUserData<UserSettings>(USER_SETTINGS_USER_DATA_TYPE, settingsToUpdate)
      .then((settings) => {
        setSettings(settings ?? undefined);
        setLocalSettings(settings ?? undefined);
      })
      .catch((err: Error) => {
        console.error(`Failed update user settings: ${getErrorMessage(err)}`);
      });
  }, []);

  const uiTheme = settings?.[USER_SETTINGS_KEY_COMMON_UI_THEME] as EuiThemeColorMode | undefined;
  const [toasts, setToasts] = useState<PageToast[]>([]);
  const addToast = useCallback((toast: PageToast) => {
    setToasts((currentToasts) => [...currentToasts, toast]);
  }, []);
  const removeToast = useCallback((removedToast: Toast) => {
    setToasts((currentToasts) => currentToasts.filter((toast) => toast.id !== removedToast.id));
  }, []);

  return (
    <EuiProvider colorMode={uiTheme}>
      <AppContext.Provider value={{ uiState, refreshUiState, settings, setSettings: updateSettings, addToast }}>
        <Outlet />
      </AppContext.Provider>
      <EuiGlobalToastList toasts={toasts} dismissToast={removeToast} toastLifeTimeMs={5000} />
    </EuiProvider>
  );
}
