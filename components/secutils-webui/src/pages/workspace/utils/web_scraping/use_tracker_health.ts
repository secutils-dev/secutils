import { useCallback, useEffect, useMemo, useState } from 'react';

import type { TrackerExecutionLog } from './tracker_execution_log';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';

export interface TrackerHealthData {
  data: AsyncData<Record<string, TrackerExecutionLog[]>>;
  refetch: () => void;
}

export function useTrackerHealth(kind: 'page' | 'api', trackerIds: string[] | undefined): TrackerHealthData {
  const [data, setData] = useState<AsyncData<Record<string, TrackerExecutionLog[]>>>({ status: 'pending' });
  const [fetchKey, setFetchKey] = useState(0);
  const trackerCount = useMemo(() => trackerIds?.length ?? 0, [trackerIds]);

  useEffect(() => {
    if (trackerCount === 0) {
      setData({ status: 'succeeded', data: {} });
      return;
    }

    fetch(getApiUrl(`/api/web_scraping/${kind}_trackers/_logs_summary`), getApiRequestConfig())
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }
        const json = (await res.json()) as Record<string, TrackerExecutionLog[]>;
        setData({ status: 'succeeded', data: json });
      })
      .catch((err: Error) => setData({ status: 'failed', error: getErrorMessage(err) }));
  }, [kind, trackerCount, fetchKey]);

  const refetch = useCallback(() => setFetchKey((k) => k + 1), []);

  return { data, refetch };
}
