import { EuiIcon } from '@elastic/eui';

import type { Responder } from './responder';
import { EntityName } from '../../components/entity_name';

export function ResponderName({ responder, href }: { responder: Responder; href: string }) {
  const icons = [];
  if (responder.settings.script) {
    icons.push(<EuiIcon key="script" type={'function'} size="s" title={'Responder generates responses dynamically'} />);
  }
  if (!responder.enabled) {
    icons.push(<EuiIcon key="offline" type={'offline'} size="s" title={'Responder is disabled'} />);
  }
  if (responder.settings.notifications) {
    icons.push(<EuiIcon key="notifications" type={'bell'} size="s" title={'Email notifications are enabled'} />);
  }

  return (
    <EntityName name={responder.name} href={href} disabled={!responder.enabled} icons={icons} tags={responder.tags} />
  );
}
