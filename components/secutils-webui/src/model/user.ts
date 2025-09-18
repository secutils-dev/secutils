import { ResponseError } from './errors';
import { getApiRequestConfig, getApiUrl } from './urls';
import type { UserSubscription } from './user_subscription';

export interface User {
  email: string;
  handle: string;
  isActivated: boolean;
  isOperator?: boolean;
  subscription: UserSubscription;
}

export async function getUserData<RType>(dataNamespace: string) {
  const response = await fetch(getApiUrl(`/api/user/data?namespace=${dataNamespace}`), getApiRequestConfig()).then(
    async (res) => {
      if (!res.ok) {
        throw await ResponseError.fromResponse(res);
      }
      return (await res.json()) as { [namespace: string]: unknown };
    },
  );
  return response[dataNamespace] as RType | null;
}

export async function setUserData<RType>(dataNamespace: string, dataValue: unknown) {
  const response = await fetch(getApiUrl(`/api/user/data?namespace=${dataNamespace}`), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify({ dataValue: JSON.stringify(dataValue) }),
  }).then(async (res) => {
    if (!res.ok) {
      throw await ResponseError.fromResponse(res);
    }
    return (await res.json()) as { [namespace: string]: unknown };
  });
  return response[dataNamespace] as RType | null;
}
