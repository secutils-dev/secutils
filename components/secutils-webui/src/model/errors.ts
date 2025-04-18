import type { AxiosError } from 'axios';
import { CanceledError } from 'axios';

export function isAbortError(err: unknown) {
  return err instanceof CanceledError || (err instanceof DOMException && err.name === 'AbortError');
}

export function getErrorMessage(err: unknown) {
  return (isApplicationError(err) ? err.response?.data.message : undefined) ?? (err as Error).message;
}

export function isClientError(err: unknown) {
  const status = getErrorStatus(err);
  return status ? status >= 400 && status < 500 : false;
}

export function getErrorStatus(err: unknown) {
  return (err as AxiosError).response?.status ?? (err as { status?: number }).status;
}

function isApplicationError(err: unknown): err is AxiosError<{ message: string }> {
  const forceCastedError = err as AxiosError<{ message: string }>;
  return forceCastedError.isAxiosError && !!forceCastedError.response?.data?.message;
}
