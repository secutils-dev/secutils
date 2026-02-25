import {
  EuiButton,
  EuiButtonEmpty,
  EuiConfirmModal,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFlyout,
  EuiFlyoutBody,
  EuiFlyoutFooter,
  EuiFlyoutHeader,
  EuiTitle,
} from '@elastic/eui';
import type { ReactNode } from 'react';
import { useCallback, useLayoutEffect, useRef, useState } from 'react';

export interface Props {
  title: ReactNode;
  children: ReactNode;
  onClose: () => void;
  onSave: () => void;

  canSave?: boolean;
  saveInProgress?: boolean;
  hasChanges?: boolean;
}

export function EditorFlyout({ title, children, onSave, onClose, canSave, saveInProgress, hasChanges }: Props) {
  const [isDiscardConfirmVisible, setIsDiscardConfirmVisible] = useState(false);

  // Track hasChanges in a ref so handleClose always reads the latest value,
  // even if React hasn't re-rendered the parent yet after a state update.
  const hasChangesRef = useRef(hasChanges ?? false);
  useLayoutEffect(() => {
    hasChangesRef.current = hasChanges ?? false;
  }, [hasChanges]);

  const handleClose = useCallback(() => {
    if (hasChangesRef.current) {
      setIsDiscardConfirmVisible(true);
    } else {
      onClose();
    }
  }, [onClose]);

  return (
    <EuiFlyout size="l" maxWidth onClose={handleClose} ownFocus hideCloseButton>
      <EuiFlyoutHeader hasBorder>
        {typeof title === 'string' ? (
          <EuiTitle size="s">
            <h1>{title}</h1>
          </EuiTitle>
        ) : (
          title
        )}
      </EuiFlyoutHeader>
      <EuiFlyoutBody>{children}</EuiFlyoutBody>
      <EuiFlyoutFooter>
        <EuiFlexGroup justifyContent="spaceBetween">
          <EuiFlexItem grow={false}>
            <EuiButtonEmpty iconType="cross" onClick={handleClose} flush="left">
              Close
            </EuiButtonEmpty>
          </EuiFlexItem>
          <EuiFlexItem grow={false}>
            <EuiButton isLoading={saveInProgress === true} isDisabled={canSave === false} onClick={() => onSave()} fill>
              Save
            </EuiButton>
          </EuiFlexItem>
        </EuiFlexGroup>
      </EuiFlyoutFooter>
      {isDiscardConfirmVisible && (
        <EuiConfirmModal
          title="Discard unsaved changes?"
          onCancel={() => setIsDiscardConfirmVisible(false)}
          onConfirm={() => {
            setIsDiscardConfirmVisible(false);
            onClose();
          }}
          cancelButtonText="Keep editing"
          confirmButtonText="Discard"
          buttonColor="danger"
          defaultFocusedButton="cancel"
        >
          You have unsaved changes. Are you sure you want to discard them?
        </EuiConfirmModal>
      )}
    </EuiFlyout>
  );
}
