import { createContext } from 'react';
import type { Dispatch } from 'react';

import type { UiState, UserSettings } from '../model';
import type { PageToast } from '../pages/page';

export interface AppContextValue {
  uiState: UiState;
  refreshUiState: () => void;
  settings?: UserSettings;
  setSettings: Dispatch<UserSettings>;
  addToast: (toast: PageToast) => void;
}

export const AppContext = createContext<AppContextValue | undefined>(undefined);
