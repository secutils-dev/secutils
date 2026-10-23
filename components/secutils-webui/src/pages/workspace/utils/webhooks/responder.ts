import type { EntityTag } from '../../../../model';
import { apiFetch, ResponseError } from '../../../../model';

export interface SecretsAccess {
  type: 'none' | 'all' | 'selected';
  secrets?: string[];
}

export interface ResponderNotificationSettings {
  throttleSeconds: number;
}

export interface ResponderLocation {
  pathType: '=' | '^';
  path: string;
  subdomainPrefix?: string;
}

export interface Responder {
  id: string;
  name: string;
  location: ResponderLocation;
  method: string;
  enabled: boolean;
  settings: {
    requestsToTrack: number;
    statusCode: number;
    headers?: Array<[string, string]>;
    body?: string;
    script?: string;
    secrets?: SecretsAccess;
    notifications?: ResponderNotificationSettings;
  };
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}

/** Lightweight responder representation returned by the responders list endpoint (no body or script). */
export interface ResponderSummary {
  id: string;
  name: string;
  location: ResponderLocation;
  method: string;
  enabled: boolean;
  settings: Omit<Responder['settings'], 'body' | 'script'> & { hasScript: boolean };
  /** Serialized settings size in bytes, including body, headers, script, and other settings. */
  settingsSize: number;
  /** Stats computed from the currently retained request history. */
  stats: {
    requestCount: number;
    lastRequestedAt?: number;
  };
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}

/** Usage of the responder script key-value storage (`secutils.kv.*`), live entries only. */
export interface ResponderStorageStats {
  entries: number;
  totalBytes: number;
  oldestCreatedAt?: number;
  nextExpiresAt?: number;
  maxEntries: number;
  maxTotalBytes: number;
}

export async function getResponderStorage(id: string): Promise<ResponderStorageStats> {
  const res = await apiFetch(`/api/webhooks/responders/${encodeURIComponent(id)}/_storage`);
  if (!res.ok) {
    throw await ResponseError.fromResponse(res);
  }
  return (await res.json()) as ResponderStorageStats;
}

/** Removes all storage entries, or only those whose key starts with `prefix`. Returns the removed count. */
export async function clearResponderStorage(id: string, prefix?: string): Promise<number> {
  const res = await apiFetch(`/api/webhooks/responders/${encodeURIComponent(id)}/_clear_storage`, {
    method: 'POST',
    body: JSON.stringify(prefix ? { prefix } : {}),
  });
  if (!res.ok) {
    throw await ResponseError.fromResponse(res);
  }
  return ((await res.json()) as { removed: number }).removed;
}

export async function getResponder(id: string, signal?: AbortSignal): Promise<Responder> {
  const res = await apiFetch(`/api/webhooks/responders/${encodeURIComponent(id)}`, { signal });
  if (!res.ok) {
    throw await ResponseError.fromResponse(res);
  }
  return (await res.json()) as Responder;
}
