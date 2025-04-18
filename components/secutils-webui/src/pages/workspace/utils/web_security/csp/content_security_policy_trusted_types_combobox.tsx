import { EuiComboBox } from '@elastic/eui';
import { useState } from 'react';

export interface ContentSecurityPolicySourcesComboboxProps {
  value?: string[];
  isDisabled?: boolean;
  onChange(value: string[]): void;
}

const NONE_KEYWORD = "'none'";
const KEYWORDS = [{ label: 'default' }, { label: "'allow-duplicates'" }, { label: NONE_KEYWORD }];

const isPolicyValid = (policy: string) => {
  if (policy.includes(',')) {
    return false;
  }

  // Prevent users from manually typing keyword sources without quotes.
  const quotedPolicy = `'${policy.toLowerCase()}'`;
  return KEYWORDS.findIndex((keywordPolicy) => keywordPolicy.label === quotedPolicy) < 0;
};

export function ContentSecurityPolicyTrustedTypesCombobox({
  onChange,
  value,
  isDisabled,
}: ContentSecurityPolicySourcesComboboxProps) {
  const [selectedPolicies, setSelectedPolicies] = useState<Array<{ label: string }>>(
    value ? (value.length > 0 ? value.map((policy) => ({ label: policy })) : [{ label: NONE_KEYWORD }]) : [],
  );

  const onCreatePolicyName = (headerValue: string) => {
    if (!isPolicyValid(headerValue)) {
      return false;
    }

    onPoliciesChange([...selectedPolicies, { label: headerValue }]);
  };

  const onPoliciesChange = (selectedPolicies: Array<{ label: string }>) => {
    const sanitizedSelectedSources =
      selectedPolicies.length > 0 && selectedPolicies[selectedPolicies.length - 1].label === NONE_KEYWORD
        ? [{ label: NONE_KEYWORD }]
        : selectedPolicies.filter((source) => source.label !== NONE_KEYWORD);

    setSelectedPolicies(sanitizedSelectedSources);
    onChange(sanitizedSelectedSources.map(({ label }) => label));
  };

  return (
    <EuiComboBox
      fullWidth
      isDisabled={isDisabled}
      isCaseSensitive={false}
      aria-label="Configure policies"
      placeholder="Configure policies"
      selectedOptions={selectedPolicies}
      onCreateOption={onCreatePolicyName}
      options={KEYWORDS}
      onChange={onPoliciesChange}
      isClearable
    />
  );
}
