import type { ServerStatus } from './server_status';
import type { User } from './user';
import type { UserSettings } from './user_settings';
import type { UserShare } from './user_share';
import type { Util } from './util';

/**
 * Licence-based properties.
 */
export interface License {
  /**
   * A maximum number of custom endpoints.
   */
  maxEndpoints: number;
}

export type WebhookUrlType = 'path' | 'subdomain';

/**
 * Defines subscription related properties returned as a part of the UI state.
 */
export interface SubscriptionState {
  /**
   * The subscription-dependent features available to the user.
   */
  features?: {
    certificates: { privateKeyAlgorithms?: string[] };
    webhooks: { responderRequests: number; responderCustomSubdomainPrefix: boolean };
    webScraping: { trackerRevisions: number; trackerSchedules?: string[] };
    webSecurity: { importPolicyFromUrl: boolean };
  };
  /**
   * The URL to the subscription management page.
   */
  manageUrl?: string;
  /**
   * The URL to the subscription feature overview page.
   */
  featureOverviewUrl?: string;
}

export interface UiState {
  synced: boolean;
  status: ServerStatus;
  license: License;
  user?: User;
  userShare?: UserShare;
  settings?: UserSettings;
  utils: Util[];
  webhookUrlType: WebhookUrlType;
  subscription?: SubscriptionState;
}
