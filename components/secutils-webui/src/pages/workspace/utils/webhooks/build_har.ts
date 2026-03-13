import type { ResponderRequest } from './responder_request';

interface HarHeader {
  name: string;
  value: string;
}

interface HarContent {
  size: number;
  mimeType: string;
  text?: string;
  encoding?: string;
}

interface HarPostData {
  mimeType: string;
  text: string;
  encoding: string;
}

interface HarRequest {
  method: string;
  url: string;
  httpVersion: string;
  cookies: unknown[];
  headers: HarHeader[];
  queryString: HarHeader[];
  headersSize: number;
  bodySize: number;
  postData?: HarPostData;
}

interface HarResponse {
  status: number;
  statusText: string;
  httpVersion: string;
  cookies: unknown[];
  headers: HarHeader[];
  content: HarContent;
  redirectURL: string;
  headersSize: number;
  bodySize: number;
}

interface HarEntry {
  startedDateTime: string;
  time: number;
  request: HarRequest;
  response: HarResponse;
  cache: Record<string, never>;
  timings: { send: number; wait: number; receive: number };
}

export interface HarLog {
  log: {
    version: string;
    creator: { name: string; version: string };
    pages: unknown[];
    entries: HarEntry[];
  };
}

function bytesToBase64(bytes: number[]): string {
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function headerValue(headerBytes: number[]): string {
  return new TextDecoder().decode(new Uint8Array(headerBytes));
}

function contentTypeFromHeaders(headers?: Array<[string, number[]]>): string {
  for (const [name, value] of headers ?? []) {
    if (name.toLowerCase() === 'content-type') {
      return headerValue(value);
    }
  }
  return 'application/octet-stream';
}

export function buildHar(requests: ResponderRequest[], responderUrl: string): HarLog {
  const entries: HarEntry[] = requests.map((req) => {
    const durationMs = req.durationMs ?? 0;

    const requestHeaders = (req.headers ?? []).map(([name, value]) => ({
      name,
      value: headerValue(value),
    }));

    const harRequest: HarRequest = {
      method: req.method,
      url: `${responderUrl}${req.url}`,
      httpVersion: 'HTTP/1.1',
      cookies: [],
      headers: requestHeaders,
      queryString: parseQueryString(req.url),
      headersSize: -1,
      bodySize: req.body?.length ?? 0,
    };

    if (req.body && req.body.length > 0) {
      harRequest.postData = {
        mimeType: contentTypeFromHeaders(req.headers),
        text: bytesToBase64(req.body),
        encoding: 'base64',
      };
    }

    let harResponse: HarResponse;
    if (req.responseStatusCode != null) {
      const responseHeaders = (req.responseHeaders ?? []).map(([name, value]) => ({
        name,
        value: headerValue(value),
      }));

      const content: HarContent = {
        size: req.responseBody?.length ?? 0,
        mimeType: contentTypeFromHeaders(req.responseHeaders),
      };
      if (req.responseBody && req.responseBody.length > 0) {
        content.text = bytesToBase64(req.responseBody);
        content.encoding = 'base64';
      }

      harResponse = {
        status: req.responseStatusCode,
        statusText: '',
        httpVersion: 'HTTP/1.1',
        cookies: [],
        headers: responseHeaders,
        content,
        redirectURL: '',
        headersSize: -1,
        bodySize: req.responseBody?.length ?? 0,
      };
    } else {
      harResponse = {
        status: 0,
        statusText: 'Not tracked',
        httpVersion: 'HTTP/1.1',
        cookies: [],
        headers: [],
        content: { size: 0, mimeType: 'application/octet-stream' },
        redirectURL: '',
        headersSize: -1,
        bodySize: 0,
      };
    }

    return {
      startedDateTime: new Date(req.createdAt * 1000).toISOString(),
      time: durationMs,
      request: harRequest,
      response: harResponse,
      cache: {},
      timings: {
        send: 0,
        wait: durationMs,
        receive: 0,
      },
    };
  });

  return {
    log: {
      version: '1.2',
      creator: { name: 'Secutils.dev', version: '1.0' },
      pages: [],
      entries,
    },
  };
}

function parseQueryString(url: string): Array<{ name: string; value: string }> {
  const qIndex = url.indexOf('?');
  if (qIndex === -1) {
    return [];
  }
  const params = new URLSearchParams(url.slice(qIndex + 1));
  const result: Array<{ name: string; value: string }> = [];
  params.forEach((value, name) => result.push({ name, value }));
  return result;
}
