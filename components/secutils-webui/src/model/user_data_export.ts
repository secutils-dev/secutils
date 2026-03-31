import { getApiRequestConfig, getApiUrl } from './urls';

export type ExportSelection = { type: 'all' } | { type: 'selected'; ids: string[] };

export type ExportTrackableSelection =
  | { type: 'all'; includeHistory: boolean }
  | { type: 'selected'; ids: string[]; includeHistory: boolean };

export interface ExportParams {
  include: {
    settings?: boolean;
    tags?: ExportSelection;
    scripts?: ExportSelection;
    secrets?: ExportSelection;
    responders?: ExportTrackableSelection;
    certificateTemplates?: ExportSelection;
    privateKeys?: ExportSelection;
    contentSecurityPolicies?: ExportSelection;
    pageTrackers?: ExportTrackableSelection;
    apiTrackers?: ExportTrackableSelection;
  };
  secretsPassphrase?: string;
}

export interface ImportPreviewParams {
  data: unknown;
  mode: 'merge' | 'apply';
}

export interface ImportConflict {
  sourceId: string;
  name: string;
  existingId: string;
  /** False when conflict is on location+method - only overwrite/skip are valid. Defaults to true if absent. */
  renameAllowed?: boolean;
}

export interface ImportEntitySummary {
  total: number;
  conflicts: ImportConflict[];
}

export interface ImportSettingsSummary {
  included: boolean;
  hasExisting: boolean;
}

export interface ApplyDeleteItem {
  id: string;
  name: string;
}

export interface ApplyDeleteSummary {
  scripts: ApplyDeleteItem[];
  secrets: ApplyDeleteItem[];
  responders: ApplyDeleteItem[];
  certificateTemplates: ApplyDeleteItem[];
  privateKeys: ApplyDeleteItem[];
  contentSecurityPolicies: ApplyDeleteItem[];
  pageTrackers: ApplyDeleteItem[];
  apiTrackers: ApplyDeleteItem[];
}

export interface ImportPreview {
  valid: boolean;
  version: number;
  summary: {
    settings: ImportSettingsSummary;
    tags: ImportEntitySummary;
    scripts: ImportEntitySummary;
    secrets: ImportEntitySummary;
    responders: ImportEntitySummary;
    certificateTemplates: ImportEntitySummary;
    privateKeys: ImportEntitySummary;
    contentSecurityPolicies: ImportEntitySummary;
    pageTrackers: ImportEntitySummary;
    apiTrackers: ImportEntitySummary;
  };
  warnings: string[];
  toDelete?: ApplyDeleteSummary;
}

export interface ImportEntitySelection {
  sourceId: string;
  action: 'import' | 'skip';
  conflictResolution?: 'rename' | 'overwrite' | 'skip';
}

export interface ApplyDeletionSelections {
  scripts: string[];
  secrets: string[];
  responders: string[];
  certificateTemplates: string[];
  privateKeys: string[];
  contentSecurityPolicies: string[];
  pageTrackers: string[];
  apiTrackers: string[];
}

export interface ImportParams {
  data: unknown;
  mode: 'merge' | 'apply';
  selections: {
    importSettings?: boolean;
    tags: ImportEntitySelection[];
    scripts: ImportEntitySelection[];
    secrets: ImportEntitySelection[];
    responders: ImportEntitySelection[];
    certificateTemplates: ImportEntitySelection[];
    privateKeys: ImportEntitySelection[];
    contentSecurityPolicies: ImportEntitySelection[];
    pageTrackers: ImportEntitySelection[];
    apiTrackers: ImportEntitySelection[];
  };
  secretsPassphrase?: string;
  applyDeletions?: ApplyDeletionSelections;
}

export interface ImportEntityResult {
  imported: number;
  updated: number;
  skipped: number;
  deleted: number;
  failed: number;
  errors: string[];
}

export interface ImportResult {
  results: {
    settings: ImportEntityResult;
    tags: ImportEntityResult;
    scripts: ImportEntityResult;
    secrets: ImportEntityResult;
    responders: ImportEntityResult;
    certificateTemplates: ImportEntityResult;
    privateKeys: ImportEntityResult;
    contentSecurityPolicies: ImportEntityResult;
    pageTrackers: ImportEntityResult;
    apiTrackers: ImportEntityResult;
  };
}

export interface NamedEntity {
  id: string;
  name: string;
}

async function fetchEntities(path: string): Promise<NamedEntity[]> {
  const response = await fetch(getApiUrl(path), getApiRequestConfig());
  if (!response.ok) {
    throw new Error(`Failed to fetch ${path}`);
  }
  return response.json();
}

export function getTags(): Promise<NamedEntity[]> {
  return fetchEntities('/api/user/tags');
}

export function getResponders(): Promise<NamedEntity[]> {
  return fetchEntities('/api/webhooks/responders');
}

export function getCertificateTemplates(): Promise<NamedEntity[]> {
  return fetchEntities('/api/certificates/templates');
}

export function getPrivateKeys(): Promise<NamedEntity[]> {
  return fetchEntities('/api/certificates/private_keys');
}

export function getContentSecurityPolicies(): Promise<NamedEntity[]> {
  return fetchEntities('/api/web_security/csp');
}

export function getPageTrackers(): Promise<NamedEntity[]> {
  return fetchEntities('/api/utils/web_scraping/page');
}

export function getApiTrackers(): Promise<NamedEntity[]> {
  return fetchEntities('/api/utils/web_scraping/api');
}

export async function exportUserData(params: ExportParams): Promise<Blob> {
  const response = await fetch(getApiUrl('/api/user/data/_export'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify(params),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to export data.');
  }
  return response.blob();
}

export async function previewImport(params: ImportPreviewParams): Promise<ImportPreview> {
  const response = await fetch(getApiUrl('/api/user/data/_import_preview'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify(params),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to preview import.');
  }
  return response.json();
}

export async function executeImport(params: ImportParams): Promise<ImportResult> {
  const response = await fetch(getApiUrl('/api/user/data/_import'), {
    ...getApiRequestConfig('POST'),
    body: JSON.stringify(params),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.message ?? 'Failed to import data.');
  }
  return response.json();
}
