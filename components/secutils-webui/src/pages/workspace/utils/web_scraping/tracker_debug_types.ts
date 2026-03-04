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

export interface PageLogEntry {
  level: string;
  message: string;
  args?: unknown[];
}

export interface PageScreenshotEntry {
  label: string;
  data: string;
  mimeType: string;
}

export interface ApiDebugTarget {
  type: 'api';
  params?: unknown;
  configurator?: ScriptDebugInfo;
  requests: ApiRequestDebugInfo[];
  extractor?: ScriptDebugInfo;
}

export interface PageDebugTarget {
  type: 'page';
  params?: unknown;
  engine?: { type: string };
  extractorSource: string;
  logs: PageLogEntry[];
  screenshots?: PageScreenshotEntry[];
  durationMs: number;
  error?: string;
}

export interface DebugResult {
  durationMs: number;
  result?: unknown;
  error?: string;
  target: ApiDebugTarget | PageDebugTarget;
}

export type PipelineStage =
  | { kind: 'configurator'; data: ScriptDebugInfo }
  | { kind: 'request'; data: ApiRequestDebugInfo; requestIndex: number }
  | { kind: 'extractor'; data: ScriptDebugInfo }
  | { kind: 'pageExtractor'; data: PageDebugTarget }
  | { kind: 'result' };

export function buildPipelineStages(result: DebugResult): PipelineStage[] {
  const stages: PipelineStage[] = [];

  if (result.target.type === 'api') {
    if (result.target.configurator) {
      stages.push({ kind: 'configurator', data: result.target.configurator });
    }

    for (let i = 0; i < result.target.requests.length; i++) {
      stages.push({ kind: 'request', data: result.target.requests[i], requestIndex: i });
    }

    if (result.target.extractor) {
      stages.push({ kind: 'extractor', data: result.target.extractor });
    }
  } else {
    stages.push({ kind: 'pageExtractor', data: result.target });
  }

  stages.push({ kind: 'result' });

  return stages;
}
