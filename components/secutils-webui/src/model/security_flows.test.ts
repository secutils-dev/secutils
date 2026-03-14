import { FetchError, ResponseError } from '@ory/kratos-client-fetch';
import { describe, expect, it } from 'vitest';

import { getCsrfToken, getSecurityErrorMessage } from './security_flows';

function makeFlow(nodes: unknown[], messages?: unknown[]) {
  return { ui: { nodes, messages } } as Parameters<typeof getCsrfToken>[0];
}

function makeCsrfNode(value: string) {
  return {
    attributes: { node_type: 'input', name: 'csrf_token', value },
    messages: [],
  };
}

describe('getCsrfToken', () => {
  it('returns the CSRF token when present', () => {
    const flow = makeFlow([
      { attributes: { node_type: 'input', name: 'method' }, messages: [] },
      makeCsrfNode('tok-abc'),
    ]);
    expect(getCsrfToken(flow)).toBe('tok-abc');
  });

  it('returns undefined when no CSRF node exists', () => {
    const flow = makeFlow([{ attributes: { node_type: 'input', name: 'method' }, messages: [] }]);
    expect(getCsrfToken(flow)).toBeUndefined();
  });

  it('returns undefined when nodes are empty', () => {
    const flow = makeFlow([]);
    expect(getCsrfToken(flow)).toBeUndefined();
  });
});

describe('getSecurityErrorMessage', () => {
  it('extracts message from a ResponseError (Ory)', async () => {
    const body = { ui: { nodes: [] }, error: { message: 'auth failed' } };
    const response = new Response(JSON.stringify(body), {
      status: 401,
      headers: { 'Content-Type': 'application/json' },
    });
    const oryErr = new ResponseError(response);
    const msg = await getSecurityErrorMessage(oryErr);
    expect(msg).toBeUndefined();
  });

  it('extracts message from a FetchError', async () => {
    const fetchErr = new FetchError(new Error('Network failure'), 'network error');
    const msg = await getSecurityErrorMessage(fetchErr);
    expect(msg).toBe('network error');
  });

  it('extracts message from a GenericError-like object', async () => {
    const genericErr = { reason: 'session expired', message: 'fallback' };
    const msg = await getSecurityErrorMessage(genericErr);
    expect(msg).toBe('session expired');
  });

  it('falls back to message when reason is empty in GenericError', async () => {
    const genericErr = { reason: '', message: 'fallback msg' };
    const msg = await getSecurityErrorMessage(genericErr);
    expect(msg).toBe('fallback msg');
  });

  it('extracts error message from UI node messages', async () => {
    const uiResponse = {
      ui: {
        nodes: [
          { attributes: { node_type: 'input', name: 'email' }, messages: [{ type: 'error', text: 'Invalid email' }] },
        ],
        messages: [],
      },
    };
    const msg = await getSecurityErrorMessage(uiResponse);
    expect(msg).toBe('Invalid email');
  });

  it('extracts error message from UI-level messages when no node errors', async () => {
    const uiResponse = {
      ui: {
        nodes: [{ attributes: { node_type: 'input', name: 'email' }, messages: [{ type: 'info', text: 'ok' }] }],
        messages: [{ type: 'error', text: 'Form error' }],
      },
    };
    const msg = await getSecurityErrorMessage(uiResponse);
    expect(msg).toBe('Form error');
  });

  it('returns undefined when UI response has no error messages', async () => {
    const uiResponse = {
      ui: {
        nodes: [{ attributes: { node_type: 'input', name: 'email' }, messages: [{ type: 'info', text: 'ok' }] }],
        messages: [{ type: 'info', text: 'All good' }],
      },
    };
    const msg = await getSecurityErrorMessage(uiResponse);
    expect(msg).toBeUndefined();
  });

  it('extracts message from a plain Error', async () => {
    const msg = await getSecurityErrorMessage(new Error('something wrong'));
    expect(msg).toBe('something wrong');
  });

  it('returns "Unknown error occurred" for non-error values', async () => {
    expect(await getSecurityErrorMessage(null)).toBe('Unknown error occurred');
    expect(await getSecurityErrorMessage(undefined)).toBe('Unknown error occurred');
    expect(await getSecurityErrorMessage(42)).toBe('Unknown error occurred');
  });
});
