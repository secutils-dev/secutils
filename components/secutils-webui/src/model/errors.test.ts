import { ResponseError as OryResponseError } from '@ory/kratos-client-fetch';
import { describe, expect, it } from 'vitest';

import { getErrorMessage, getErrorStatus, isClientError, ResponseError } from './errors';

describe('getErrorMessage', () => {
  it('extracts message from an Error', () => {
    expect(getErrorMessage(new Error('something broke'))).toBe('something broke');
  });

  it('extracts message from a ResponseError', () => {
    expect(getErrorMessage(new ResponseError('not found', 404))).toBe('not found');
  });

  it('returns fallback for null', () => {
    expect(getErrorMessage(null)).toBe('Unknown error');
  });

  it('returns fallback for undefined', () => {
    expect(getErrorMessage(undefined)).toBe('Unknown error');
  });

  it('returns fallback for a string (no .message property)', () => {
    expect(getErrorMessage('plain string')).toBe('Unknown error');
  });

  it('returns fallback for an object without message', () => {
    expect(getErrorMessage({ code: 42 })).toBe('Unknown error');
  });
});

describe('ResponseError', () => {
  it('sets name, message, and status', () => {
    const err = new ResponseError('Bad request', 400);
    expect(err.name).toBe('ResponseError');
    expect(err.message).toBe('Bad request');
    expect(err.status).toBe(400);
    expect(err).toBeInstanceOf(Error);
    expect(err).toBeInstanceOf(ResponseError);
  });

  describe('fromResponse', () => {
    it('extracts message from JSON body', async () => {
      const res = new Response(JSON.stringify({ message: 'Validation failed' }), {
        status: 422,
        statusText: 'Unprocessable Entity',
        headers: { 'Content-Type': 'application/json' },
      });

      const err = await ResponseError.fromResponse(res);
      expect(err.message).toBe('Validation failed');
      expect(err.status).toBe(422);
    });

    it('falls back to status text when JSON parsing fails', async () => {
      const res = new Response('not json', {
        status: 500,
        statusText: 'Internal Server Error',
      });

      const err = await ResponseError.fromResponse(res);
      expect(err.message).toBe('500 Internal Server Error');
      expect(err.status).toBe(500);
    });

    it('falls back to "Unknown error" when JSON has no message and statusText is empty', async () => {
      const res = new Response(JSON.stringify({}), {
        status: 502,
        headers: { 'Content-Type': 'application/json' },
      });

      const err = await ResponseError.fromResponse(res);
      expect(err.message).toBe('502 ');
      expect(err.status).toBe(502);
    });
  });
});

describe('getErrorStatus', () => {
  it('returns status from ResponseError', () => {
    expect(getErrorStatus(new ResponseError('err', 403))).toBe(403);
  });

  it('returns status from OryResponseError', () => {
    const oryErr = new OryResponseError(new Response(null, { status: 401 }));
    expect(getErrorStatus(oryErr)).toBe(401);
  });

  it('returns undefined for a plain Error', () => {
    expect(getErrorStatus(new Error('plain'))).toBeUndefined();
  });

  it('returns undefined for non-error values', () => {
    expect(getErrorStatus(null)).toBeUndefined();
    expect(getErrorStatus('string')).toBeUndefined();
    expect(getErrorStatus(42)).toBeUndefined();
  });
});

describe('isClientError', () => {
  it('returns true for 4xx ResponseError', () => {
    expect(isClientError(new ResponseError('bad', 400))).toBe(true);
    expect(isClientError(new ResponseError('not found', 404))).toBe(true);
    expect(isClientError(new ResponseError('conflict', 409))).toBe(true);
    expect(isClientError(new ResponseError('too many', 429))).toBe(true);
  });

  it('returns false for 5xx ResponseError', () => {
    expect(isClientError(new ResponseError('server', 500))).toBe(false);
    expect(isClientError(new ResponseError('gateway', 502))).toBe(false);
  });

  it('returns true for 4xx OryResponseError', () => {
    const oryErr = new OryResponseError(new Response(null, { status: 422 }));
    expect(isClientError(oryErr)).toBe(true);
  });

  it('returns false for non-error values', () => {
    expect(isClientError(null)).toBe(false);
    expect(isClientError(new Error('plain'))).toBe(false);
    expect(isClientError('string')).toBe(false);
  });
});
