import { EuiIcon, EuiLink, useEuiTheme } from '@elastic/eui';

import type { Responder } from './responder';

export function ResponderName({ responder, href }: { responder: Responder; href: string }) {
  const theme = useEuiTheme();
  const disabledColor = theme.euiTheme.colors.textDisabled;
  const textColor = theme.euiTheme.colors.text;

  const icons = [];
  if (responder.settings.script) {
    icons.push(<EuiIcon key="script" type={'function'} size="s" title={'Responder generates responses dynamically'} />);
  }
  if (!responder.enabled) {
    icons.push(<EuiIcon key="offline" type={'offline'} size="s" title={'Responder is disabled'} />);
  }

  return (
    <span style={{ whiteSpace: 'nowrap' }}>
      <EuiLink href={href} style={{ color: responder.enabled ? textColor : disabledColor }}>
        {responder.name}
      </EuiLink>
      {icons.length > 0 ? (
        <span style={{ display: 'inline-flex', gap: 4, marginLeft: 4, verticalAlign: 'middle' }}>{icons}</span>
      ) : null}
    </span>
  );
}
