import { useEffect, useState } from 'react';

import { createLatestLoader } from './latest_loader';

/**
 * Loads full entities on demand (e.g., before opening an editor from a summary grid). The latest
 * load wins, superseded loads are aborted, and unmounting cancels any load in progress.
 *
 * `fetchEntity` is captured once, so pass a stable function.
 */
export function useEntityLoader<T>(fetchEntity: (id: string, signal: AbortSignal) => Promise<T>) {
  const [loading, setLoading] = useState(false);
  const [loader] = useState(() => createLatestLoader(fetchEntity, setLoading));
  useEffect(() => () => loader.cancel(), [loader]);
  return { load: loader.load, cancel: loader.cancel, loading };
}
