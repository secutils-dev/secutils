import { useContext } from 'react';

import { AppContext } from '../app_container';

export function useAppContext() {
  const appContext = useContext(AppContext);
  if (!appContext) {
    throw new Error('App context provider is not found.');
  }

  return appContext;
}
