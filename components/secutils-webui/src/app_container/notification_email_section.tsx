import {
  EuiBadge,
  EuiButton,
  EuiButtonEmpty,
  EuiDescribedFormGroup,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormRow,
  EuiText,
} from '@elastic/eui';
import type { ChangeEvent } from 'react';
import { useCallback, useState } from 'react';

import { useAppContext } from '../hooks';
import type { AsyncData, UserNotificationDestination } from '../model';
import {
  clearNotificationEmail,
  getErrorMessage,
  isNotificationDestinationUnsubscribed,
  isNotificationDestinationVerificationPending,
  isNotificationDestinationVerified,
  resendNotificationEmailCode,
  setNotificationEmail,
  verifyNotificationEmail,
} from '../model';

type Mode =
  | { kind: 'idle' }
  | { kind: 'editing'; address: string }
  | { kind: 'verifying'; record: UserNotificationDestination; code: string };

function modeForRecord(record?: UserNotificationDestination): Mode {
  if (!record) {
    return { kind: 'editing', address: '' };
  }
  if (isNotificationDestinationVerificationPending(record)) {
    return { kind: 'verifying', record, code: '' };
  }
  return { kind: 'idle' };
}

export default function NotificationEmailSection() {
  const { uiState, refreshUiState, addToast } = useAppContext();
  const record = uiState.notificationEmail;

  const [mode, setMode] = useState<Mode>(() => modeForRecord(record));
  const [status, setStatus] = useState<AsyncData<null> | null>(null);

  const inFlight = status?.status === 'pending';

  const onSendCode = useCallback(
    (address: string) => {
      if (inFlight) {
        return;
      }
      setStatus({ status: 'pending' });
      setNotificationEmail(address)
        .then((next) => {
          setStatus({ status: 'succeeded', data: null });
          setMode({ kind: 'verifying', record: next, code: '' });
          addToast({
            id: 'notification-email-sent',
            color: 'success',
            title: 'Verification code sent',
            text: <>Check {address} for a 6-digit code.</>,
          });
          refreshUiState();
        })
        .catch((err: Error) => {
          setStatus({ status: 'failed', error: getErrorMessage(err) });
          addToast({
            id: 'notification-email-set-error',
            color: 'danger',
            title: 'Failed to send verification code',
            text: <>{getErrorMessage(err)}</>,
          });
        });
    },
    [inFlight, addToast, refreshUiState],
  );

  const onVerify = useCallback(
    (code: string) => {
      if (inFlight) {
        return;
      }
      setStatus({ status: 'pending' });
      verifyNotificationEmail(code)
        .then(() => {
          setStatus({ status: 'succeeded', data: null });
          setMode({ kind: 'idle' });
          addToast({
            id: 'notification-email-verified',
            color: 'success',
            title: 'Notification email verified',
          });
          refreshUiState();
        })
        .catch((err: Error) => {
          setStatus({ status: 'failed', error: getErrorMessage(err) });
          addToast({
            id: 'notification-email-verify-error',
            color: 'danger',
            title: 'Failed to verify notification email',
            text: <>{getErrorMessage(err)}</>,
          });
        });
    },
    [inFlight, addToast, refreshUiState],
  );

  const onResend = useCallback(() => {
    if (inFlight) {
      return;
    }
    setStatus({ status: 'pending' });
    resendNotificationEmailCode()
      .then(() => {
        setStatus({ status: 'succeeded', data: null });
        addToast({
          id: 'notification-email-resent',
          color: 'success',
          title: 'Verification code resent',
        });
      })
      .catch((err: Error) => {
        setStatus({ status: 'failed', error: getErrorMessage(err) });
        addToast({
          id: 'notification-email-resend-error',
          color: 'danger',
          title: 'Failed to resend verification code',
          text: <>{getErrorMessage(err)}</>,
        });
      });
  }, [inFlight, addToast]);

  const onClear = useCallback(() => {
    if (inFlight) {
      return;
    }
    setStatus({ status: 'pending' });
    clearNotificationEmail()
      .then(() => {
        setStatus({ status: 'succeeded', data: null });
        setMode({ kind: 'editing', address: '' });
        addToast({
          id: 'notification-email-cleared',
          color: 'success',
          title: 'Notification email removed',
        });
        refreshUiState();
      })
      .catch((err: Error) => {
        setStatus({ status: 'failed', error: getErrorMessage(err) });
        addToast({
          id: 'notification-email-clear-error',
          color: 'danger',
          title: 'Failed to remove notification email',
          text: <>{getErrorMessage(err)}</>,
        });
      });
  }, [inFlight, addToast, refreshUiState]);

  let content;
  if (mode.kind === 'idle' && record && isNotificationDestinationVerified(record)) {
    content = (
      <>
        <EuiFormRow label={'Notification email'} fullWidth>
          <EuiFlexGroup gutterSize={'s'} alignItems={'center'} wrap responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiText size={'s'}>{record.address}</EuiText>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiBadge color={'success'}>Verified</EuiBadge>
            </EuiFlexItem>
            {isNotificationDestinationUnsubscribed(record) ? (
              <EuiFlexItem grow={false}>
                <EuiBadge color={'warning'}>Unsubscribed</EuiBadge>
              </EuiFlexItem>
            ) : null}
          </EuiFlexGroup>
        </EuiFormRow>
        <EuiFormRow fullWidth>
          <EuiFlexGroup gutterSize={'s'} responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiButton
                size={'s'}
                onClick={() => setMode({ kind: 'editing', address: record.address })}
                isDisabled={inFlight}
              >
                Change
              </EuiButton>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiButtonEmpty size={'s'} color={'danger'} onClick={onClear} isDisabled={inFlight}>
                Remove
              </EuiButtonEmpty>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiFormRow>
      </>
    );
  } else if (mode.kind === 'verifying') {
    content = (
      <>
        <EuiFormRow label={'Notification email'} fullWidth isDisabled>
          <EuiFieldText type={'email'} value={mode.record.address} />
        </EuiFormRow>
        <EuiFormRow
          label={'Verification code'}
          helpText={'We sent a 6-digit code. The code expires in 15 minutes.'}
          fullWidth
        >
          <EuiFieldText
            inputMode={'numeric'}
            placeholder={'123456'}
            value={mode.code}
            onChange={(e: ChangeEvent<HTMLInputElement>) =>
              setMode({ kind: 'verifying', record: mode.record, code: e.target.value })
            }
          />
        </EuiFormRow>
        <EuiFormRow fullWidth>
          <EuiFlexGroup gutterSize={'s'} responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiButton
                size={'s'}
                fill
                isDisabled={inFlight || mode.code.trim().length === 0}
                isLoading={inFlight}
                onClick={() => onVerify(mode.code.trim())}
              >
                Verify
              </EuiButton>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiButtonEmpty size={'s'} onClick={onResend} isDisabled={inFlight}>
                Resend code
              </EuiButtonEmpty>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiButtonEmpty size={'s'} color={'danger'} onClick={onClear} isDisabled={inFlight}>
                Cancel
              </EuiButtonEmpty>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiFormRow>
      </>
    );
  } else {
    const address = mode.kind === 'editing' ? mode.address : '';
    const trimmed = address.trim();
    const looksLikeEmail = /.+@.+\..+/.test(trimmed);
    content = (
      <>
        <EuiFormRow
          label={'Notification email'}
          helpText={
            'Where Secutils sends product notifications such as tracker change emails. Leave empty to use your login email.'
          }
          fullWidth
        >
          <EuiFieldText
            type={'email'}
            placeholder={uiState.user?.email}
            value={address}
            onChange={(e: ChangeEvent<HTMLInputElement>) => setMode({ kind: 'editing', address: e.target.value })}
          />
        </EuiFormRow>
        <EuiFormRow fullWidth>
          <EuiFlexGroup gutterSize={'s'} responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiButton
                size={'s'}
                fill
                isDisabled={inFlight || !looksLikeEmail}
                isLoading={inFlight}
                onClick={() => onSendCode(trimmed)}
              >
                Send verification code
              </EuiButton>
            </EuiFlexItem>
            {record ? (
              <EuiFlexItem grow={false}>
                <EuiButtonEmpty size={'s'} onClick={() => setMode({ kind: 'idle' })} isDisabled={inFlight}>
                  Cancel
                </EuiButtonEmpty>
              </EuiFlexItem>
            ) : null}
          </EuiFlexGroup>
        </EuiFormRow>
      </>
    );
  }

  return (
    <EuiDescribedFormGroup
      title={<h3>Notification email</h3>}
      description={
        <>
          A separate, verified address for product notifications such as tracker change emails. Account activation,
          password recovery, and other security messages always go to your login email and are not affected by this
          setting.
        </>
      }
    >
      {content}
    </EuiDescribedFormGroup>
  );
}
