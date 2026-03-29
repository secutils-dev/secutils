import { EuiIcon } from '@elastic/eui';

import type { ApiTracker } from './api_tracker';
import type { PageTracker } from './page_tracker';
import { EntityName } from '../../components/entity_name';

type TrackerLike = PageTracker | ApiTracker;

export function TrackerName({ tracker, href }: { tracker: TrackerLike; href: string }) {
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

  return <EntityName name={tracker.name} href={href} disabled={isDisabled} icons={icons} tags={tracker.tags} />;
}
