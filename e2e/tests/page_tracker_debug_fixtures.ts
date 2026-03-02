export interface PageLogEntry {
  level: string;
  message: string;
  args?: unknown[];
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
    durationMs: number;
    error?: string;
  };
}
