export interface LatestLoader<T> {
  /**
   * Loads the entity with the given ID, aborting any load in progress. Only the latest load may
   * invoke its callbacks: superseded, cancelled, and aborted loads are dropped silently.
   */
  load: (id: string, onSuccess: (entity: T) => void, onError: (err: unknown) => void) => void;
  /** Aborts and invalidates the load in progress, if any. */
  cancel: () => void;
}

/**
 * Creates a "latest wins" loader for full entities, used by summary grids to fetch the complete
 * entity before opening an editor. `onLoadingChange` reports whether a load is in progress.
 */
export function createLatestLoader<T>(
  fetchEntity: (id: string, signal: AbortSignal) => Promise<T>,
  onLoadingChange: (loading: boolean) => void,
): LatestLoader<T> {
  let active: AbortController | null = null;

  const settle = (controller: AbortController) => {
    if (active !== controller) {
      return false;
    }
    active = null;
    onLoadingChange(false);
    return true;
  };

  return {
    load(id, onSuccess, onError) {
      active?.abort();
      const controller = new AbortController();
      active = controller;
      onLoadingChange(true);

      let request: Promise<T>;
      try {
        request = fetchEntity(id, controller.signal);
      } catch (err) {
        request = Promise.reject(err);
      }

      request.then(
        (entity) => {
          if (settle(controller)) {
            onSuccess(entity);
          }
        },
        (err: unknown) => {
          if (settle(controller)) {
            onError(err);
          }
        },
      );
    },
    cancel() {
      if (active) {
        active.abort();
        active = null;
        onLoadingChange(false);
      }
    },
  };
}
