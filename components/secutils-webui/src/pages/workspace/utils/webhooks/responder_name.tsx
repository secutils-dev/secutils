import { EuiIcon } from '@elastic/eui';

import type { ResponderSummary } from './responder';
import { ScriptIcon } from '../../../../components/legacy_icons';
import { EntityName } from '../../components/entity_name';

export function ResponderName({ responder, href }: { responder: ResponderSummary; href: string }) {
  const icons = [];
  if (responder.settings.hasScript) {
    icons.push(<EuiIcon key="script" type={ScriptIcon} size="s" title={'Responder generates responses dynamically'} />);
  }
  if (!responder.enabled) {
    icons.push(<EuiIcon key="offline" type={'wifiSlash'} size="s" title={'Responder is disabled'} />);
  }
  if (responder.settings.hasNotifications) {
    icons.push(<EuiIcon key="notifications" type={'bell'} size="s" title={'Email notifications are enabled'} />);
  }

  return (
    <EntityName name={responder.name} href={href} disabled={!responder.enabled} icons={icons} tags={responder.tags} />
  );
}
