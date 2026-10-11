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

/** Lightweight responder representation returned by the responders list endpoint. */
export interface ResponderSummary {
  id: string;
  name: string;
  location: ResponderLocation;
  method: string;
  enabled: boolean;
  settings: {
    requestsToTrack: number;
    hasScript: boolean;
    hasNotifications: boolean;
  };
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

export async function getResponder(id: string, signal?: AbortSignal): Promise<Responder> {
  const res = await apiFetch(`/api/webhooks/responders/${encodeURIComponent(id)}`, { signal });
  if (!res.ok) {
    throw await ResponseError.fromResponse(res);
  }
  return (await res.json()) as Responder;
}
