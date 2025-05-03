import type { ComponentType, LazyExoticComponent } from 'react';
import { lazy } from 'react';

export const UTIL_HANDLES = Object.freeze({
  home: 'home',
  webhooks: 'webhooks',
  webhooksResponders: 'webhooks__responders',
  certificates: 'certificates',
  certificatesCertificateTemplates: 'certificates__certificate_templates',
  certificatesPrivateKeys: 'certificates__private_keys',
  webSecurity: 'web_security',
  webSecurityCsp: 'web_security__csp',
  webSecurityCspPolicies: 'web_security__csp__policies',
  webScraping: 'web_scraping',
  webScrapingPage: 'web_scraping__page',
});

export const UtilsComponents = new Map<string, LazyExoticComponent<ComponentType>>([
  [UTIL_HANDLES.home, lazy(() => import('./home/home'))],
  [UTIL_HANDLES.webhooksResponders, lazy(() => import('./webhooks/responders'))],
  [
    UTIL_HANDLES.certificatesCertificateTemplates,
    lazy(() => import('./certificates/certificates_certificate_templates')),
  ],
  [UTIL_HANDLES.certificatesPrivateKeys, lazy(() => import('./certificates/certificates_private_keys'))],
  [UTIL_HANDLES.webSecurityCspPolicies, lazy(() => import('./web_security/csp/web_security_csp_policies'))],
  [UTIL_HANDLES.webScrapingPage, lazy(() => import('./web_scraping/page_trackers'))],
]);

// Dedicated set of component overrides for user shares.
export const UtilsShareComponents = new Map<string, LazyExoticComponent<ComponentType>>([
  [UTIL_HANDLES.certificatesCertificateTemplates, lazy(() => import('./certificates/shared_certificate_template'))],
  [UTIL_HANDLES.webSecurityCspPolicies, lazy(() => import('./web_security/csp/web_security_csp_shared_policy'))],
]);

export function getUtilIcon(utilHandle: string, purpose: 'navigation' | 'search' | 'share') {
  switch (utilHandle) {
    case UTIL_HANDLES.home:
      return 'home';
    case UTIL_HANDLES.webhooks:
      return 'node';
    case UTIL_HANDLES.webhooksResponders:
      return purpose === 'search' || purpose === 'share' ? 'node' : undefined;
    case UTIL_HANDLES.certificates:
      return 'securityApp';
    case UTIL_HANDLES.certificatesCertificateTemplates:
    case UTIL_HANDLES.certificatesPrivateKeys:
      return purpose === 'search' || purpose === 'share' ? 'securityApp' : undefined;
    case UTIL_HANDLES.webSecurity:
      return 'globe';
    case UTIL_HANDLES.webSecurityCsp:
    case UTIL_HANDLES.webSecurityCspPolicies:
      return purpose === 'search' || purpose === 'share' ? 'globe' : undefined;
    case UTIL_HANDLES.webScraping:
      return 'cut';
    case UTIL_HANDLES.webScrapingPage:
      return purpose === 'search' || purpose === 'share' ? 'cut' : undefined;
    default:
      return;
  }
}
