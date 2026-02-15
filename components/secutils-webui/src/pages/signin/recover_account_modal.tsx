import {
  EuiButton,
  EuiButtonEmpty,
  EuiButtonIcon,
  EuiFieldText,
  EuiForm,
  EuiFormRow,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiTitle,
} from '@elastic/eui';
import type { FrontendApi, RecoveryFlow } from '@ory/kratos-client-fetch';
import type { MouseEventHandler } from 'react';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';

import { useAppContext } from '../../hooks';
import { type AsyncData, getCsrfToken, getErrorStatus, getSecurityErrorMessage } from '../../model';
import { getOryApi } from '../../tools/ory';

export interface RecoverAccountModalProps {
  email?: string;
  onClose: () => void;
}

async function getRecoverFlow(api: FrontendApi, flowId?: string) {
  if (flowId) {
    // Try to retrieve the existing flow first, otherwise create a new one.
    try {
      return await api.getRecoveryFlow({ id: flowId });
    } catch (err) {
      console.error('Failed to initialize verification flow.', err);
    }
  }

  return await api.createBrowserRecoveryFlow();
}

export function RecoverAccountModal({ email, onClose }: RecoverAccountModalProps) {
  const { addToast, uiState, refreshUiState } = useAppContext();
  const navigate = useNavigate();

  const [userEmail, setUserEmail] = useState<string>(email ?? '');
  const [recoveryCode, setRecoveryCode] = useState<string>('');

  const [accountRecoveryStatus, setAccountRecoveryStatus] = useState<AsyncData<undefined, RecoveryFlow> | null>(null);
  const onSendRecoveryCode: MouseEventHandler<HTMLButtonElement> = (e) => {
    e.preventDefault();

    if (accountRecoveryStatus?.status === 'pending') {
      return;
    }

    const recoverFlow = accountRecoveryStatus?.state;
    setAccountRecoveryStatus({ status: 'pending', state: recoverFlow });

    getOryApi()
      .then(async (api) => {
        const flow = recoverFlow ?? (await getRecoverFlow(api));
        const errorMessage = await getSecurityErrorMessage(
          await api.updateRecoveryFlow({
            flow: flow.id,
            updateRecoveryFlowBody: { method: 'code', csrf_token: getCsrfToken(flow), email: userEmail },
          }),
        );
        if (errorMessage) {
          throw new Error(errorMessage);
        }

        setAccountRecoveryStatus({ status: 'succeeded', data: undefined, state: flow });
        addToast({
          id: 'send-recovery-code',
          color: 'success',
          title: 'Account recovery code sent',
          text: (
            <>
              Account recovery code is on its way to your email. If you don&apos;t see it soon, please check your spam
              folder.
            </>
          ),
        });
      })
      .catch(async (err: Error) => {
        setAccountRecoveryStatus({
          status: 'failed',
          error: (await getSecurityErrorMessage(err)) ?? 'Unknown error occurred',
          state: recoverFlow,
        });

        addToast({
          id: 'send-recovery-code-error',
          color: 'danger',
          title: 'Failed to send account recovery code',
          text: <>Unable to send account recovery code, please try again later.</>,
        });
      });
  };

  const onRecoverAccount: MouseEventHandler<HTMLButtonElement> = (e) => {
    e.preventDefault();

    if (accountRecoveryStatus?.status === 'pending' || !accountRecoveryStatus?.state) {
      return;
    }

    const recoverFlow = accountRecoveryStatus.state;
    setAccountRecoveryStatus({ status: 'pending', state: recoverFlow });

    getOryApi()
      .then(async (api) => {
        // Successful recovery should result into 422 HTTP status code that requires redirect.
        throw await api.updateRecoveryFlow({
          flow: recoverFlow.id,
          updateRecoveryFlowBody: { method: 'code', csrf_token: getCsrfToken(recoverFlow), code: recoveryCode },
        });
      })
      .catch(async (err: unknown) => {
        if (getErrorStatus(err) !== 422) {
          setAccountRecoveryStatus({
            status: 'failed',
            error: (await getSecurityErrorMessage(err)) ?? 'Unknown error occurred',
            state: recoverFlow,
          });
          addToast({
            id: 'account-recovery-error',
            color: 'danger',
            title: 'Failed to recover account',
            text: <>Unable to recover account with the provided recovery code, please try again later.</>,
          });
          return;
        }

        setAccountRecoveryStatus({ status: 'succeeded', data: undefined, state: recoverFlow });
        addToast({
          id: 'account-recovery-success',
          color: 'success',
          title: 'Account access is recovered',
          text: (
            <>
              You&apos;ve regained access to your account. Please navigate to the Settings and update your credentials.
            </>
          ),
        });

        refreshUiState();
      });
  };

  useEffect(() => {
    if (uiState.user) {
      navigate('/ws');
    }
  }, [uiState, navigate]);

  const awaitingRecoveryCode = !!accountRecoveryStatus?.state;
  return (
    <EuiModal onClose={onClose}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size={'s'}>
            <span>Recover your account</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm id="account-recovery-form" component="form">
          <EuiFormRow label="Email">
            <EuiFieldText
              value={userEmail}
              autoComplete={'email'}
              type={'email'}
              required
              disabled={awaitingRecoveryCode}
              onChange={(e) => setUserEmail(e.target.value)}
            />
          </EuiFormRow>
          {awaitingRecoveryCode ? (
            <EuiFormRow label={'Recovery code'}>
              <EuiFieldText
                value={recoveryCode}
                autoComplete={'off'}
                type={'text'}
                append={<EuiButtonIcon iconType="refresh" onClick={onSendRecoveryCode} aria-label="Resend code" />}
                onChange={(e) => setRecoveryCode(e.target.value)}
              />
            </EuiFormRow>
          ) : null}
        </EuiForm>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty disabled={accountRecoveryStatus?.status === 'pending'} onClick={onClose}>
          Cancel
        </EuiButtonEmpty>
        {awaitingRecoveryCode ? (
          <EuiButton
            type="submit"
            form="account-recovery-form"
            fill
            disabled={accountRecoveryStatus?.status === 'pending' || !userEmail?.trim() || !recoveryCode?.trim()}
            onClick={onRecoverAccount}
            isLoading={accountRecoveryStatus?.status === 'pending'}
          >
            Submit
          </EuiButton>
        ) : (
          <EuiButton
            type="submit"
            form="account-recovery-form"
            fill
            disabled={accountRecoveryStatus?.status === 'pending' || !userEmail?.trim()}
            onClick={onSendRecoveryCode}
            isLoading={accountRecoveryStatus?.status === 'pending'}
          >
            Send code
          </EuiButton>
        )}
      </EuiModalFooter>
    </EuiModal>
  );
}
