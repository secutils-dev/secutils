import { ResponseError } from './errors';
import { apiFetch } from './urls';

/**
 * Notification destination record returned by the API. Mirrors the Rust
 * `UserNotificationDestination` DTO. All timestamps are unix-seconds.
 *
 * The verified / unsubscribed / verification-pending booleans are not transmitted; clients
 * derive them from the underlying timestamps. See `isVerified`, `isUnsubscribed`, and
 * `isVerificationPending` helpers exported below.
 */
export interface UserNotificationDestination {
  id: string;
  kind: 'email';
  address: string;
  verifiedAt?: number;
  verificationExpiresAt?: number;
  verificationSentAt?: number;
  unsubscribedAt?: number;
  createdAt: number;
  updatedAt: number;
}

export function isVerified(record: UserNotificationDestination): boolean {
  return record.verifiedAt != null;
}

export function isUnsubscribed(record: UserNotificationDestination): boolean {
  return record.unsubscribedAt != null;
}

/**
 * True while a verification code is outstanding (issued, not yet entered, not expired).
 * Uses `Date.now()` as the local clock; the verification window is 15 minutes so a few
 * seconds of clock drift between client and server is harmless.
 */
export function isVerificationPending(record: UserNotificationDestination): boolean {
  if (record.verificationExpiresAt == null) {
    return false;
  }
  return record.verificationExpiresAt * 1000 > Date.now();
}

export async function getNotificationEmail(): Promise<UserNotificationDestination | null> {
  const response = await apiFetch('/api/user/notification_email');
  if (response.status === 404) {
    return null;
  }
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to fetch notification email.');
  }
  const json = (await response.json()) as UserNotificationDestination | null;
  return json ?? null;
}

export async function setNotificationEmail(email: string): Promise<UserNotificationDestination> {
  const response = await apiFetch('/api/user/notification_email', {
    method: 'PUT',
    body: JSON.stringify({ email }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to set notification email.');
  }
  return response.json();
}

export async function verifyNotificationEmail(code: string): Promise<UserNotificationDestination> {
  const response = await apiFetch('/api/user/notification_email/_verify', {
    method: 'POST',
    body: JSON.stringify({ code }),
  });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to verify notification email.');
  }
  return response.json();
}

export async function resendNotificationEmailCode(): Promise<void> {
  const response = await apiFetch('/api/user/notification_email/_resend', { method: 'POST' });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to resend verification code.');
  }
}

export async function clearNotificationEmail(): Promise<void> {
  const response = await apiFetch('/api/user/notification_email', { method: 'DELETE' });
  if (!response.ok) {
    throw await ResponseError.fromResponse(response, 'Failed to clear notification email.');
  }
}
