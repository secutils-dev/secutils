import type { ChangeEvent, MouseEventHandler } from 'react';
import { useCallback, useState } from 'react';

import {
  EuiButton,
  EuiFieldText,
  EuiForm,
  EuiFormRow,
  EuiHorizontalRule,
  EuiModal,
  EuiModalBody,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiTitle,
} from '@elastic/eui';
import type { FrontendApi, LoginFlow, UiNodeInputAttributes } from '@ory/client';

import { useAppContext } from '../../hooks';
import type { AsyncData, SerializedPublicKeyCredentialRequestOptions } from '../../model';
import { getCsrfToken, getSecurityErrorMessage, isClientError } from '../../model';
import { signinWithPasskey } from '../../model/webauthn';
import { getOryApi } from '../../tools/ory';
import { isWebAuthnSupported } from '../../tools/webauthn';

export interface ConfirmAccessModalProps {
  email: string;
  action: () => Promise<void>;
  onClose: () => void;
}

export default function ConfirmAccessModal({ email, action, onClose }: ConfirmAccessModalProps) {
  const { refreshUiState, addToast } = useAppContext();

  const [password, setPassword] = useState<string>('');
  const onPasswordChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setPassword(e.target.value);
  }, []);

  const [isPasskeySupported] = useState<boolean>(isWebAuthnSupported());

  function startSigninFlow(signinFunc: (api: FrontendApi, flow: LoginFlow) => Promise<void>) {
    getOryApi()
      .then(async (api) => {
        const { data: flow } = await api.createBrowserLoginFlow({ refresh: true });

        await signinFunc(api, flow);
        refreshUiState();

        try {
          await action();
        } finally {
          onClose();
        }
      })
      .catch((err: Error) => {
        const originalErrorMessage = getSecurityErrorMessage(err);
        setSigninStatus({ status: 'failed', error: originalErrorMessage ?? 'Unknown error' });

        addToast({
          id: 'confirm-access-toast',
          color: 'danger',
          title: 'Failed to confirm access',
          text: (
            <>
              {isClientError(err) && originalErrorMessage
                ? originalErrorMessage
                : 'Unable to confirm access, please try again later or contact us.'}
            </>
          ),
        });
      });
  }

  const [signinStatus, setSigninStatus] = useState<AsyncData<null, { isPasskey: boolean }> | null>(null);
  const onSignin: MouseEventHandler<HTMLButtonElement> = useCallback(
    (e) => {
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
    },
    [email, password, signinStatus],
  );

  const onSigninWithPasskey: MouseEventHandler<HTMLButtonElement> = useCallback(
    (e) => {
      e.preventDefault();

      if (signinStatus?.status === 'pending') {
        return;
      }

      setSigninStatus({ status: 'pending', state: { isPasskey: true } });
      startSigninFlow(async (api, flow) => {
        const axiosResponse = await api.updateLoginFlow(
          {
            flow: flow.id,
            updateLoginFlowBody: { method: 'webauthn' as const, csrf_token: getCsrfToken(flow), identifier: email },
          },
          { validateStatus: (status) => status < 500 },
        );

        const { data: updatedFlow } = await api.getLoginFlow({ id: flow.id });
        const publicKeyNode = updatedFlow?.ui?.nodes?.find(
          (node) => node.attributes.node_type === 'input' && node.attributes.name === 'webauthn_login_trigger',
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
    },
    [email, signinStatus],
  );

  return (
    <EuiModal onClose={() => onClose()}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size={'s'}>
            <span>Confirm access</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm id="signin-form" component="form">
          <EuiFormRow>
            <EuiFieldText placeholder="Email" value={email} autoComplete={'email'} type={'email'} disabled />
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
              onClick={onSignin}
              isLoading={signinStatus?.status === 'pending' && signinStatus?.state?.isPasskey !== true}
              isDisabled={
                email.trim().length === 0 ||
                email.includes(' ') ||
                !email.includes('@') ||
                password.trim().length === 0 ||
                signinStatus?.status === 'pending'
              }
            >
              Use password
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
                  onClick={onSigninWithPasskey}
                  isLoading={signinStatus?.status === 'pending' && signinStatus?.state?.isPasskey === true}
                  isDisabled={email.trim().length === 0 || signinStatus?.status === 'pending'}
                >
                  Use passkey or security key
                </EuiButton>
              </EuiFormRow>
            </>
          ) : null}
        </EuiForm>
      </EuiModalBody>
    </EuiModal>
  );
}
