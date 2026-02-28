import type { SchedulerJobConfig, SecretsAccess } from './page_tracker';

export interface ApiTracker {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
  secrets?: SecretsAccess;
  retrack: {
    id: string;
    enabled?: boolean;
    target?: ApiTrackerTarget;
    config?: {
      revisions: number;
      job?: SchedulerJobConfig;
    };
    notifications?: boolean;
  };
}

export interface ApiTrackerTarget {
  url: string;
  method?: string;
  headers?: Record<string, string>;
  body?: unknown;
  mediaType?: string;
  acceptStatuses?: number[];
  acceptInvalidCertificates?: boolean;
  configurator?: string;
  extractor?: string;
}
