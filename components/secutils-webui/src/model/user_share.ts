export const USER_SHARE_ID_HEADER_NAME = 'x-user-share-id';

export function getUserShareId() {
  return new URLSearchParams(window.location.search).get(USER_SHARE_ID_HEADER_NAME);
}

export function removeUserShareId() {
  const searchParams = new URLSearchParams(window.location.search);
  if (searchParams.has(USER_SHARE_ID_HEADER_NAME)) {
    searchParams.delete(USER_SHARE_ID_HEADER_NAME);
    window.history.replaceState(
      null,
      '',
      searchParams.size > 0 ? `${window.location.pathname}?${searchParams.toString()}` : window.location.pathname,
    );
  }
}

/**
 * Describes a user share.
 */
export interface UserShare {
  id: string;
  resource: UserShareResource;
  createdAt: number;
}

/**
 * Describes a resource that can be shared with other users.
 */
export type UserShareResource =
  | {
      type: 'contentSecurityPolicy';
      policyId: string;
    }
  | {
      type: 'certificateTemplate';
      templateId: string;
    };
