import {
  EuiButton,
  EuiButtonEmpty,
  EuiButtonIcon,
  EuiContextMenuItem,
  EuiContextMenuPanel,
  EuiPopover,
  useEuiTheme,
} from '@elastic/eui';
import { css } from '@emotion/react';
import { useState } from 'react';

import { useAppContext } from './use_app_context';
import { getOryApi } from '../tools/ory';

export function usePageHeaderActions() {
  const { uiState } = useAppContext();
  const euiTheme = useEuiTheme();
  const { refreshUiState, addToast } = useAppContext();

  const [isAccountPopoverOpen, setIsAccountPopoverOpen] = useState<boolean>(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);

  const onToggleSettings = () => {
    // Refresh UI state every time settings are opened.
    if (!isSettingsOpen) {
      refreshUiState();
    }

    setIsAccountPopoverOpen(false);
    setIsSettingsOpen(!isSettingsOpen);
  };

  const actions = uiState.user
    ? [
        <EuiButtonIcon
          key="btn-docs"
          iconType={'documentation'}
          css={css`
            margin-right: ${euiTheme.euiTheme.size.xxs};
          `}
          iconSize="m"
          size="m"
          title={`Documentation`}
          aria-label={`Open documentation`}
          target={'_blank'}
          href={'/docs'}
        />,
        <EuiPopover
          key="btn-account"
          className="eui-fullWidth"
          button={
            <EuiButtonIcon
              aria-label={'Account menu'}
              size={'m'}
              display={'empty'}
              iconType="user"
              title={'Account'}
              onClick={() => setIsAccountPopoverOpen(!isAccountPopoverOpen)}
            />
          }
          isOpen={isAccountPopoverOpen}
          closePopover={() => setIsAccountPopoverOpen(false)}
          panelPaddingSize="none"
          anchorPosition="downLeft"
        >
          <EuiContextMenuPanel
            size="m"
            title={uiState.user ? uiState.user.email : null}
            items={[
              <EuiContextMenuItem key="settings" icon="gear" onClick={onToggleSettings}>
                Settings
              </EuiContextMenuItem>,
              <EuiContextMenuItem
                key="signout"
                icon="exit"
                onClick={() => {
                  setIsAccountPopoverOpen(false);

                  getOryApi()
                    .then(async (api) => {
                      const flow = await api.createBrowserLogoutFlow();
                      await api.updateLogoutFlow({ token: flow.data.logout_token });

                      window.location.replace('/signin');
                      setTimeout(() => window.location.reload(), 500);
                    })
                    .catch((err) => {
                      console.log(`Failed to sign out: ${err}`);
                      addToast({ id: 'signout-error', title: 'Failed to sign out' });
                    });
                }}
              >
                Sign out
              </EuiContextMenuItem>,
            ]}
          />
        </EuiPopover>,
      ]
    : [
        <EuiButtonEmpty
          key="btn-signin"
          href="/signin"
          size="s"
          css={css`
            margin-right: ${euiTheme.euiTheme.size.xxs};
          `}
        >
          Sign in
        </EuiButtonEmpty>,
        <EuiButton key="btn-get-started" href="/signup" fill size="s">
          Get started
        </EuiButton>,
      ];

  return { actions, isSettingsOpen, hideSettings: () => setIsSettingsOpen(false) };
}
