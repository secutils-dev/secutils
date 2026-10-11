import { describe, expect, it, vi } from 'vitest';

import { createLatestLoader } from './latest_loader';

function deferredFetch() {
  const calls: Array<{
    id: string;
    signal: AbortSignal;
    resolve: (value: string) => void;
    reject: (err: unknown) => void;
  }> = [];
  const fetchEntity = (id: string, signal: AbortSignal) =>
    new Promise<string>((resolve, reject) => {
      calls.push({ id, signal, resolve, reject });
    });
  return { calls, fetchEntity };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('createLatestLoader', () => {
  it('only lets the latest load succeed, even if an older one resolves later', async () => {
    const { calls, fetchEntity } = deferredFetch();
    const loading: boolean[] = [];
    const loader = createLatestLoader(fetchEntity, (value) => loading.push(value));
    const first = vi.fn();
    const second = vi.fn();

    loader.load('a', first, vi.fn());
    loader.load('b', second, vi.fn());
    expect(calls[0].signal.aborted).toBe(true);

    calls[1].resolve('B');
    await flush();
    calls[0].resolve('A');
    await flush();

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith('B');
    expect(loading).toEqual([true, true, false]);
  });

  it('drops failures of superseded loads', async () => {
    const { calls, fetchEntity } = deferredFetch();
    const loading: boolean[] = [];
    const loader = createLatestLoader(fetchEntity, (value) => loading.push(value));
    const firstError = vi.fn();
    const secondError = vi.fn();

    loader.load('a', vi.fn(), firstError);
    loader.load('b', vi.fn(), secondError);
    calls[0].reject(new Error('aborted'));
    await flush();

    expect(firstError).not.toHaveBeenCalled();
    // The obsolete failure must not clear the latest load's loading state.
    expect(loading).toEqual([true, true]);

    calls[1].reject(new Error('boom'));
    await flush();
    expect(secondError).toHaveBeenCalledWith(new Error('boom'));
    expect(loading).toEqual([true, true, false]);
  });

  it('cancel (used on unmount and on Create) aborts and silences the pending load', async () => {
    const { calls, fetchEntity } = deferredFetch();
    const loading: boolean[] = [];
    const loader = createLatestLoader(fetchEntity, (value) => loading.push(value));
    const onSuccess = vi.fn();
    const onError = vi.fn();

    loader.load('a', onSuccess, onError);
    loader.cancel();
    expect(calls[0].signal.aborted).toBe(true);
    expect(loading).toEqual([true, false]);

    calls[0].resolve('A');
    calls[0].reject(new Error('aborted'));
    await flush();
    expect(onSuccess).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();

    // Cancelling with nothing in progress is a no-op.
    loader.cancel();
    expect(loading).toEqual([true, false]);
  });

  it('reports synchronous fetch errors through onError', async () => {
    const loader = createLatestLoader<string>(() => {
      throw new Error('sync');
    }, vi.fn());
    const onError = vi.fn();

    loader.load('a', vi.fn(), onError);
    await flush();
    expect(onError).toHaveBeenCalledWith(new Error('sync'));
  });
});
