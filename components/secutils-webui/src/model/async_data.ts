export type AsyncData<TValue, TState = never> =
  | { status: 'pending'; state?: TState }
  | { status: 'failed'; error: string; state?: TState }
  | { status: 'succeeded'; data: TValue; state?: TState };
