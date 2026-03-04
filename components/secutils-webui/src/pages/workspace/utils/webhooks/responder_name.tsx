import { EuiIcon, EuiText, useEuiTheme } from '@elastic/eui';

import type { Responder } from './responder';

export function ResponderName({ responder }: { responder: Responder }) {
  const theme = useEuiTheme();

  const icons = [];
  if (responder.settings.script) {
    icons.push(<EuiIcon key="script" type={'function'} size="s" title={'Responder generates responses dynamically'} />);
  }
  if (!responder.enabled) {
    icons.push(<EuiIcon key="offline" type={'offline'} size="s" title={'Responder is disabled'} />);
  }

  if (icons.length === 0) {
    return <span style={{ whiteSpace: 'nowrap' }}>{responder.name}</span>;
  }

  return (
    <EuiText
      size="s"
      color={!responder.enabled ? theme.euiTheme.colors.textDisabled : undefined}
      style={{ whiteSpace: 'nowrap' }}
    >
      {responder.name} <span style={{ display: 'inline-flex', gap: 4, verticalAlign: 'middle' }}>{icons}</span>
    </EuiText>
  );
}
