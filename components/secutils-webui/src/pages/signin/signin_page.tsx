import {
  EuiButton,
  EuiButtonEmpty,
  EuiFieldText,
  EuiForm,
  EuiFormRow,
  EuiHorizontalRule,
  EuiPanel,
} from '@elastic/eui';
import type { FrontendApi, LoginFlow, UiNodeInputAttributes } from '@ory/client';
import type { ChangeEvent } from 'react';
import { useCallback, useState } from 'react';
import { Navigate, useNavigate, useSearchParams } from 'react-router';

import { RecoverAccountModal } from './recover_account_modal';
import { useAppContext, usePageMeta } from '../../hooks';
import { getCsrfToken, getSecurityErrorMessage, isClientError } from '../../model';
import type { AsyncData, SerializedPublicKeyCredentialRequestOptions } from '../../model';
import { signinWithPasskey } from '../../model/webauthn';
import { getOryApi } from '../../tools/ory';
import { isSafeNextUrl } from '../../tools/url';
import { isWebAuthnSupported } from '../../tools/webauthn';
import { Page } from '../page';

async function getSigninFlow(api: FrontendApi, searchParams: URLSearchParams) {
  const flowId = searchParams.get('flow');
  if (flowId) {
    // Try to retrieve the existing flow first, otherwise create a new one.
    try {
      return await api.getLoginFlow({ id: flowId });
    } catch (err) {
      console.error('Failed to initialize signin flow.', err);
    }
  }

  return await api.createBrowserLoginFlow();
}
export function SigninPage() {
  usePageMeta('Sign-in');

  const [searchParams, setSearchParams] = useSearchParams();

  const navigate = useNavigate();
  const { uiState, refreshUiState, addToast } = useAppContext();

  const [email, setEmail] = useState<string>('');
  const onEmailChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setEmail(e.target.value);
  }, []);

  const [password, setPassword] = useState<string>('');
  const onPasswordChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setPassword(e.target.value);
  }, []);

  const [isPasskeySupported] = useState<boolean>(isWebAuthnSupported());

  function startSigninFlow(signinFunc: (api: FrontendApi, flow: LoginFlow) => Promise<void>) {
    getOryApi()
      .then(async (api) => {
        // Start/retrieve a flow and remember it in the URL.
        const { data: flow } = await getSigninFlow(api, searchParams);

        searchParams.set('flow', flow.id);
        setSearchParams(searchParams);

        await signinFunc(api, flow);

        refreshUiState();
      })
      .catch((err: Error) => {
        const originalErrorMessage = getSecurityErrorMessage(err);
        setSigninStatus({ status: 'failed', error: originalErrorMessage ?? 'Unknown error' });

        addToast({
          id: `signin-toast-${Math.floor(Math.random() * 100)}`,
          color: 'danger',
          title: 'Failed to sign in',
          text: (
            <>
              {isClientError(err) && originalErrorMessage
                ? originalErrorMessage
                : 'Unable to sign you in, please try again later or contact us.'}
            </>
          ),
        });
      });
  }

  const [signinStatus, setSigninStatus] = useState<AsyncData<null, { isPasskey: boolean }> | null>(null);
  const [isResetPasswordModalOpen, setIsResetPasswordModalOpen] = useState(false);
  const onToggleResetPasswordModal = useCallback(() => {
    setIsResetPasswordModalOpen((isOpen) => !isOpen);
  }, []);

  const resetPasswordModal = isResetPasswordModalOpen ? (
    <RecoverAccountModal onClose={onToggleResetPasswordModal} email={email} />
  ) : null;

  if (uiState.user) {
    const urlToRedirect = searchParams.get('next');
    return <Navigate to={urlToRedirect && isSafeNextUrl(urlToRedirect) ? urlToRedirect : '/ws'} />;
  }

  return (
    <Page contentAlignment={'center'}>
      <EuiPanel>
        <EuiForm id="signin-form" component="form" className="signin-form">
          <EuiFormRow>
            <EuiFieldText
              placeholder="Email"
              value={email}
              autoComplete={'email'}
              type={'email'}
              disabled={signinStatus?.status === 'pending'}
              onChange={onEmailChange}
            />
          </EuiFormRow>
          <EuiFormRow>
            <EuiFieldText
              placeholder="Password"
              value={password}
              type={'password'}
              disabled={signinStatus?.status === 'pending'}
              onChange={onPasswordChange}
            />
          </EuiFormRow>
          <EuiFormRow>
            <EuiButton
              type="submit"
              form="signin-form"
              fill
              fullWidth
              onClick={(e) => {
                e.preventDefault();

                if (signinStatus?.status === 'pending') {
                  return;
                }

                setSigninStatus({ status: 'pending', state: { isPasskey: false } });
                startSigninFlow(async (api, flow) => {
                  await api.updateLoginFlow({
                    flow: flow.id,
                    updateLoginFlowBody: {
                      method: 'password' as const,
                      password,
                      csrf_token: getCsrfToken(flow),
                      identifier: email,
                    },
                  });
                });
              }}
              isLoading={signinStatus?.status === 'pending' && signinStatus?.state?.isPasskey !== true}
              isDisabled={
                email.trim().length === 0 ||
                email.includes(' ') ||
                !email.includes('@') ||
                password.trim().length === 0 ||
                signinStatus?.status === 'pending'
              }
            >
              Sign in
            </EuiButton>
          </EuiFormRow>
          {isPasskeySupported ? (
            <>
              <EuiFormRow>
                <EuiHorizontalRule size={'half'} margin="xs" />
              </EuiFormRow>
              <EuiFormRow>
                <EuiButton
                  type="submit"
                  form="signin-form"
                  fill
                  fullWidth
                  onClick={(e) => {
                    e.preventDefault();

                    if (signinStatus?.status === 'pending') {
                      return;
                    }

                    setSigninStatus({ status: 'pending', state: { isPasskey: true } });
                    startSigninFlow(async (api, flow) => {
                      const axiosResponse = await api.updateLoginFlow(
                        {
                          flow: flow.id,
                          updateLoginFlowBody: {
                            method: 'webauthn' as const,
                            csrf_token: getCsrfToken(flow),
                            identifier: email,
                          },
                        },
                        { validateStatus: (status) => status < 500 },
                      );

                      const { data: updatedFlow } = await api.getLoginFlow({ id: flow.id });
                      const publicKeyNode = updatedFlow?.ui?.nodes?.find(
                        (node) =>
                          node.attributes.node_type === 'input' && node.attributes.name === 'webauthn_login_trigger',
                      );
                      if (!publicKeyNode) {
                        throw axiosResponse;
                      }

                      const publicKey = (
                        JSON.parse((publicKeyNode.attributes as UiNodeInputAttributes).value as string) as {
                          publicKey: SerializedPublicKeyCredentialRequestOptions;
                        }
                      ).publicKey;

                      await api.updateLoginFlow({
                        flow: updatedFlow.id,
                        updateLoginFlowBody: {
                          method: 'webauthn' as const,
                          csrf_token: getCsrfToken(updatedFlow),
                          webauthn_login: await signinWithPasskey(publicKey),
                          identifier: email,
                        },
                      });
                    });
                  }}
                  isLoading={signinStatus?.status === 'pending' && signinStatus?.state?.isPasskey === true}
                  isDisabled={email.trim().length === 0 || signinStatus?.status === 'pending'}
                >
                  Sign in with passkey
                </EuiButton>
              </EuiFormRow>
            </>
          ) : null}

          <EuiFormRow className="eui-textCenter">
            <>
              <EuiButtonEmpty
                size={'s'}
                onClick={() => {
                  navigate('/signup');
                }}
              >
                Create account
              </EuiButtonEmpty>
              <EuiButtonEmpty
                size={'s'}
                onClick={() => {
                  setIsResetPasswordModalOpen(true);
                }}
              >
                Cannot sign in?
              </EuiButtonEmpty>
            </>
          </EuiFormRow>
        </EuiForm>
        {resetPasswordModal}
      </EuiPanel>
    </Page>
  );
}
