import { EuiIcon, EuiLink, useEuiTheme } from '@elastic/eui';

import type { ApiTracker } from './api_tracker';
import type { PageTracker } from './page_tracker';

type TrackerLike = PageTracker | ApiTracker;

export function TrackerName({ tracker, href }: { tracker: TrackerLike; href: string }) {
  const { euiTheme } = useEuiTheme();
  const isDisabled = tracker.retrack.enabled === false;
  const jobConfig = tracker.retrack.config?.job;
  const textColor = euiTheme.colors.text;

  const icons = [];
  if (jobConfig) {
    icons.push(<EuiIcon key="time" type={'timeRefresh'} size="s" title={'Scheduled checks are enabled'} />);
  }
  if (tracker.retrack.notifications) {
    icons.push(<EuiIcon key="bell" type={'bell'} size="s" title={'Notifications are enabled'} />);
  }
  if (isDisabled) {
    icons.push(<EuiIcon key="offline" type={'offline'} size="s" title={'Tracker is disabled'} />);
  }

  return (
    <span style={{ whiteSpace: 'nowrap' }}>
      <EuiLink href={href} style={{ color: isDisabled ? euiTheme.colors.textDisabled : textColor }}>
        {tracker.name}
      </EuiLink>
      {icons.length > 0 ? (
        <span style={{ display: 'inline-flex', gap: 4, marginLeft: 4, verticalAlign: 'middle' }}>{icons}</span>
      ) : null}
    </span>
  );
}
