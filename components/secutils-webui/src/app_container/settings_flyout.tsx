import { lazy, useCallback, useState } from 'react';
import type { ChangeEvent } from 'react';

import type { EuiThemeColorMode } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiDescribedFormGroup,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFlyout,
  EuiFlyoutBody,
  EuiFlyoutHeader,
  EuiFormRow,
  EuiLink,
  EuiSelect,
  EuiSpacer,
  EuiTab,
  EuiTabs,
  EuiText,
  EuiTitle,
} from '@elastic/eui';
import { css } from '@emotion/react';
import type { UiNodeInputAttributes } from '@ory/client';
import type { AxiosError } from 'axios';
import { unix, utc } from 'moment/moment';

import { useAppContext } from '../hooks';
import type { AsyncData, SerializedPublicKeyCredentialCreationOptions } from '../model';
import { getCsrfToken, getSecurityErrorMessage, isClientError, USER_SETTINGS_KEY_COMMON_UI_THEME } from '../model';
import { signupWithPasskey } from '../model/webauthn';
import { getOryApi } from '../tools/ory';
import { isWebAuthnSupported } from '../tools/webauthn';

const ConfirmAccessModal = lazy(() => import('../pages/signin/confirm_access_modal'));

export interface Props {
  onClose: () => void;
}

