export interface PageTracker {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
  retrack: {
    id: string;
    enabled?: boolean;
    target?: {
      extractor: string;
    };
    config?: {
      revisions: number;
      job?: SchedulerJobConfig;
    };
    notifications?: boolean;
  };
}

export interface SchedulerJobConfig {
  schedule: string;
  retryStrategy?: SchedulerJobRetryStrategy;
}

export interface SchedulerJobRetryStrategy {
  type: 'constant';
  interval: number;
  maxAttempts: number;
}

export function areSchedulerJobsEqual(jobA?: SchedulerJobConfig | null, jobB?: SchedulerJobConfig | null) {
  if (!jobA && !jobB) {
    return true;
  }

  if (!jobA || !jobB) {
    return false;
  }

  return jobA.schedule === jobB.schedule && JSON.stringify(jobA.retryStrategy) === JSON.stringify(jobB.retryStrategy);
}
