import { useCallback, useEffect, useState } from 'react';

import { getApiRequestConfig, getApiUrl } from '../../../../model';

interface ToolCount {
  webhooks: number | null;
  certificates: number | null;
  csp: number | null;
  webScraping: number | null;
}

export interface RecentItem {
  name: string;
  toolId: string;
  path: string;
  updatedAt: number;
}

export interface WorkspaceSummary {
  status: 'pending' | 'succeeded' | 'failed';
  counts: ToolCount;
  recentItems: RecentItem[];
}

const EMPTY_COUNTS: ToolCount = { webhooks: null, certificates: null, csp: null, webScraping: null };

interface RawItem {
  name: string;
  updatedAt: number;
}

async function fetchItems(path: string): Promise<RawItem[]> {
  const res = await fetch(getApiUrl(path), getApiRequestConfig());
  if (!res.ok) {
    throw new Error(`Failed to fetch ${path}`);
  }
  return (await res.json()) as RawItem[];
}

function toRecentItems(items: RawItem[], toolId: string, path: string): RecentItem[] {
  return items.map((item) => ({ name: item.name, toolId, path, updatedAt: item.updatedAt }));
}

const MAX_RECENT_ITEMS = 3;

export function useWorkspaceSummary(isAuthenticated: boolean): WorkspaceSummary {
  const [summary, setSummary] = useState<WorkspaceSummary>({
    status: 'pending',
    counts: EMPTY_COUNTS,
    recentItems: [],
  });

  const load = useCallback(() => {
    if (!isAuthenticated) {
      setSummary({ status: 'succeeded', counts: EMPTY_COUNTS, recentItems: [] });
      return;
    }

    setSummary({ status: 'pending', counts: EMPTY_COUNTS, recentItems: [] });

    Promise.all([
      fetchItems('/api/utils/webhooks/responders'),
      fetchItems('/api/utils/certificates/templates'),
      fetchItems('/api/utils/certificates/private_keys'),
      fetchItems('/api/utils/web_security/csp'),
      fetchItems('/api/utils/web_scraping/page'),
      fetchItems('/api/utils/web_scraping/api'),
    ])
      .then(([webhooks, certTemplates, privateKeys, csp, pageTrackers, apiTrackers]) => {
        const recentItems = [
          ...toRecentItems(webhooks, 'webhooks', '/ws/webhooks__responders'),
          ...toRecentItems(certTemplates, 'certificates', '/ws/certificates__certificate_templates'),
          ...toRecentItems(privateKeys, 'certificates', '/ws/certificates__private_keys'),
          ...toRecentItems(csp, 'csp', '/ws/web_security__csp__policies'),
          ...toRecentItems(pageTrackers, 'webScraping', '/ws/web_scraping__page'),
          ...toRecentItems(apiTrackers, 'webScraping', '/ws/web_scraping__api'),
        ]
          .sort((a, b) => b.updatedAt - a.updatedAt)
          .slice(0, MAX_RECENT_ITEMS);

        setSummary({
          status: 'succeeded',
          counts: {
            webhooks: webhooks.length,
            certificates: certTemplates.length + privateKeys.length,
            csp: csp.length,
            webScraping: pageTrackers.length + apiTrackers.length,
          },
          recentItems,
        });
      })
      .catch(() => {
        setSummary({ status: 'failed', counts: EMPTY_COUNTS, recentItems: [] });
      });
  }, [isAuthenticated]);

  useEffect(() => {
    load();
  }, [load]);

  return summary;
}
