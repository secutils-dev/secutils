import { describe, expect, it } from 'vitest';

import { buildHar } from './build_har';
import type { ResponderRequest } from './responder_request';

describe('buildHar', () => {
  it('builds valid HAR from request-only entries', () => {
    const requests: ResponderRequest[] = [
      {
        id: '1',
        method: 'GET',
        url: '/api/test?foo=bar',
        createdAt: 1700000000,
        durationMs: 42,
      },
    ];

    const har = buildHar(requests, 'https://example.com');
    expect(har.log.version).toBe('1.2');
    expect(har.log.creator.name).toBe('Secutils.dev');
    expect(har.log.entries).toHaveLength(1);

    const entry = har.log.entries[0];
    expect(entry.request.method).toBe('GET');
    expect(entry.request.url).toBe('https://example.com/api/test?foo=bar');
    expect(entry.request.queryString).toEqual([{ name: 'foo', value: 'bar' }]);
    expect(entry.time).toBe(42);
    expect(entry.timings).toEqual({ send: 0, wait: 42, receive: 0 });

    expect(entry.response.status).toBe(0);
    expect(entry.response.statusText).toBe('Not tracked');
  });

  it('includes response data when present', () => {
    const requests: ResponderRequest[] = [
      {
        id: '2',
        method: 'POST',
        url: '/api/data',
        createdAt: 1700000000,
        durationMs: 100,
        headers: [['content-type', Array.from(new TextEncoder().encode('application/json'))]],
        body: Array.from(new TextEncoder().encode('{"key":"value"}')),
        responseStatusCode: 201,
        responseHeaders: [['content-type', Array.from(new TextEncoder().encode('application/json'))]],
        responseBody: Array.from(new TextEncoder().encode('{"id":1}')),
      },
    ];

    const har = buildHar(requests, 'https://example.com');
    const entry = har.log.entries[0];

    expect(entry.response.status).toBe(201);
    expect(entry.response.headers).toEqual([{ name: 'content-type', value: 'application/json' }]);
    expect(entry.response.content.size).toBe(8);
    expect(entry.response.content.mimeType).toBe('application/json');
    expect(entry.response.content.encoding).toBeUndefined();
    expect(entry.response.content.text).toBe('{"id":1}');
  });

  it('base64-encodes binary request body', () => {
    const binaryBody = [0x00, 0x01, 0xff, 0xfe];
    const requests: ResponderRequest[] = [
      {
        id: '3',
        method: 'PUT',
        url: '/upload',
        createdAt: 1700000000,
        body: binaryBody,
        headers: [['content-type', Array.from(new TextEncoder().encode('application/octet-stream'))]],
      },
    ];

    const har = buildHar(requests, '');
    const entry = har.log.entries[0];

    expect(entry.request.postData).toBeDefined();
    expect(entry.request.postData!.encoding).toBe('base64');
    expect(entry.request.postData!.mimeType).toBe('application/octet-stream');
    const decoded = atob(entry.request.postData!.text);
    expect(Array.from(decoded).map((c) => c.charCodeAt(0))).toEqual(binaryBody);
  });

  it('handles empty request list', () => {
    const har = buildHar([], 'https://example.com');
    expect(har.log.version).toBe('1.2');
    expect(har.log.entries).toHaveLength(0);
  });

  it('formats startedDateTime correctly', () => {
    const requests: ResponderRequest[] = [
      {
        id: '4',
        method: 'GET',
        url: '/',
        createdAt: 1700000000,
      },
    ];

    const har = buildHar(requests, '');
    const entry = har.log.entries[0];
    expect(entry.startedDateTime).toBe(new Date(1700000000 * 1000).toISOString());
  });

  it('uses zero duration when durationMs is absent', () => {
    const requests: ResponderRequest[] = [
      {
        id: '5',
        method: 'GET',
        url: '/',
        createdAt: 1700000000,
      },
    ];

    const har = buildHar(requests, '');
    const entry = har.log.entries[0];
    expect(entry.time).toBe(0);
    expect(entry.timings.wait).toBe(0);
  });

  it('emits plain text for text/html response body', () => {
    const html = '<html><body>Hello</body></html>';
    const requests: ResponderRequest[] = [
      {
        id: '7',
        method: 'GET',
        url: '/',
        createdAt: 1700000000,
        responseStatusCode: 200,
        responseHeaders: [['content-type', Array.from(new TextEncoder().encode('text/html; charset=utf-8'))]],
        responseBody: Array.from(new TextEncoder().encode(html)),
      },
    ];

    const har = buildHar(requests, '');
    const content = har.log.entries[0].response.content;
    expect(content.text).toBe(html);
    expect(content.encoding).toBeUndefined();
  });

  it('base64-encodes binary response body', () => {
    const binaryBody = [0x00, 0x01, 0xff, 0xfe];
    const requests: ResponderRequest[] = [
      {
        id: '8',
        method: 'GET',
        url: '/',
        createdAt: 1700000000,
        responseStatusCode: 200,
        responseHeaders: [['content-type', Array.from(new TextEncoder().encode('application/octet-stream'))]],
        responseBody: binaryBody,
      },
    ];

    const har = buildHar(requests, '');
    const content = har.log.entries[0].response.content;
    expect(content.encoding).toBe('base64');
    const decoded = atob(content.text!);
    expect(Array.from(decoded).map((c) => c.charCodeAt(0))).toEqual(binaryBody);
  });

  it('emits plain text for JSON request postData', () => {
    const jsonBody = '{"key":"value"}';
    const requests: ResponderRequest[] = [
      {
        id: '9',
        method: 'POST',
        url: '/api',
        createdAt: 1700000000,
        headers: [['content-type', Array.from(new TextEncoder().encode('application/json'))]],
        body: Array.from(new TextEncoder().encode(jsonBody)),
      },
    ];

    const har = buildHar(requests, '');
    const postData = har.log.entries[0].request.postData!;
    expect(postData.text).toBe(jsonBody);
    expect(postData.encoding).toBe('');
  });

  it('omits postData when body is absent', () => {
    const requests: ResponderRequest[] = [
      {
        id: '6',
        method: 'GET',
        url: '/no-body',
        createdAt: 1700000000,
      },
    ];

    const har = buildHar(requests, '');
    const entry = har.log.entries[0];
    expect(entry.request.postData).toBeUndefined();
    expect(entry.request.bodySize).toBe(0);
  });
});
