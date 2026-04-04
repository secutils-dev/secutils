import type { ComponentType, LazyExoticComponent } from 'react';
import { lazy } from 'react';

export const UTIL_HANDLES = Object.freeze({
  workspace: 'workspace',
  workspaceOverview: 'workspace__overview',
  workspaceTags: 'workspace__tags',
  workspaceSecrets: 'workspace__secrets',
  workspaceScripts: 'workspace__scripts',
  webhooks: 'webhooks',
  webhooksResponders: 'webhooks__responders',
  certificates: 'certificates',
  certificatesCertificateTemplates: 'certificates__certificate_templates',
  certificatesPrivateKeys: 'certificates__private_keys',
  webSecurity: 'web_security',
  webSecurityCsp: 'web_security__csp',
  webScraping: 'web_scraping',
  webScrapingPage: 'web_scraping__page',
  webScrapingApi: 'web_scraping__api',
});

export const UtilsComponents = new Map<string, LazyExoticComponent<ComponentType>>([
  [UTIL_HANDLES.workspaceOverview, lazy(() => import('./home/home'))],
  [UTIL_HANDLES.workspaceTags, lazy(() => import('./workspace/workspace_tags'))],
  [UTIL_HANDLES.workspaceSecrets, lazy(() => import('./workspace/workspace_secrets'))],
  [UTIL_HANDLES.workspaceScripts, lazy(() => import('./workspace/workspace_scripts'))],
  [UTIL_HANDLES.webhooksResponders, lazy(() => import('./webhooks/responders'))],
  [
    UTIL_HANDLES.certificatesCertificateTemplates,
    lazy(() => import('./certificates/certificates_certificate_templates')),
  ],
  [UTIL_HANDLES.certificatesPrivateKeys, lazy(() => import('./certificates/certificates_private_keys'))],
  [UTIL_HANDLES.webSecurityCsp, lazy(() => import('./web_security/csp/web_security_csp_policies'))],
  [UTIL_HANDLES.webScrapingPage, lazy(() => import('./web_scraping/page_trackers'))],
  [UTIL_HANDLES.webScrapingApi, lazy(() => import('./web_scraping/api_trackers'))],
]);

// Dedicated set of component overrides for user shares.
export const UtilsShareComponents = new Map<string, LazyExoticComponent<ComponentType>>([
  [UTIL_HANDLES.certificatesCertificateTemplates, lazy(() => import('./certificates/shared_certificate_template'))],
  [UTIL_HANDLES.webSecurityCsp, lazy(() => import('./web_security/csp/web_security_csp_shared_policy'))],
]);

export function getUtilIcon(utilHandle: string) {
  switch (utilHandle) {
    case UTIL_HANDLES.workspaceOverview:
      return 'spaces';
    case UTIL_HANDLES.workspaceTags:
      return 'tag';
    case UTIL_HANDLES.workspaceSecrets:
      return 'lock';
    case UTIL_HANDLES.workspaceScripts:
      return 'editorCodeBlock';
    case UTIL_HANDLES.webhooksResponders:
      return 'node';
    case UTIL_HANDLES.certificatesCertificateTemplates:
      return 'document';
    case UTIL_HANDLES.certificatesPrivateKeys:
      return 'key';
    case UTIL_HANDLES.webSecurityCsp:
      return 'documents';
    case UTIL_HANDLES.webScrapingPage:
      return 'article';
    case UTIL_HANDLES.webScrapingApi:
      return 'nested';
    default:
      return;
  }
}
