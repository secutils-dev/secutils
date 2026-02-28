import { EuiIcon, EuiText, useEuiTheme } from '@elastic/eui';

import type { ApiTracker } from './api_tracker';
import type { PageTracker } from './page_tracker';

type TrackerLike = PageTracker | ApiTracker;

export function TrackerName({ tracker }: { tracker: TrackerLike }) {
  const { euiTheme } = useEuiTheme();
  const isDisabled = tracker.retrack.enabled === false;
  const jobConfig = tracker.retrack.config?.job;

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

  if (icons.length === 0) {
    return <span style={{ whiteSpace: 'nowrap' }}>{tracker.name}</span>;
  }

  return (
    <EuiText size="s" color={isDisabled ? euiTheme.colors.textDisabled : undefined} style={{ whiteSpace: 'nowrap' }}>
      {tracker.name} {icons}
    </EuiText>
  );
}
