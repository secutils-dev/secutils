import type { UserSubscription } from './user_subscription';

export interface User {
  email: string;
  handle: string;
  isActivated: boolean;
  isOperator?: boolean;
  subscription: UserSubscription;
}
