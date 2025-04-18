import { useState } from 'react';

import { EuiCheckbox, EuiComboBox, EuiSpacer } from '@elastic/eui';

export interface ContentSecurityPolicySandboxComboboxProps {
  value?: string[];
  isDisabled?: boolean;
  onChange(value: string[], isSandboxEnforced: boolean): void;
}

export function ContentSecurityPolicySandboxCombobox({
  onChange,
  value,
  isDisabled,
}: ContentSecurityPolicySandboxComboboxProps) {
  const [restrictionsToLift, setRestrictionsToLift] = useState<Array<{ label: string }>>(
    value?.map((restrictionToLift) => ({ label: restrictionToLift })) ?? [],
  );
  const [enforceSandbox, setEnforceSandbox] = useState(!!value && value.length === 0);

  const onLiftedRestrictionsChange = (
    selectedRestrictionsToLift: Array<{ label: string }>,
    currentEnforceSandbox?: boolean,
  ) => {
    setRestrictionsToLift(selectedRestrictionsToLift);
    onChange(
      selectedRestrictionsToLift.map(({ label }) => label),
      currentEnforceSandbox ?? enforceSandbox,
    );
  };

  const placeholder = enforceSandbox
    ? 'All sandbox restrictions are enforced'
    : 'Select what sandbox restrictions to lift';

  return (
    <>
      <EuiComboBox
        fullWidth
        aria-label={placeholder}
        placeholder={placeholder}
        isDisabled={enforceSandbox || isDisabled}
        selectedOptions={enforceSandbox ? [] : restrictionsToLift}
        options={[
          { label: 'allow-downloads' },
          { label: 'allow-forms' },
          { label: 'allow-modals' },
          { label: 'allow-orientation-lock' },
          { label: 'allow-pointer-lock' },
          { label: 'allow-popups' },
          { label: 'allow-popups-to-escape-sandbox' },
          { label: 'allow-presentation' },
          { label: 'allow-same-origin' },
          { label: 'allow-scripts' },
          { label: 'allow-top-navigation' },
          { label: 'allow-top-navigation-by-user-activation' },
          { label: 'allow-top-navigation-to-custom-protocols' },
        ]}
        onChange={onLiftedRestrictionsChange}
        isClearable
      />
      {!isDisabled ? (
        <>
          <EuiSpacer size="xs" />
          <EuiCheckbox
            id={'sandbox-enforce'}
            label="Enforce all restrictions"
            checked={enforceSandbox}
            disabled={isDisabled}
            onChange={(e) => {
              setEnforceSandbox(e.target.checked);
              onLiftedRestrictionsChange([], e.target.checked);
            }}
          />
        </>
      ) : null}
    </>
  );
}
