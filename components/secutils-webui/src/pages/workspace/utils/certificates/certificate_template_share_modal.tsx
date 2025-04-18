import { useCallback, useEffect, useState } from 'react';

import type { EuiSwitchEvent } from '@elastic/eui';
import {
  EuiButtonEmpty,
  EuiCallOut,
  EuiCopy,
  EuiFlexGroup,
  EuiFlexItem,
  EuiForm,
  EuiFormRow,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSwitch,
  EuiTitle,
} from '@elastic/eui';
import axios from 'axios';

import type { CertificateTemplate } from './certificate_template';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage, USER_SHARE_ID_HEADER_NAME } from '../../../../model';
import type { UserShare } from '../../../../model/user_share';
import { useWorkspaceContext } from '../../hooks';

export interface CertificateTemplateShareModalProps {
  template: CertificateTemplate;
  onClose: () => void;
}

type GetCertificateTemplateResponse = { userShare?: UserShare };

export function CertificateTemplateShareModal({ template, onClose }: CertificateTemplateShareModalProps) {
  const { uiState } = useWorkspaceContext();

  const [isTemplateShared, setIsTemplateShared] = useState<boolean>(false);
  const onIsTemplateSharedChange = useCallback((e: EuiSwitchEvent) => {
    setIsTemplateShared(e.target.checked);
    onShareToggle(e.target.checked);
  }, []);

  const [userShare, setUserShare] = useState<AsyncData<UserShare | null>>({ status: 'pending' });

  const onShareToggle = useCallback(
    (share: boolean) => {
      if (userShare.state === 'pending') {
        return;
      }

      setUserShare({ status: 'pending' });

      axios
        .post<UserShare | null>(
          getApiUrl(
            `/api/utils/certificates/templates/${encodeURIComponent(template.id)}/${share ? 'share' : 'unshare'}`,
          ),
          getApiRequestConfig(),
        )
        .then(
          (res) => {
            setUserShare({ status: 'succeeded', data: share ? (res.data ?? null) : null });
          },
          (err: Error) => {
            setUserShare({ status: 'failed', error: getErrorMessage(err) });
          },
        );
    },
    [template, userShare],
  );

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    axios
      .get<GetCertificateTemplateResponse>(
        getApiUrl(`/api/utils/certificates/templates/${encodeURIComponent(template.id)}`),
        getApiRequestConfig(),
      )
      .then(
        (res) => {
          const userShare = res.data.userShare ?? null;
          setUserShare({ status: 'succeeded', data: userShare });
          setIsTemplateShared(!!userShare);
        },
        (err: Error) => {
          setUserShare({ status: 'failed', error: getErrorMessage(err) });
        },
      );
  }, [uiState, template]);

  const statusCallout =
    userShare?.status === 'failed' ? (
      <EuiFormRow>
        <EuiCallOut
          size="s"
          title={userShare.error ?? 'An error occurred, please try again later'}
          color="danger"
          iconType="warning"
        />
      </EuiFormRow>
    ) : undefined;
  return (
    <EuiModal onClose={onClose}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size={'s'}>
            <span>{`Share "${template.name}" template`}</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm id="share-form" component="form">
          {statusCallout}
          <EuiFormRow
            helpText={'Anyone on the internet with the link can view the template'}
            isDisabled={userShare.status === 'pending'}
          >
            <EuiSwitch label="Share template" checked={isTemplateShared} onChange={onIsTemplateSharedChange} />
          </EuiFormRow>
        </EuiForm>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiFlexGroup responsive={!isTemplateShared} justifyContent={'flexEnd'}>
          {userShare.status === 'succeeded' && userShare.data?.id ? (
            <EuiFlexItem>
              <EuiCopy
                textToCopy={`${location.origin}${location.pathname}?${USER_SHARE_ID_HEADER_NAME}=${encodeURIComponent(
                  userShare.data.id,
                )}`}
              >
                {(copy) => (
                  <EuiButtonEmpty iconType={'link'} onClick={copy}>
                    Copy link
                  </EuiButtonEmpty>
                )}
              </EuiCopy>
            </EuiFlexItem>
          ) : null}
          <EuiFlexItem grow={false}>
            <EuiButtonEmpty onClick={onClose}>Close</EuiButtonEmpty>
          </EuiFlexItem>
        </EuiFlexGroup>
      </EuiModalFooter>
    </EuiModal>
  );
}
