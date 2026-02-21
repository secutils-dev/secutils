import { EuiIcon, EuiText, useEuiTheme } from '@elastic/eui';

import type { Responder } from './responder';

export function ResponderName({ responder }: { responder: Responder }) {
  const theme = useEuiTheme();
  if (!responder.settings.script && responder.enabled) {
    return <span style={{ whiteSpace: 'nowrap' }}>{responder.name}</span>;
  }

  const disabledIcon = <EuiIcon type={'offline'} size="s" title={'Responder is disabled'} />;
  if (responder.settings.script) {
    const scriptIcon = <EuiIcon type={'function'} size="s" title={'Responder generates responses dynamically'} />;
    return responder.enabled ? (
      <EuiText size="s" style={{ whiteSpace: 'nowrap' }}>
        {responder.name} {scriptIcon}
      </EuiText>
    ) : (
      <EuiText size="s" color={theme.euiTheme.colors.textDisabled} style={{ whiteSpace: 'nowrap' }}>
        {responder.name} {scriptIcon} {disabledIcon}
      </EuiText>
    );
  }

  return (
    <EuiText size="s" color={theme.euiTheme.colors.textDisabled} style={{ whiteSpace: 'nowrap' }}>
      {responder.name} {disabledIcon}
    </EuiText>
  );
}
