import { useContext } from 'react';

import { useAppContext } from '../../../hooks';
import { WorkspaceContext } from '../workspace_context';

export function useWorkspaceContext() {
  const appContext = useAppContext();

  const workspaceContext = useContext(WorkspaceContext);
  if (!workspaceContext) {
    throw new Error('Workspace context provider is not found.');
  }

  return { ...appContext, ...workspaceContext };
}
