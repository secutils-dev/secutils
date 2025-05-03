import type { Criteria, Pagination, PropertySort } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiInMemoryTable,
  EuiLink,
  EuiSpacer,
  EuiText,
  EuiToolTip,
} from '@elastic/eui';
import axios from 'axios';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { PRIVATE_KEYS_PROD_WARNING_USER_SETTINGS_KEY } from './consts';
import type { PrivateKey } from './private_key';
import { privateKeyAlgString } from './private_key_alg';
import { PrivateKeyExportModal } from './private_key_export_modal';
import { SavePrivateKeyFlyout } from './save_private_key_flyout';
import { PageErrorState, PageLoadingState } from '../../../../components';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../hooks';

export default function CertificatesPrivateKeys() {
  const { uiState, setTitleActions, setSettings, settings } = useWorkspaceContext();

  const [privateKeys, setPrivateKeys] = useState<AsyncData<PrivateKey[]>>({ status: 'pending' });

  const [privateKeyToRemove, setPrivateKeyToRemove] = useState<PrivateKey | null>(null);
  const [privateKeyToExport, setPrivateKeyToExport] = useState<PrivateKey | null>(null);
  const [privateKeyToEdit, setPrivateKeyToEdit] = useState<PrivateKey | null | undefined>(null);

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusInCircle'}
        title="Create a new private key"
        fill
        onClick={() => setPrivateKeyToEdit(undefined)}
      >
        Create private key
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to create and use private keys"
      target={'_blank'}
      href={'/docs/guides/digital_certificates/private_keys'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const loadPrivateKeys = useCallback(() => {
    axios.get<PrivateKey[]>(getApiUrl('/api/utils/certificates/private_keys'), getApiRequestConfig()).then(
      (response) => {
        setPrivateKeys({ status: 'succeeded', data: response.data });
        setTitleActions(response.data.length === 0 ? null : createButton);
      },
      (err: Error) => {
        setPrivateKeys({ status: 'failed', error: getErrorMessage(err) });
      },
    );
  }, [setTitleActions, createButton]);

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadPrivateKeys();
  }, [uiState, loadPrivateKeys]);

  const editFlyout =
    privateKeyToEdit !== null ? (
      <SavePrivateKeyFlyout
        onClose={(success) => {
          if (success) {
            loadPrivateKeys();
          }
          setPrivateKeyToEdit(null);
        }}
        privateKey={privateKeyToEdit}
      />
    ) : null;

  const generateModal = privateKeyToExport ? (
    <PrivateKeyExportModal onClose={() => setPrivateKeyToExport(null)} privateKey={privateKeyToExport} />
  ) : null;

  const removeConfirmModal = privateKeyToRemove ? (
    <EuiConfirmModal
      title={`Remove "${privateKeyToRemove.name}"?`}
      onCancel={() => setPrivateKeyToRemove(null)}
      onConfirm={() => {
        setPrivateKeyToRemove(null);

        axios
          .delete(
            getApiUrl(`/api/utils/certificates/private_keys/${encodeURIComponent(privateKeyToRemove?.id)}`),
            getApiRequestConfig(),
          )
          .then(
            () => loadPrivateKeys(),
            (err: Error) => {
              console.error(`Failed to remove private key: ${getErrorMessage(err)}`);
            },
          );
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The private key will be removed. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 15,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });
  const [sorting, setSorting] = useState<{ sort: PropertySort }>({ sort: { field: 'name', direction: 'asc' } });
  const onTableChange = useCallback(
    ({ page, sort }: Criteria<PrivateKey>) => {
      setPagination({
        ...pagination,
        pageIndex: page?.index ?? 0,
        pageSize: page?.size ?? 15,
      });

      if (sort?.field) {
        setSorting({ sort });
      }
    },
    [pagination],
  );

  if (privateKeys.status === 'pending') {
    return <PageLoadingState />;
  }

  if (privateKeys.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load private keys"
        content={
          <p>
            Cannot load private keys
            <br />
            <br />
            <strong>{privateKeys.error}</strong>.
          </p>
        }
      />
    );
  }

  let content;
  if (privateKeys.data.length === 0) {
    content = (
      <EuiFlexGroup
        direction={'column'}
        gutterSize={'s'}
        justifyContent="center"
        alignItems="center"
        style={{ height: '100%' }}
      >
        <EuiFlexItem>
          <EuiEmptyPrompt
            icon={<EuiIcon type={'securityApp'} size={'xl'} />}
            title={<h2>You don&apos;t have any private keys yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and create your first private key.</p>
                {createButton}
                <EuiSpacer size={'s'} />
                {docsButton}
              </div>
            }
          />
        </EuiFlexItem>
      </EuiFlexGroup>
    );
  } else {
    const privateKeysProdWarning =
      settings?.[PRIVATE_KEYS_PROD_WARNING_USER_SETTINGS_KEY] === true ? null : (
        <div>
          <EuiCallOut
            title="Don't use generated private keys in production environments"
            color="warning"
            iconType="warning"
          >
            <p>
              While private keys generated through Secutils.dev are encrypted at rest, they are intended for use in
              development and testing environments only. Please do not use these private keys in production environments
              unless you are running{' '}
              <EuiLink target="_blank" href="https://github.com/secutils-dev/secutils">
                your own version
              </EuiLink>{' '}
              of the Secutils.dev in a trusted and controlled environment.
            </p>
            <EuiButton
              color="accent"
              onClick={() => setSettings({ [PRIVATE_KEYS_PROD_WARNING_USER_SETTINGS_KEY]: true })}
            >
              Do not show again
            </EuiButton>
          </EuiCallOut>
          <EuiSpacer />
        </div>
      );

    content = (
      <>
        {privateKeysProdWarning}
        <EuiInMemoryTable
          pagination={pagination}
          allowNeutralSort={false}
          sorting={sorting}
          onTableChange={onTableChange}
          items={privateKeys.data}
          itemId={(item) => item.id}
          tableLayout={'auto'}
          columns={[
            {
              name: (
                <EuiToolTip content="A unique name of the private key">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              textOnly: true,
              sortable: true,
              render: (_, privateKey: PrivateKey) => privateKey.name,
            },
            {
              name: (
                <EuiToolTip content="Algorithm used to generate a private key.">
                  <span>
                    Type <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'alg',
              textOnly: true,
              sortable: true,
              width: '400px',
              mobileOptions: { width: 'unset' },
              render: (_, privateKey: PrivateKey) => privateKeyAlgString(privateKey.alg),
            },
            {
              name: (
                <EuiToolTip content="Indicates whether the private key is encrypted with a passphrase or not.">
                  <span>
                    Encryption <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'encrypted',
              textOnly: true,
              sortable: true,
              width: '110px',
              render: (_, privateKey: PrivateKey) => (
                <EuiText size={'s'} color={privateKey.encrypted ? 'success' : 'danger'}>
                  {privateKey.encrypted ? 'Passphrase' : <b>None</b>}
                </EuiText>
              ),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: (privateKey) => privateKey.updatedAt,
              render: (_, privateKey: PrivateKey) => <TimestampTableCell timestamp={privateKey.updatedAt} />,
            },
            {
              name: 'Actions',
              field: 'headers',
              width: '105px',
              actions: [
                {
                  name: 'Export',
                  description: 'Export private key',
                  icon: 'download',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setPrivateKeyToExport,
                },
                {
                  name: 'Edit',
                  description: 'Edit private key details',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setPrivateKeyToEdit,
                },
                {
                  name: 'Remove',
                  description: 'Remove private key',
                  icon: 'minusInCircle',
                  type: 'icon',
                  onClick: setPrivateKeyToRemove,
                },
              ],
            },
          ]}
        />
      </>
    );
  }

  return (
    <>
      {content}
      {editFlyout}
      {generateModal}
      {removeConfirmModal}
    </>
  );
}
