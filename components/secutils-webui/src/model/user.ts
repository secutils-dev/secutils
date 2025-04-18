import axios from 'axios';

import { getApiUrl } from './urls';
import type { UserSubscription } from './user_subscription';

export interface User {
  email: string;
  handle: string;
  isActivated: boolean;
  isOperator?: boolean;
  subscription: UserSubscription;
}

export async function getUserData<RType>(dataNamespace: string) {
  const response = await axios.get<{ [namespace: string]: unknown }>(
    getApiUrl(`/api/user/data?namespace=${dataNamespace}`),
  );
  return response.data[dataNamespace] as RType | null;
}

export async function setUserData<RType>(dataNamespace: string, dataValue: unknown) {
  const response = await axios.post<{ [namespace: string]: unknown }>(
    getApiUrl(`/api/user/data?namespace=${dataNamespace}`),
    { dataValue: JSON.stringify(dataValue) },
  );
  return response.data[dataNamespace] as RType | null;
}
