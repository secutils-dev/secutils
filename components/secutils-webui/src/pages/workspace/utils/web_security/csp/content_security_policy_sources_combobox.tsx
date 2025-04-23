import { EuiComboBox } from '@elastic/eui';
import { useMemo, useState } from 'react';

export interface ContentSecurityPolicySourcesComboboxProps {
  value?: string[];
  isDisabled?: boolean;
  onChange(value: string[]): void;
  omitKeywordSources?: string[];
}

const NONE_KEYWORD_SOURCE = "'none'";

const KEYWORD_SOURCES = [
  { name: "'self'", safe: true },
  { name: NONE_KEYWORD_SOURCE, safe: true },
  { name: "'strict-dynamic'", safe: true },
  { name: "'unsafe-inline'", safe: false },
  { name: "'unsafe-eval'", safe: false },
  { name: "'wasm-unsafe-eval'", safe: false },
  { name: "'unsafe-hashes'", safe: false },
  { name: "'unsafe-allow-redirects'", safe: false },
  { name: "'report-sample'", safe: true },
];

const isSourceValid = (source: string) => {
  if (source.includes(',')) {
    return false;
  }

  // Prevent users from manually typing keyword sources without quotes.
  const quotedSource = `'${source.toLowerCase()}'`;
  return KEYWORD_SOURCES.findIndex((keywordSource) => keywordSource.name === quotedSource) < 0;
};

const isSourceSafe = (source: string) => {
  const keywordSource = KEYWORD_SOURCES.find(({ name }) => name === source);
  if (keywordSource) {
    return keywordSource.safe;
  }

  const lowerCaseSource = source.toLowerCase();
  return lowerCaseSource !== '*';
};

export function ContentSecurityPolicySourcesCombobox({
  onChange,
  value,
  isDisabled,
  omitKeywordSources,
}: ContentSecurityPolicySourcesComboboxProps) {
  const knownSources = useMemo(() => {
    const keywordSourcesToPermit =
      omitKeywordSources && omitKeywordSources.length > 0
        ? KEYWORD_SOURCES.filter((keywordSource) => !omitKeywordSources.includes(keywordSource.name))
        : KEYWORD_SOURCES;
    return keywordSourcesToPermit.map(({ name, safe }) => ({ label: name, color: !safe ? 'red' : undefined }));
  }, [omitKeywordSources]);

  const [selectedSources, setSelectedSources] = useState<Array<{ label: string; color?: string }>>(
    value?.map((source) => ({ label: source, color: isSourceSafe(source) ? undefined : 'red' })) ?? [],
  );

  const onCreateSource = (headerValue: string) => {
    if (!isSourceValid(headerValue)) {
      return false;
    }

    onSourcesChange([...selectedSources, { label: headerValue, color: isSourceSafe(headerValue) ? undefined : 'red' }]);
  };

  const onSourcesChange = (selectedSources: Array<{ label: string }>) => {
    const sanitizedSelectedSources =
      selectedSources.length > 0 && selectedSources[selectedSources.length - 1].label === NONE_KEYWORD_SOURCE
        ? [{ label: NONE_KEYWORD_SOURCE }]
        : selectedSources.filter((source) => source.label !== NONE_KEYWORD_SOURCE);

    setSelectedSources(sanitizedSelectedSources);
    onChange(sanitizedSelectedSources.map(({ label }) => label));
  };

  return (
    <EuiComboBox
      fullWidth
      isDisabled={isDisabled}
      isCaseSensitive={false}
      aria-label="Select or create sources"
      placeholder="Select or create sources"
      selectedOptions={selectedSources}
      onCreateOption={onCreateSource}
      options={knownSources}
      onChange={onSourcesChange}
      isClearable
    />
  );
}
