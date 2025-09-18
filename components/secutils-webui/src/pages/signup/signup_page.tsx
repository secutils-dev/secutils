import {
  EuiButton,
  EuiButtonEmpty,
  euiCanAnimate,
  EuiFieldText,
  EuiForm,
  EuiFormRow,
  EuiHorizontalRule,
  EuiPanel,
  useEuiTheme,
} from '@elastic/eui';
import { css } from '@emotion/react';
import type { FrontendApi, RegistrationFlow, UiNodeInputAttributes } from '@ory/client';
import type { ChangeEvent, MouseEventHandler } from 'react';
import { useCallback, useState } from 'react';
import { Navigate, useNavigate, useSearchParams } from 'react-router';

import { useAppContext, usePageMeta } from '../../hooks';
import { getCsrfToken, getSecurityErrorMessage, isClientError } from '../../model';
import type { AsyncData, SerializedPublicKeyCredentialCreationOptions } from '../../model';
import { signupWithPasskey } from '../../model/webauthn';
import { getOryApi } from '../../tools/ory';
import { isWebAuthnSupported } from '../../tools/webauthn';
import { Page } from '../page';

enum FormState {
  Default,
  WithPassword,
}

async function getSignupFlow(api: FrontendApi, searchParams: URLSearchParams) {
  const flowId = searchParams.get('flow');
  if (flowId) {
    // Try to retrieve the existing flow first, otherwise create a new one.
    try {
      return await api.getRegistrationFlow({ id: flowId });
    } catch (err) {
      console.error('Failed to initialize signup flow.', err);
    }
  }

  return await api.createBrowserRegistrationFlow();
}

