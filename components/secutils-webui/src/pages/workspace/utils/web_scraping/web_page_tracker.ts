export interface WebPageTracker<S = unknown> {
  id: string;
  name: string;
  url: string;
  createdAt: number;
  updatedAt: number;
  settings: {
    revisions: number;
    delay: number;
    schedule?: string;
    scripts?: S;
    headers?: Record<string, string>;
  };
  jobConfig?: SchedulerJobConfig;
}

export interface SchedulerJobConfig {
  schedule: string;
  retryStrategy?: SchedulerJobRetryStrategy;
  notifications: boolean;
}

export interface SchedulerJobRetryStrategy {
  type: 'constant';
  interval: number;
  maxAttempts: number;
}

export interface WebPageResourcesTracker
  extends WebPageTracker<{
    resourceFilterMap?: string;
  }> {}

export interface WebPageContentTracker
  extends WebPageTracker<{
    extractContent?: string;
  }> {}
