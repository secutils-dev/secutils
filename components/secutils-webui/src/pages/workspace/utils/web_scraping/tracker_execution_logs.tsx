import type { Criteria, EuiBasicTableColumn, EuiTableSortingType } from '@elastic/eui';
import {
  EuiBadge,
  EuiBasicTable,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiHealth,
  EuiIcon,
  EuiLoadingSpinner,
  EuiSpacer,
  EuiText,
  EuiToolTip,
} from '@elastic/eui';
import { css } from '@emotion/react';
import moment from 'moment';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type { ApiTracker } from './api_tracker';
import type { PageTracker } from './page_tracker';
import type { TrackerExecutionLog, TrackerExecutionLogPhase } from './tracker_execution_log';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../model';

function formatDurationMs(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60_000).toFixed(1)}m`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function PhaseDetails({ phases }: { phases: TrackerExecutionLogPhase[] }) {
  return (
    <EuiFlexGroup
      gutterSize="s"
      wrap
      responsive={false}
      css={css`
        padding: 8px 0;
      `}
    >
      {phases.map((phase, i) => {
        const isSuccess = phase.status === 'success';
        const durationLabel =
          phase.durationMs >= 1000 ? `${(phase.durationMs / 1000).toFixed(1)}s` : `${phase.durationMs}ms`;
        const metaEntries = phase.meta ? Object.entries(phase.meta) : [];

        return (
          <EuiFlexItem key={i} grow={false}>
            <EuiFlexGroup gutterSize="xs" alignItems="center" responsive={false}>
              {i > 0 && (
                <EuiFlexItem grow={false}>
                  <EuiIcon type="sortRight" size="s" color="subdued" />
                </EuiFlexItem>
              )}
              <EuiFlexItem grow={false}>
                <EuiToolTip
                  content={
                    metaEntries.length > 0
                      ? metaEntries.map(([k, v]) => `${k}: ${JSON.stringify(v)}`).join(', ')
                      : undefined
                  }
                >
                  <EuiBadge color={isSuccess ? 'success' : 'danger'} iconType={isSuccess ? 'check' : 'cross'}>
                    {phase.phase} ({durationLabel})
                  </EuiBadge>
                </EuiToolTip>
              </EuiFlexItem>
            </EuiFlexGroup>
          </EuiFlexItem>
        );
      })}
    </EuiFlexGroup>
  );
}

export interface TrackerExecutionLogsProps {
  kind: 'page' | 'api';
  tracker: PageTracker | ApiTracker;
}

export function TrackerExecutionLogs({ kind, tracker }: TrackerExecutionLogsProps) {
  const [logs, setLogs] = useState<AsyncData<TrackerExecutionLog[]>>({ status: 'pending' });
  const [sorting, setSorting] = useState<EuiTableSortingType<TrackerExecutionLog>>({
    sort: { field: 'startedAt', direction: 'desc' },
  });
  const [expandedRows, setExpandedRows] = useState<Record<string, ReactNode>>({});

  const loadLogs = useCallback(() => {
    setLogs({ status: 'pending' });
    fetch(getApiUrl(`/api/utils/web_scraping/${kind}/${encodeURIComponent(tracker.id)}/logs`), getApiRequestConfig())
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }
        setLogs({ status: 'succeeded', data: (await res.json()) as TrackerExecutionLog[] });
      })
      .catch((err: Error) => setLogs({ status: 'failed', error: getErrorMessage(err) }));
  }, [kind, tracker.id]);

  useEffect(() => {
    loadLogs();
  }, [loadLogs]);

  const toggleRow = useCallback((log: TrackerExecutionLog) => {
    setExpandedRows((prev) => {
      const next = { ...prev };
      if (next[log.id]) {
        delete next[log.id];
      } else if (log.phases && log.phases.length > 0) {
        next[log.id] = <PhaseDetails phases={log.phases} />;
      }
      return next;
    });
  }, []);

  const columns = useMemo<EuiBasicTableColumn<TrackerExecutionLog>[]>(
    () => [
      {
        field: 'status',
        name: 'Status',
        width: '80px',
        sortable: true,
        render: (status: string) => (
          <EuiHealth color={status === 'success' ? 'success' : 'danger'}>
            {status === 'success' ? 'OK' : 'Fail'}
          </EuiHealth>
        ),
      },
      {
        field: 'hasChanges',
        name: 'Changes',
        width: '90px',
        sortable: true,
        render: (hasChanges: boolean | undefined) =>
          hasChanges === true ? (
            <EuiBadge color="primary" iconType="indexEdit">
              Yes
            </EuiBadge>
          ) : hasChanges === false ? (
            <EuiText size="xs" color="subdued">
              No
            </EuiText>
          ) : (
            '—'
          ),
      },
      {
        field: 'startedAt',
        name: 'Started',
        sortable: true,
        width: '140px',
        render: (ts: number) => (
          <EuiToolTip content={moment.unix(ts).format('YYYY-MM-DD HH:mm:ss')}>
            <span>{moment.unix(ts).format('MMM D, HH:mm:ss')}</span>
          </EuiToolTip>
        ),
      },
      {
        name: 'Duration',
        field: 'durationMs',
        sortable: true,
        width: '90px',
        render: (ms: number) => formatDurationMs(ms),
      },
      {
        name: 'Type',
        field: 'isManual',
        sortable: true,
        width: '115px',
        render: (isManual: boolean) => (
          <EuiBadge color={isManual ? 'hollow' : 'default'} iconType={isManual ? 'user' : 'clock'}>
            {isManual ? 'Manual' : 'Scheduled'}
          </EuiBadge>
        ),
      },
      {
        name: 'Retry',
        width: '70px',
        render: (log: TrackerExecutionLog) =>
          log.retryAttempt != null && log.maxRetryAttempts != null
            ? `${log.retryAttempt} / ${log.maxRetryAttempts}`
            : '—',
      },
      {
        name: 'Rev. size',
        field: 'revisionSize',
        sortable: true,
        width: '90px',
        render: (size: number | undefined) => (size != null ? formatBytes(size) : '—'),
      },
      {
        name: 'Error',
        field: 'error',
        truncateText: true,
        render: (error: string | undefined) =>
          error ? (
            <EuiToolTip content={error}>
              <EuiText size="xs" color="danger">
                {error}
              </EuiText>
            </EuiToolTip>
          ) : (
            '—'
          ),
      },
      {
        name: '',
        width: '40px',
        isExpander: true,
        render: (log: TrackerExecutionLog) => {
          const hasPhases = log.phases && log.phases.length > 0;
          if (!hasPhases) return null;
          return (
            <EuiIcon
              type={expandedRows[log.id] ? 'arrowDown' : 'arrowRight'}
              onClick={() => toggleRow(log)}
              css={css`
                cursor: pointer;
              `}
              aria-label={expandedRows[log.id] ? 'Collapse phases' : 'Expand phases'}
            />
          );
        },
      },
    ],
    [expandedRows, toggleRow],
  );

  const onTableChange = useCallback(({ sort }: Criteria<TrackerExecutionLog>) => {
    if (sort) {
      setSorting({ sort: { field: sort.field, direction: sort.direction } });
    }
  }, []);

  const sortedItems = useMemo(() => {
    if (logs.status !== 'succeeded' || !sorting.sort) return logs.status === 'succeeded' ? logs.data : [];
    const { field, direction } = sorting.sort;
    const sorted = [...logs.data].sort((a, b) => {
      const aVal = a[field as keyof TrackerExecutionLog];
      const bVal = b[field as keyof TrackerExecutionLog];
      if (aVal == null && bVal == null) return 0;
      if (aVal == null) return 1;
      if (bVal == null) return -1;
      if (aVal < bVal) return -1;
      if (aVal > bVal) return 1;
      return 0;
    });
    return direction === 'desc' ? sorted.reverse() : sorted;
  }, [logs, sorting]);

  if (logs.status === 'pending') {
    return (
      <EuiFlexGroup
        justifyContent="center"
        alignItems="center"
        css={css`
          min-height: 120px;
        `}
      >
        <EuiFlexItem grow={false}>
          <EuiLoadingSpinner size="l" />
        </EuiFlexItem>
      </EuiFlexGroup>
    );
  }

  if (logs.status === 'failed') {
    return (
      <EuiText color="danger" size="s" textAlign="center">
        Failed to load execution logs: {logs.error}
      </EuiText>
    );
  }

  if (logs.data.length === 0) {
    return (
      <EuiEmptyPrompt
        iconType="editorComment"
        titleSize="xs"
        title={<h3>No execution logs yet</h3>}
        body={<p>Logs will appear after the tracker runs.</p>}
      />
    );
  }

  return (
    <>
      <EuiSpacer size="s" />
      <EuiBasicTable
        items={sortedItems}
        columns={columns}
        itemId="id"
        itemIdToExpandedRowMap={expandedRows}
        sorting={sorting}
        onChange={onTableChange}
      />
    </>
  );
}
