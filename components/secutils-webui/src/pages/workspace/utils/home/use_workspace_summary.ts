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
  utilHandle: string;
  path: string;
  updatedAt: number;
}

export interface WorkspaceSummary {
  status: 'pending' | 'succeeded' | 'failed';
  counts: ToolCount;
  recentItems: RecentItem[];
}

const EMPTY_COUNTS: ToolCount = { webhooks: null, certificates: null, csp: null, webScraping: null };

interface ServerSummary {
  counts: {
    webhooks: number;
    certificates: number;
    csp: number;
    webScraping: number;
  };
  recentItems: {
    name: string;
    utilHandle: string;
    updatedAt: number;
  }[];
}

function getUtilPath(utilHandle: string): string {
  return utilHandle === 'home' ? '/ws' : `/ws/${utilHandle}`;
}

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

    fetch(getApiUrl('/api/ui/home/summary'), getApiRequestConfig())
      .then(async (res) => {
        if (!res.ok) {
          throw new Error(`Failed to fetch home summary: ${res.status}`);
        }
        return (await res.json()) as ServerSummary;
      })
      .then((data) => {
        const recentItems: RecentItem[] = data.recentItems.map((item) => {
          const utilHandle = item.utilHandle;
          return {
            name: item.name,
            utilHandle,
            path: getUtilPath(utilHandle),
            updatedAt: item.updatedAt,
          };
        });

        setSummary({
          status: 'succeeded',
          counts: data.counts,
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
