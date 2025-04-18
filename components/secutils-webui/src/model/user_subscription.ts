/**
 * User subscription model.
 */
export interface UserSubscription {
  tier: 'basic' | 'standard' | 'professional' | 'ultimate';
  /**
   * Indicates since when the subscription is active.
   */
  startedAt: number;
  /**
   * Indicates when the subscription ends.
   */
  endsAt?: number;
  /**
   * Indicates when the trial started.
   */
  trialStartedAt?: number;
  /**
   * Indicates when the trial ends.
   */
  trialEndsAt?: number;
}