export function SettingsFlyout({ onClose }: Props) {
  const { settings, setSettings, uiState, refreshUiState, addToast } = useAppContext();

  const uiTheme = settings?.[USER_SETTINGS_KEY_COMMON_UI_THEME] as EuiThemeColorMode | undefined;
  const onThemeChange = useCallback(
    (e: ChangeEvent<HTMLSelectElement>) => {
      setSettings({ [USER_SETTINGS_KEY_COMMON_UI_THEME]: e.target.value });
    },
    [settings],
  );

  const [isPasskeySupported] = useState<boolean>(isWebAuthnSupported());

  const [password, setPassword] = useState<string>('');
  const onPasswordChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setPassword(e.target.value);
  }, []);

  const [repeatPassword, setRepeatPassword] = useState<string>('');
  const onRepeatPasswordChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setRepeatPassword(e.target.value);
  }, []);

  const [isReauthenticateModalVisible, setIsReauthenticateModalVisible] = useState<
    { visible: false } | { visible: true; action: () => Promise<void> }
  >({ visible: false });

  const [setPasswordStatus, setSetPasswordStatus] = useState<AsyncData<null> | null>(null);
  const onSetPassword = useCallback(() => {
    if (setPasswordStatus?.status === 'pending' || password !== repeatPassword) {
      return;
    }

    setSetPasswordStatus({ status: 'pending' });

    getOryApi()
      .then(async (api) => {
        const updateState = () => {
          setSetPasswordStatus({ status: 'succeeded', data: null });
          setPassword('');
          setRepeatPassword('');

          addToast({ id: 'set-password', color: 'success', title: 'Password has been set' });

          refreshUiState();
        };

        const { data: flow } = await api.createBrowserSettingsFlow();
        try {
          await api.updateSettingsFlow({
            flow: flow.id,
            updateSettingsFlowBody: { method: 'password' as const, password, csrf_token: getCsrfToken(flow) },
          });
        } catch (err) {
          if ((err as AxiosError).response?.status !== 403) {
            throw err;
          }

          setSetPasswordStatus({ status: 'failed', error: 'Access confirmation required' });
          setIsReauthenticateModalVisible({
            visible: true,
            action: async () => {
              const { data: updatedFlow } = await api.getSettingsFlow({ id: flow.id });
              await api.updateSettingsFlow({
                flow: flow.id,
                updateSettingsFlowBody: {
                  method: 'password' as const,
                  password,
                  csrf_token: getCsrfToken(updatedFlow),
                },
              });

              updateState();
            },
          });
          return;
        }

        updateState();
      })
      .catch((err: Error) => {
        const originalErrorMessage = getSecurityErrorMessage(err);
        setSetPasswordStatus({ status: 'failed', error: originalErrorMessage ?? 'Unknown error' });

        addToast({
          id: 'set-password-error',
          color: 'danger',
          title: 'Failed to set password',
          text: (
            <>
              {isClientError(err) && originalErrorMessage
                ? originalErrorMessage
                : 'Unable to set password, please try again later.'}
            </>
          ),
        });
      });
  }, [password, repeatPassword, refreshUiState]);

  const [setPasskeyStatus, setSetPasskeyStatus] = useState<AsyncData<null> | null>(null);
  const onSetPasskey = useCallback(() => {
    if (setPasskeyStatus?.status === 'pending') {
      return;
    }

    setSetPasskeyStatus({ status: 'pending' });

    getOryApi()
      .then(async (api) => {
        const updateState = () => {
          setSetPasskeyStatus({ status: 'succeeded', data: null });
          addToast({ id: 'set-passkey', color: 'success', title: 'Passkey has been set' });
          refreshUiState();
        };

        const { data: flow } = await api.createBrowserSettingsFlow();
        const publicKeyNode = flow?.ui?.nodes?.find(
          (node) => node.attributes.node_type === 'input' && node.attributes.name === 'webauthn_register_trigger',
        );
        if (!publicKeyNode) {
          throw new Error('Cannot set passkey.');
        }

        const { publicKey } = JSON.parse((publicKeyNode.attributes as UiNodeInputAttributes).value as string) as {
          publicKey: SerializedPublicKeyCredentialCreationOptions;
        };

        try {
          await api.updateSettingsFlow({
            flow: flow.id,
            updateSettingsFlowBody: {
              method: 'webauthn' as const,
              csrf_token: getCsrfToken(flow),
              webauthn_register: await signupWithPasskey(publicKey),
              webauthn_register_displayname: uiState.user!.email,
            },
          });
        } catch (err) {
          if ((err as AxiosError).response?.status !== 403) {
            throw err;
          }

          setSetPasskeyStatus({ status: 'failed', error: 'Access confirmation required' });
          setIsReauthenticateModalVisible({
            visible: true,
            action: async () => {
              const { data: updatedFlow } = await api.getSettingsFlow({ id: flow.id });
              await api.updateSettingsFlow({
                flow: flow.id,
                updateSettingsFlowBody: {
                  method: 'webauthn' as const,
                  csrf_token: getCsrfToken(updatedFlow),
                  webauthn_register: await signupWithPasskey(publicKey),
                  webauthn_register_displayname: uiState.user!.email,
                },
              });

              updateState();
            },
          });
          return;
        }

        updateState();
      })
      .catch((err: Error) => {
        const originalErrorMessage = getSecurityErrorMessage(err);
        setSetPasskeyStatus({ status: 'failed', error: originalErrorMessage ?? 'Unknown error' });

        addToast({
          id: 'set-passkey-error',
          color: 'danger',
          title: 'Failed to set passkey',
          text: (
            <>
              {isClientError(err) && originalErrorMessage
                ? originalErrorMessage
                : 'Unable to set passkey, please try again later.'}
            </>
          ),
        });
      });
  }, [uiState, refreshUiState]);

  const changeInProgress = setPasswordStatus?.status === 'pending' || setPasskeyStatus?.status === 'pending';
  const passkeySection = isPasskeySupported ? (
    <EuiFormRow fullWidth>
      {
        <EuiButton
          fullWidth
          disabled={changeInProgress}
          onClick={onSetPasskey}
          isLoading={setPasskeyStatus?.status === 'pending'}
        >
          Set passkey
        </EuiButton>
      }
    </EuiFormRow>
  ) : null;

  const [selectedTab, setSelectedTab] = useState<'general' | 'security' | 'account'>('general');
  let selectedTabContent;
  if (selectedTab === 'general') {
    selectedTabContent = (
      <EuiDescribedFormGroup title={<h3>Appearance</h3>} description={'Customize Secutils.dev appearance'}>
        <EuiFormRow label="Theme" fullWidth>
          <EuiSelect
            options={[
              { value: 'light', text: 'Light' },
              { value: 'dark', text: 'Dark' },
            ]}
            value={uiTheme ?? 'light'}
            onChange={onThemeChange}
          />
        </EuiFormRow>
      </EuiDescribedFormGroup>
    );
  } else if (selectedTab === 'security') {
    selectedTabContent = (
      <EuiDescribedFormGroup title={<h3>Credentials</h3>} description={'Configure your Secutils.dev credentials'}>
        <EuiFormRow fullWidth isDisabled={changeInProgress}>
          <EuiFieldText
            placeholder="New password"
            type={'password'}
            autoComplete="new-password"
            onChange={onPasswordChange}
            minLength={8}
            value={password}
          />
        </EuiFormRow>
        <EuiFormRow fullWidth isDisabled={changeInProgress}>
          <EuiFieldText
            placeholder="Repeat new password"
            type={'password'}
            autoComplete="new-password"
            onChange={onRepeatPasswordChange}
            minLength={8}
            isInvalid={repeatPassword !== password}
            value={repeatPassword}
          />
        </EuiFormRow>
        <EuiFormRow fullWidth>
          <EuiFlexGroup justifyContent={'spaceBetween'} wrap>
            <EuiFlexItem>
              <EuiButton
                disabled={password !== repeatPassword || password.length < 8 || changeInProgress}
                isLoading={setPasswordStatus?.status === 'pending'}
                onClick={onSetPassword}
              >
                Set password
              </EuiButton>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiFormRow>
        {passkeySection}
      </EuiDescribedFormGroup>
    );
  } else {
    const subscription = uiState.user?.subscription;
    let trialSection = null;
    if (subscription?.trialStartedAt && (subscription?.tier === 'basic' || subscription?.tier === 'standard')) {
      let text;
      if (subscription?.trialEndsAt !== undefined) {
        const nowUtc = utc();
        const trialEndsAtUtc = unix(subscription?.trialEndsAt);
        text = trialEndsAtUtc.isSameOrBefore(nowUtc) ? (
          <EuiText size={'s'} color={'danger'}>
            <b>Expired</b>
          </EuiText>
        ) : (
          <EuiText size={'s'} color={'success'}>
            <b>Active</b> (
            {(trialEndsAtUtc.diff(nowUtc, 'days') || 1).toLocaleString('en-GB', {
              style: 'unit',
              unit: 'day',
              unitDisplay: 'long',
            })}{' '}
            left)
          </EuiText>
        );
      }
      trialSection = (
        <EuiFormRow label="Trial" helpText={'Trial gives you access to all available Secutils.dev features.'} fullWidth>
          {text ?? <EuiText size={'s'}>-</EuiText>}
        </EuiFormRow>
      );
    }

    const subscriptionDescription = <span>View and manage your current Secutils.dev subscription</span>;
    selectedTabContent = (
      <>
        <EuiDescribedFormGroup
          title={<h3>Account</h3>}
          description={
            <>
              <span>Manage your Secutils.dev account</span>
              <br />
              <br />
              {uiState.user?.isActivated ? null : (
                <>
                  <EuiButtonEmpty
                    iconType={'email'}
                    color={'danger'}
                    target="_blank"
                    title={'Activate your account to access all features.'}
                    href={'/activate'}
                    flush={'left'}
                  >
                    Activate account
                  </EuiButtonEmpty>
                  <br />
                </>
              )}
              <EuiButtonEmpty
                iconType={'trash'}
                color={'danger'}
                title={'The action is not supported yet. Please, contact us instead.'}
                isDisabled
                flush={'left'}
              >
                Delete account
              </EuiButtonEmpty>
            </>
          }
        >
          <EuiFormRow label={'Email'} helpText={'Used for all communications and notifications.'} fullWidth isDisabled>
            <EuiFieldText type={'email'} value={uiState.user?.email} />
          </EuiFormRow>
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup
          title={<h3>Subscription</h3>}
          description={
            uiState.subscription?.manageUrl ? (
              <>
                {subscriptionDescription}
                <br />
                <br />
                <EuiButtonEmpty
                  iconType={'payment'}
                  flush={'left'}
                  href={uiState.subscription.manageUrl}
                  target="_blank"
                >
                  Manage subscription
                </EuiButtonEmpty>
              </>
            ) : (
              subscriptionDescription
            )
          }
        >
          <EuiFormRow
            label="Tier"
            helpText={
              uiState.subscription?.featureOverviewUrl ? (
                <span>
                  Compare your current tier to other available tiers at the{' '}
                  <EuiLink target="_blank" href={uiState.subscription.featureOverviewUrl}>
                    <b>feature overview page</b>
                  </EuiLink>
                  .
                </span>
              ) : null
            }
            fullWidth
          >
            <EuiText
              css={css`
                text-transform: capitalize;
              `}
              size={'s'}
            >
              <b>{subscription?.tier ?? ''}</b>
            </EuiText>
          </EuiFormRow>
          {trialSection}
        </EuiDescribedFormGroup>
      </>
    );
  }

  const reauthenticateModal = isReauthenticateModalVisible.visible ? (
    <ConfirmAccessModal
      email={uiState.user!.email}
      action={isReauthenticateModalVisible.action}
      onClose={() => setIsReauthenticateModalVisible({ visible: false })}
    />
  ) : null;

  return (
    <EuiFlyout
      size="m"
      maxWidth
      onClose={() => onClose()}
      ownFocus={true}
      maskProps={{ headerZindexLocation: 'above' }}
    >
      <EuiFlyoutHeader>
        <EuiTitle size="s">
          <h1>Settings</h1>
        </EuiTitle>
      </EuiFlyoutHeader>
      <EuiFlyoutBody>
        <EuiTabs>
          <EuiTab onClick={() => setSelectedTab('general')} isSelected={selectedTab === 'general'}>
            General
          </EuiTab>
          <EuiTab onClick={() => setSelectedTab('security')} isSelected={selectedTab === 'security'}>
            Security
          </EuiTab>
          <EuiTab onClick={() => setSelectedTab('account')} isSelected={selectedTab === 'account'}>
            Account
          </EuiTab>
        </EuiTabs>
        <EuiSpacer />
        {selectedTabContent}
        {reauthenticateModal}
      </EuiFlyoutBody>
    </EuiFlyout>
  );
}
