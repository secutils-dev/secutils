export interface DebugResult {
  durationMs: number;
  result?: unknown;
  error?: string;
  target: {
    type: 'api';
    params?: unknown;
    configurator?: ScriptDebugInfo;
    requests: ApiRequestDebugInfo[];
    extractor?: ScriptDebugInfo;
  };
}

export interface ScriptDebugInfo {
  durationMs: number;
  result?: unknown;
  error?: string;
}

export interface ApiRequestDebugInfo {
  index: number;
  source: string;
  url?: string;
  method?: string;
  requestHeaders?: Record<string, string>;
  requestBody?: unknown;
  statusCode?: number;
  responseHeaders?: Record<string, string>;
  responseBodyRaw?: string;
  responseBodyRawSize?: number;
  responseBodyParsed?: unknown;
  autoParse?: { mediaType: string; success: boolean; error?: string };
  durationMs: number;
  error?: string;
}
