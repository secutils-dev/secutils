import { type ChangeEvent, useCallback, useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router';

import { EuiButton, EuiFieldText, EuiForm, EuiFormRow, EuiLink, EuiPanel } from '@elastic/eui';
import type { FrontendApi, VerificationFlow } from '@ory/client';

import { SettingsFlyout } from '../../app_container';
import { PageErrorState, PageLoadingState, PageSuccessState } from '../../components';
import { useAppContext, usePageHeaderActions, usePageMeta } from '../../hooks';
import { type AsyncData, getCsrfToken, getSecurityErrorMessage } from '../../model';
import { getOryApi } from '../../tools/ory';

async function getVerificationFlow(api: FrontendApi, flowId?: string) {
  if (flowId) {
    // Try to retrieve existing flow first, otherwise create a new one.
    try {
      return (await api.getVerificationFlow({ id: flowId })).data;
    } catch (err) {
      console.error('Failed to initialize verification flow.', err);
    }
  }

  return (await api.createBrowserVerificationFlow()).data;
}

import { Page } from '../page';

type VerificationProcess =
  | {
      step: 'activated' | 'doesnt_require_activation';
    }
  | {
      step: 'awaiting_reactivation' | 'awaiting_activation_code';
      email: string;
      flow: VerificationFlow;
      isLoading: boolean;
    };

export function ActivatePage() {
  usePageMeta('Activate account');

  const { addToast, uiState } = useAppContext();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const { actions, isSettingsOpen, hideSettings } = usePageHeaderActions();

  const [code, setCode] = useState<string | undefined>(searchParams.get('code') ?? undefined);
  const onCodeChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setCode(e.target.value);
  }, []);

  const [process, setProcess] = useState<AsyncData<VerificationProcess>>({
    status: 'pending',
  });
  useEffect(() => {
    // Don't try to initialize the flow if the status is already set or the user is already activated.
    const user = uiState.user;
    if (process.status !== 'pending' || !user) {
      return;
    }

    if (user.isActivated) {
      setProcess({ status: 'succeeded', data: { step: 'doesnt_require_activation' } });
      return;
    }

    getOryApi()
      .then(async (api) => {
        const flowId = searchParams.get('flow') ?? undefined;

        // If the code is available in the query parameters, try to verify the account straight away.
        const flow = await getVerificationFlow(api, flowId);
        if (code && flowId === flow.id) {
          const successfullyActivated = !getSecurityErrorMessage(
            await api.updateVerificationFlow({
              flow: flow.id,
              updateVerificationFlowBody: { method: 'code', code, csrf_token: getCsrfToken(flow) },
            }),
          );
          if (!successfullyActivated) {
            throw new Error('Verification failed.');
          }

          setProcess({ status: 'succeeded', data: { step: 'activated' } });
        } else {
          // Reset activation code and update flow ID in the query string.
          setCode(undefined);
          searchParams.set('flow', flow.id);
          searchParams.delete('code');
          setSearchParams(searchParams);

          setProcess({
            status: 'succeeded',
            data: { step: 'awaiting_reactivation', email: user.email, flow, isLoading: false },
          });
        }
      })
      .catch(() => {
        setProcess({
          status: 'failed',
          error:
            'The activation link is not valid or may have already expired. You can request a new link from the account settings.',
        });
      });
  }, [searchParams, code, process, uiState]);

  const onSendActivationLink = (flow: VerificationFlow, email: string) => {
    setProcess({ status: 'succeeded', data: { step: 'awaiting_reactivation', flow, email, isLoading: true } });
    getOryApi()
      .then(async (api) => {
        const successfullyActivated = !getSecurityErrorMessage(
          await api.updateVerificationFlow({
            flow: flow.id,
            updateVerificationFlowBody: { method: 'code', csrf_token: getCsrfToken(flow), email },
          }),
        );
        if (successfullyActivated) {
          addToast({
            id: 'send-activation-link',
            color: 'success',
            title: 'Activation link sent',
            text: (
              <>
                Activation link is on its way to your email. If you don&apos;t see it soon, please check your spam
                folder.
              </>
            ),
          });
          setProcess({
            status: 'succeeded',
            data: { step: 'awaiting_activation_code', flow, email, isLoading: false },
          });
        } else {
          throw new Error('Verification failed.');
        }
      })
      .catch((err: Error) => {
        setProcess({ status: 'failed', error: err?.message ?? 'Failed to resend activation link.' });
      });
  };

  const onActivate = (flow: VerificationFlow, email: string) => {
    setProcess({ status: 'succeeded', data: { step: 'awaiting_activation_code', flow, email, isLoading: true } });
    getOryApi()
      .then(async (api) => {
        const successfullyActivated = !getSecurityErrorMessage(
          await api.updateVerificationFlow({
            flow: flow.id,
            updateVerificationFlowBody: { method: 'code', csrf_token: getCsrfToken(flow), code },
          }),
        );
        if (successfullyActivated) {
          setProcess({ status: 'succeeded', data: { step: 'activated' } });
        } else {
          throw new Error('Verification failed.');
        }
      })
      .catch((err: Error) => {
        setProcess({ status: 'failed', error: err?.message ?? 'The activation code is invalid.' });
      });
  };

  const continueToLink = (
    <p>
      <EuiLink
        href={'/ws'}
        onClick={(e) => {
          e.preventDefault();
          navigate('/ws');
        }}
      >
        Continue to workspace
      </EuiLink>
    </p>
  );

  let pageBody = null;
  if (process.status === 'pending') {
    pageBody = <PageLoadingState title={'Activating your account…'} />;
  } else if (process.status === 'failed') {
    pageBody = (
      <PageErrorState title="Cannot activate account" content={<p>{process.error}</p>} action={continueToLink} />
    );
  } else if (process.data.step === 'doesnt_require_activation') {
    pageBody = (
      <PageSuccessState
        title="Account already activated"
        content={<p>Your account has already been activated.</p>}
        action={continueToLink}
      />
    );
  } else if (process.data.step === 'activated') {
    pageBody = (
      <PageSuccessState
        title="Successfully activated account"
        content={<p>Your account has been successfully activated!</p>}
        action={continueToLink}
      />
    );
  } else if (process.data.step === 'awaiting_reactivation') {
    const data = process.data;
    pageBody = (
      <EuiPanel>
        <EuiForm id="activation-form" component="form">
          <EuiFormRow>
            <EuiFieldText placeholder="Email" value={data.email} autoComplete={'email'} type={'email'} disabled />
          </EuiFormRow>
          <EuiFormRow>
            <EuiButton
              type="submit"
              form="activation-form"
              fill
              fullWidth
              onClick={(e) => {
                e.preventDefault();
                onSendActivationLink(data.flow, data.email);
              }}
              isLoading={process.data.isLoading}
              isDisabled={process.data.isLoading}
            >
              Send activation code
            </EuiButton>
          </EuiFormRow>
        </EuiForm>
      </EuiPanel>
    );
  } else if (process.data.step === 'awaiting_activation_code') {
    const data = process.data;
    pageBody = (
      <EuiPanel>
        <EuiForm id="activation-form" component="form">
          <EuiFormRow>
            <EuiFieldText placeholder="Email" value={data.email} autoComplete={'email'} type={'email'} disabled />
          </EuiFormRow>
          <EuiFormRow>
            <EuiFieldText
              placeholder="Activation code"
              value={code ?? ''}
              autoComplete={'off'}
              type={'text'}
              disabled={process.data.isLoading}
              onChange={onCodeChange}
            />
          </EuiFormRow>
          <EuiFormRow>
            <EuiButton
              type="submit"
              form="activation-form"
              fill
              fullWidth
              onClick={(e) => {
                e.preventDefault();
                onActivate(data.flow, data.email);
              }}
              isLoading={process.data.isLoading}
              isDisabled={process.data.isLoading || !code || code.trim().length === 0}
            >
              Activate account
            </EuiButton>
          </EuiFormRow>
        </EuiForm>
      </EuiPanel>
    );
  }

  return (
    <Page
      contentAlignment={'center'}
      headerActions={actions}
      headerBreadcrumbs={[{ text: 'Account' }, { text: 'Activation' }]}
    >
      <>
        {pageBody}
        {isSettingsOpen ? <SettingsFlyout onClose={hideSettings} /> : null}
      </>
    </Page>
  );
}
