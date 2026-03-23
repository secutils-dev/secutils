import { EuiText, EuiToolTip, useEuiTheme } from '@elastic/eui';
import { css, keyframes } from '@emotion/react';
import moment from 'moment';
import { useMemo } from 'react';

import type { TrackerExecutionLog } from './tracker_execution_log';

const MAX_DOTS = 10;
const DOT_SIZE = 8;
const DOT_GAP = 3;

const pulse = keyframes`
  0%, 100% { opacity: 0.12; }
  50% { opacity: 0.28; }
`;

export function TrackerHealthDots({ logs }: { logs: TrackerExecutionLog[] | undefined }) {
  const { euiTheme } = useEuiTheme();

  const sortedLogs = useMemo(() => {
    if (!logs || logs.length === 0) {
      return [];
    }
    return [...logs].sort((a, b) => a.startedAt - b.startedAt).slice(-MAX_DOTS);
  }, [logs]);

  const containerStyle = css`
    display: inline-flex;
    flex-direction: column;
    gap: 2px;
    min-height: 32px;
    min-width: 80px;
    justify-content: center;
  `;

  const dotsRowStyle = css`
    display: inline-flex;
    gap: ${DOT_GAP}px;
    align-items: center;
  `;

  const placeholderDotStyle = css`
    width: ${DOT_SIZE}px;
    height: ${DOT_SIZE}px;
    border-radius: 50%;
    background: ${euiTheme.colors.mediumShade};
    animation: ${pulse} 1.5s ease-in-out infinite;
  `;

  const realDotStyle = (color: string) => css`
    width: ${DOT_SIZE}px;
    height: ${DOT_SIZE}px;
    border-radius: 50%;
    background: ${color};
    transition: opacity 150ms ease-in;
  `;

  // Loading state
  if (logs === undefined) {
    return (
      <div css={containerStyle}>
        <div css={dotsRowStyle}>
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} css={placeholderDotStyle} style={{ animationDelay: `${i * 100}ms` }} />
          ))}
        </div>
      </div>
    );
  }

  // No data
  if (sortedLogs.length === 0) {
    return (
      <div css={containerStyle}>
        <EuiText size="xs" color="subdued">
          —
        </EuiText>
      </div>
    );
  }

  const lastLog = sortedLogs[sortedLogs.length - 1];
  const lastRunLabel = moment.unix(lastLog.startedAt).fromNow();

  return (
    <div css={containerStyle}>
      <div css={dotsRowStyle}>
        {sortedLogs.map((log) => {
          const isSuccess = log.status === 'success';
          const color = isSuccess
            ? log.hasChanges === false
              ? euiTheme.colors.mediumShade
              : euiTheme.colors.success
            : euiTheme.colors.danger;
          const durationLabel =
            log.durationMs >= 1000 ? `${(log.durationMs / 1000).toFixed(1)}s` : `${Math.round(log.durationMs)}ms`;
          const statusLabel = !isSuccess
            ? 'Failed'
            : log.hasChanges === false
              ? 'No changes'
              : log.hasChanges === true
                ? 'Changed'
                : 'Success';
          const tooltipContent = `${moment.unix(log.startedAt).format('MMM D, HH:mm:ss')} · ${statusLabel} · ${durationLabel}`;
          return (
            <EuiToolTip key={log.id} content={tooltipContent} position="top">
              <div css={realDotStyle(color)} />
            </EuiToolTip>
          );
        })}
      </div>
      <EuiText
        size="xs"
        color="subdued"
        css={css`
          white-space: nowrap;
        `}
      >
        {lastRunLabel}
      </EuiText>
    </div>
  );
}
