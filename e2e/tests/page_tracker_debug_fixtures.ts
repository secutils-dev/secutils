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

export interface PageDebugResult {
  durationMs: number;
  result?: unknown;
  error?: string;
  target: {
    type: 'page';
    params?: unknown;
    engine?: string;
    extractorSource: string;
    logs: PageLogEntry[];
    screenshots?: PageScreenshotEntry[];
    durationMs: number;
    error?: string;
  };
}
