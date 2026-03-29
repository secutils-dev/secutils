import { EuiText, EuiToolTip, useEuiTheme } from '@elastic/eui';
import { now, unix } from 'moment/moment';

/**
 * The maximum difference in days between the current date and the timestamp for the timestamp to be displayed as a
 * relative timestamp.
 */
const MAX_DIFF_FOR_RELATIVE_TIMESTAMP_DAYS = -3;

/**
 * The threshold in minutes for a timestamp to be considered recent (used for highlighting).
 */
const RECENT_HIGHLIGHT_THRESHOLD_MINUTES = -60;

export interface Props {
  timestamp?: number | null;
  disabled?: boolean;
  color?: string;
  highlightRecent?: boolean;
}

export function TimestampTableCell({ timestamp, disabled, color, highlightRecent }: Props) {
  const { euiTheme } = useEuiTheme();

  if (timestamp == null) {
    return (
      <EuiText size="s" color={disabled ? euiTheme.colors.textDisabled : undefined}>
        —
      </EuiText>
    );
  }

  const unixTimestamp = unix(timestamp);
  const isRecent = highlightRecent && unixTimestamp.diff(now(), 'minutes') >= RECENT_HIGHLIGHT_THRESHOLD_MINUTES;

  // Detect if the text should be highlighted, unless fixed color is provided.
  const textColor = disabled ? euiTheme.colors.textDisabled : isRecent && !color ? 'danger' : color;

  const formattedTimestamp =
    isRecent || unixTimestamp.diff(now(), 'days') > MAX_DIFF_FOR_RELATIVE_TIMESTAMP_DAYS
      ? unixTimestamp.fromNow(false)
      : unixTimestamp.format('ll');

  return (
    <EuiToolTip content={unixTimestamp.format('ll HH:mm:ss')}>
      <EuiText size={'s'} color={textColor}>
        {isRecent ? <b>{formattedTimestamp}</b> : formattedTimestamp}
      </EuiText>
    </EuiToolTip>
  );
}