export function SignupPage() {
  usePageMeta('Sign-up');

  const [searchParams, setSearchParams] = useSearchParams();

  const navigate = useNavigate();
  const { uiState, refreshUiState, addToast } = useAppContext();
  const theme = useEuiTheme();

  const [formState, setFormState] = useState(FormState.Default);

  const [email, setEmail] = useState<string>('');
  const onEmailChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setEmail(e.target.value);
  }, []);

  const [password, setPassword] = useState<string>('');
  const onPasswordChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setPassword(e.target.value);
  }, []);

  const [repeatPassword, setRepeatPassword] = useState<string>('');
  const onRepeatPasswordChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setRepeatPassword(e.target.value);
  }, []);

  const [isPasskeySupported] = useState<boolean>(isWebAuthnSupported());

  function startSignupFlow(signupFunc: (api: FrontendApi, flow: RegistrationFlow) => Promise<void>) {
    getOryApi()
      .then(async (api) => {
        // Start/retrieve a flow and remember it in the URL.
        const { data: flow } = await getSignupFlow(api, searchParams);
        setSearchParams({ flow: flow.id });

        await signupFunc(api, flow);

        refreshUiState();
      })
      .catch((err) => {
        const originalErrorMessage = getSecurityErrorMessage(err);
        setSignupStatus({ status: 'failed', error: originalErrorMessage ?? 'Unknown error' });

        addToast({
          id: `signup-toast-${Math.floor(Math.random() * 100)}`,
          color: 'danger',
          title: 'Failed to sign up',
          text: (
            <>
              {isClientError(err) && originalErrorMessage
                ? originalErrorMessage
                : 'Unable to sign you up, please try again later or contact us.'}
            </>
          ),
        });
      });
  }

  const [signupStatus, setSignupStatus] = useState<AsyncData<null, { isPasskey: boolean }> | null>(null);
  const onContinueWithPassword: MouseEventHandler<HTMLButtonElement> = useCallback((e) => {
    e.preventDefault();

    setFormState(FormState.WithPassword);
  }, []);

  if (uiState.user) {
    return <Navigate to="/ws" />;
  }

  const signupWithPasswordButton =
    formState === FormState.Default && isPasskeySupported ? (
      <EuiButton
        type="button"
        fill
        fullWidth
        onClick={onContinueWithPassword}
        isDisabled={email.trim().length === 0 || signupStatus?.status === 'pending'}
      >
        Continue with password
      </EuiButton>
    ) : (
      <EuiButton
        type="submit"
        form="signup-form"
        fill
        fullWidth
        onClick={(e) => {
          e.preventDefault();

          if (signupStatus?.status === 'pending') {
            return;
          }

          setSignupStatus({ status: 'pending', state: { isPasskey: false } });
          startSignupFlow(async (api, flow) => {
            await api.updateRegistrationFlow({
              flow: flow.id,
              updateRegistrationFlowBody: {
                method: 'password' as const,
                password,
                csrf_token: getCsrfToken(flow),
                traits: { email },
              },
            });
          });
        }}
        isLoading={signupStatus?.status === 'pending' && signupStatus?.state?.isPasskey !== true}
        isDisabled={
          email.trim().length === 0 ||
          password.trim().length === 0 ||
          password !== repeatPassword ||
          signupStatus?.status === 'pending'
        }
      >
        Sign up
      </EuiButton>
    );

  // Use transition to show password fields, and workaround fixed margin-top for the hidden fields.
  const passwordFieldStyles = css`
    max-height: ${formState === FormState.Default && isPasskeySupported ? 0 : theme.euiTheme.size.xxl};
    margin-top: ${formState === FormState.Default && isPasskeySupported ? '0 !important' : 'unset'};
    overflow: hidden;
    ${euiCanAnimate} {
      transition: max-height 1s ${theme.euiTheme.animation.bounce};
    }
  `;

  return (
    <Page contentAlignment={'center'}>
      <EuiPanel>
        <EuiForm id="signup-form" component="form" autoComplete="off" fullWidth className="signup-form">
          <EuiFormRow>
            <EuiFieldText
              placeholder="Email"
              value={email}
              type={'email'}
              autoComplete="email"
              disabled={signupStatus?.status === 'pending'}
              onChange={onEmailChange}
            />
          </EuiFormRow>
          <EuiFormRow css={passwordFieldStyles}>
            <EuiFieldText
              placeholder="Password"
              value={password}
              type={'password'}
              autoComplete="new-password"
              disabled={signupStatus?.status === 'pending'}
              onChange={onPasswordChange}
            />
          </EuiFormRow>
          <EuiFormRow css={passwordFieldStyles}>
            <EuiFieldText
              placeholder="Repeat password"
              value={repeatPassword}
              type={'password'}
              autoComplete="new-password"
              isInvalid={!!repeatPassword && !!password && repeatPassword !== password}
              disabled={signupStatus?.status === 'pending'}
              onChange={onRepeatPasswordChange}
            />
          </EuiFormRow>
          <EuiFormRow>{signupWithPasswordButton}</EuiFormRow>
          {isPasskeySupported ? (
            <>
              <EuiFormRow>
                <EuiHorizontalRule size={'half'} margin="m" />
              </EuiFormRow>
              <EuiFormRow>
                <EuiButton
                  type="submit"
                  form="signup-form"
                  fill
                  fullWidth
                  onClick={(e) => {
                    e.preventDefault();

                    if (signupStatus?.status === 'pending') {
                      return;
                    }

                    setSignupStatus({ status: 'pending', state: { isPasskey: true } });
                    startSignupFlow(async (api, flow) => {
                      const response = await api.updateRegistrationFlow(
                        {
                          flow: flow.id,
                          updateRegistrationFlowBody: {
                            method: 'profile' as const,
                            csrf_token: getCsrfToken(flow),
                            traits: { email },
                          },
                        },
                        { validateStatus: (status) => status < 500 },
                      );

                      const updatedFlow = response.data as unknown as RegistrationFlow;
                      const publicKeyNode = updatedFlow?.ui?.nodes?.find(
                        (node) =>
                          node.attributes.node_type === 'input' && node.attributes.name === 'webauthn_register_trigger',
                      );
                      if (!publicKeyNode) {
                        throw response;
                      }

                      const { publicKey } = JSON.parse(
                        (publicKeyNode.attributes as UiNodeInputAttributes).value as string,
                      ) as {
                        publicKey: SerializedPublicKeyCredentialCreationOptions;
                      };

                      await api.updateRegistrationFlow({
                        flow: updatedFlow.id,
                        updateRegistrationFlowBody: {
                          method: 'webauthn' as const,
                          csrf_token: getCsrfToken(updatedFlow),
                          webauthn_register: await signupWithPasskey(publicKey),
                          webauthn_register_displayname: email,
                          traits: { email },
                        },
                      });
                    });
                  }}
                  isLoading={signupStatus?.status === 'pending' && signupStatus?.state?.isPasskey === true}
                  isDisabled={email.trim().length === 0 || signupStatus?.status === 'pending'}
                >
                  Sign up with passkey
                </EuiButton>
              </EuiFormRow>
            </>
          ) : null}
          <EuiFormRow className="eui-textCenter">
            <EuiButtonEmpty
              size={'s'}
              onClick={() => {
                navigate('/signin');
              }}
            >
              Sign in instead
            </EuiButtonEmpty>
          </EuiFormRow>
        </EuiForm>
      </EuiPanel>
    </Page>
  );
}
