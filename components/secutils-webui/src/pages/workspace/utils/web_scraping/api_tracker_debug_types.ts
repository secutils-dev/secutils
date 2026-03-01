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

export type PipelineStage =
  | { kind: 'configurator'; data: ScriptDebugInfo }
  | { kind: 'request'; data: ApiRequestDebugInfo; requestIndex: number }
  | { kind: 'extractor'; data: ScriptDebugInfo }
  | { kind: 'result' };

export function buildPipelineStages(result: DebugResult): PipelineStage[] {
  const stages: PipelineStage[] = [];

  if (result.target.configurator) {
    stages.push({ kind: 'configurator', data: result.target.configurator });
  }

  for (let i = 0; i < result.target.requests.length; i++) {
    stages.push({ kind: 'request', data: result.target.requests[i], requestIndex: i });
  }

  if (result.target.extractor) {
    stages.push({ kind: 'extractor', data: result.target.extractor });
  }

  stages.push({ kind: 'result' });

  return stages;
}
