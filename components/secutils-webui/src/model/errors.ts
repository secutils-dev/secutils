import type { OryError } from '../tools/ory';

export function isAbortError(err: unknown) {
  return (
    (err instanceof DOMException || isOryError(err)) && (err.name === 'AbortError' || err.name === 'CanceledError')
  );
}

export function getErrorMessage(err: unknown) {
  return (isOryError(err) ? err.response?.data?.message : undefined) ?? (err as Error).message ?? 'Unknown error';
}

export function isClientError(err: unknown) {
  const status = getErrorStatus(err);
  return status ? status >= 400 && status < 500 : false;
}

export function getErrorStatus(err: unknown) {
  if (err instanceof ResponseError) {
    return err.status;
  }

  if (isOryError(err)) {
    return err.response?.status;
  }
}

function isOryError(err: unknown): err is OryError {
  const forceCastedError = err as OryError;
  return forceCastedError.isAxiosError && !!forceCastedError.response?.data?.message;
}

export class ResponseError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);

    // Maintains a proper stack trace for where our error was thrown.
    if ('captureStackTrace' in Error && typeof Error.captureStackTrace === 'function') {
      Error.captureStackTrace(this, ResponseError);
    }

    this.name = 'ResponseError';
    this.status = status;
  }

  static async fromResponse(res: Response) {
    let message: string | undefined;
    try {
      message = (await res.json()).message;
    } catch {
      //
    }
    return new ResponseError(message ?? `${res.status} ${res.statusText ?? 'Unknown error'}`, res.status);
  }
}
