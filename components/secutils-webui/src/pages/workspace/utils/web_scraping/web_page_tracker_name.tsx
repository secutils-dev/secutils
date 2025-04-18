import { EuiIcon, EuiText } from '@elastic/eui';

import type { WebPageTracker } from './web_page_tracker';

export function WebPageTrackerName({ tracker }: { tracker: WebPageTracker }) {
  if (!tracker.jobConfig) {
    return tracker.name;
  }

  const timeIcon = <EuiIcon type={'timeRefresh'} size="s" title={'Scheduled checks are enabled'} />;
  return tracker.jobConfig.notifications ? (
    <EuiText size="s">
      {tracker.name} {timeIcon} <EuiIcon type={'bell'} size="s" title={'Notifications are enabled'} />
    </EuiText>
  ) : (
    <EuiText size="s">
      {tracker.name} {timeIcon}
    </EuiText>
  );
}
