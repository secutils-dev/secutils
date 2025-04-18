import { EuiIcon, EuiText, useEuiTheme } from '@elastic/eui';

import type { Responder } from './responder';

export function ResponderName({ responder }: { responder: Responder }) {
  const theme = useEuiTheme();
  if (!responder.settings.script && responder.enabled) {
    return responder.name;
  }

  const disabledIcon = <EuiIcon type={'offline'} size="s" title={'Responder is disabled'} />;
  if (responder.settings.script) {
    const scriptIcon = <EuiIcon type={'function'} size="s" title={'Responder generates responses dynamically'} />;
    return responder.enabled ? (
      <EuiText size="s">
        {responder.name} {scriptIcon}
      </EuiText>
    ) : (
      <EuiText size="s" color={theme.euiTheme.colors.disabledText}>
        {responder.name} {scriptIcon} {disabledIcon}
      </EuiText>
    );
  }

  return (
    <EuiText size="s" color={theme.euiTheme.colors.disabledText}>
      {responder.name} {disabledIcon}
    </EuiText>
  );
}
