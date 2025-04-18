import { now, unix } from 'moment/moment';
import { EuiText, EuiToolTip } from '@elastic/eui';

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
  timestamp: number;
  color?: string;
  highlightRecent?: boolean;
}

export function TimestampTableCell({ timestamp, color, highlightRecent }: Props) {
  const unixTimestamp = unix(timestamp);
  const isRecent = highlightRecent && unixTimestamp.diff(now(), 'minutes') >= RECENT_HIGHLIGHT_THRESHOLD_MINUTES;

  // Detect if the text should be highlighted, unless fixed color is provided.
  let textColor = color;
  if (isRecent && !textColor) {
    textColor = 'danger';
  }

  const formattedTimestamp =
    isRecent || unixTimestamp.diff(now(), 'days') > MAX_DIFF_FOR_RELATIVE_TIMESTAMP_DAYS
      ? unixTimestamp.fromNow(false)
      : unixTimestamp.format('LL');

  return (
    <EuiToolTip content={unixTimestamp.format('ll HH:mm:ss')}>
      <EuiText size={'s'} color={textColor}>
        {isRecent ? <b>{formattedTimestamp}</b> : formattedTimestamp}
      </EuiText>
    </EuiToolTip>
  );
}
