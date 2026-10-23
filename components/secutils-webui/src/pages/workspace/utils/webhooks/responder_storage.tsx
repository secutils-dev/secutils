import {
  EuiButtonIcon,
  EuiCallOut,
  EuiConfirmModal,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormRow,
  EuiPanel,
  EuiSpacer,
  EuiText,
  EuiToolTip,
} from '@elastic/eui';
import { unix } from 'moment';
import { useCallback, useEffect, useState } from 'react';

import type { ResponderStorageStats, ResponderSummary } from './responder';
import { clearResponderStorage, getResponderStorage } from './responder';
import { type AsyncData, formatBytes, getErrorMessage } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface ResponderStorageProps {
  responder: ResponderSummary;
}

function percentOf(value: number, max: number) {
  if (max <= 0) {
    return '-';
  }
  const percent = Math.round((value / max) * 1000) / 10;
  return value > 0 && percent === 0 ? '<0.1%' : `${percent}%`;
}

export function ResponderStorage({ responder }: ResponderStorageProps) {
  const { addToast } = useWorkspaceContext();

  const [stats, setStats] = useState<AsyncData<ResponderStorageStats>>({ status: 'pending' });
  const [clearStatus, setClearStatus] = useState<{ isModalVisible: boolean; isInProgress: boolean; prefix: string }>({
    isModalVisible: false,
    isInProgress: false,
    prefix: '',
  });

  const loadStats = useCallback(() => {
    getResponderStorage(responder.id).then(
      (data) => setStats({ status: 'succeeded', data }),
      (err: Error) => setStats({ status: 'failed', error: getErrorMessage(err) }),
    );
  }, [responder.id]);

  useEffect(() => loadStats(), [loadStats]);

  let summary;
  if (stats.status === 'pending') {
    summary = 'Loading storage usage…';
  } else if (stats.status === 'failed') {
    summary = `Cannot load storage usage: ${stats.error}`;
  } else {
    const { entries, totalBytes, maxEntries, maxTotalBytes, oldestCreatedAt, nextExpiresAt } = stats.data;
    summary = [
      `${entries.toLocaleString()} of ${maxEntries.toLocaleString()} entries (${percentOf(entries, maxEntries)})`,
      `${formatBytes(totalBytes)} of ${formatBytes(maxTotalBytes)} (${percentOf(totalBytes, maxTotalBytes)})`,
      ...(oldestCreatedAt ? [`oldest ${unix(oldestCreatedAt).fromNow()}`] : []),
      ...(nextExpiresAt ? [`next expiry ${unix(nextExpiresAt).fromNow()}`] : []),
    ].join(' · ');
  }

  const clearConfirmModal = clearStatus.isModalVisible ? (
    <EuiConfirmModal
      aria-label="Clear responder storage"
      title="Clear responder storage?"
      onCancel={() => setClearStatus({ isModalVisible: false, isInProgress: false, prefix: '' })}
      isLoading={clearStatus.isInProgress}
      onConfirm={() => {
        setClearStatus((current) => ({ ...current, isInProgress: true }));
        const prefix = clearStatus.prefix || undefined;
        clearResponderStorage(responder.id, prefix).then(
          (removed) => {
            addToast({
              id: `success-clear-responder-storage-${responder.name}`,
              iconType: 'check',
              color: 'success',
              title: `Removed ${removed.toLocaleString()} storage entries from "${responder.name}" responder`,
            });
            setClearStatus({ isModalVisible: false, isInProgress: false, prefix: '' });
            loadStats();
          },
          (err: Error) => {
            addToast({
              id: `failed-clear-responder-storage-${responder.name}`,
              iconType: 'warning',
              color: 'danger',
              title: `Unable to clear storage for "${responder.name}" responder: ${getErrorMessage(err)}`,
            });
            setClearStatus((current) => ({ ...current, isInProgress: false }));
          },
        );
      }}
      cancelButtonText="Cancel"
      confirmButtonText={clearStatus.prefix ? 'Clear matching entries' : 'Clear all entries'}
      buttonColor="danger"
    >
      <p>
        The script storage entries of <b>{responder.name}</b> will be permanently removed. Leave the prefix empty to
        remove all entries.
      </p>
      <EuiFormRow label="Key prefix" helpText="Only entries whose key starts with this prefix are removed.">
        <EuiFieldText
          placeholder="e.g. req/"
          value={clearStatus.prefix}
          onChange={(e) => setClearStatus((current) => ({ ...current, prefix: e.target.value }))}
        />
      </EuiFormRow>
      {clearStatus.prefix ? (
        <>
          <EuiSpacer size="m" />
          <EuiCallOut
            announceOnMount
            size="s"
            color="warning"
            iconType="warning"
            title="Scripts often keep related entries under different prefixes. Removing only some of them can leave the script's data inconsistent until the remaining entries expire."
          />
        </>
      ) : null}
    </EuiConfirmModal>
  ) : null;

  return (
    <EuiPanel hasShadow={false} hasBorder={true} data-test-subj="responder-storage">
      <EuiFlexGroup alignItems="center" justifyContent="spaceBetween" responsive={false} gutterSize="s">
        <EuiFlexItem>
          <EuiText size="s">
            <b>Script storage:</b> {summary}
          </EuiText>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiFlexGroup alignItems="center" gutterSize="s" responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiToolTip content="Update storage usage" disableScreenReaderOutput>
                <EuiButtonIcon iconType="refresh" aria-label="Update storage usage" onClick={loadStats} />
              </EuiToolTip>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiToolTip content="Clear storage" disableScreenReaderOutput>
                <EuiButtonIcon
                  iconType="trash"
                  color="danger"
                  aria-label="Clear storage"
                  isDisabled={stats.status !== 'succeeded' || stats.data.entries === 0}
                  onClick={() => setClearStatus({ isModalVisible: true, isInProgress: false, prefix: '' })}
                />
              </EuiToolTip>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiFlexItem>
      </EuiFlexGroup>
      {clearConfirmModal}
    </EuiPanel>
  );
}
