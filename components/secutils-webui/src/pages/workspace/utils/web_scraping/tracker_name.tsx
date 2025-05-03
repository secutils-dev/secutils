import { EuiIcon, EuiText } from '@elastic/eui';

import type { PageTracker } from './page_tracker';

export function TrackerName({ tracker }: { tracker: PageTracker }) {
  const jobConfig = tracker.retrack.config?.job;
  if (!jobConfig) {
    return tracker.name;
  }

  const timeIcon = <EuiIcon type={'timeRefresh'} size="s" title={'Scheduled checks are enabled'} />;
  return tracker.retrack.notifications ? (
    <EuiText size="s">
      {tracker.name} {timeIcon} <EuiIcon type={'bell'} size="s" title={'Notifications are enabled'} />
    </EuiText>
  ) : (
    <EuiText size="s">
      {tracker.name} {timeIcon}
    </EuiText>
  );
}
