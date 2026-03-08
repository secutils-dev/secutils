export type TrackerExecutionLogStatus = 'success' | 'failure';

export interface TrackerExecutionLogPhase {
  phase: string;
  durationMs: number;
  status: TrackerExecutionLogStatus;
  meta?: Record<string, unknown>;
}

export interface TrackerExecutionLog {
  id: string;
  trackerId: string;
  startedAt: number;
  finishedAt: number;
  status: TrackerExecutionLogStatus;
  error?: string;
  isManual: boolean;
  retryAttempt?: number;
  maxRetryAttempts?: number;
  revisionSize?: number;
  hasChanges?: boolean;
  durationMs: number;
  phases?: TrackerExecutionLogPhase[];
}
