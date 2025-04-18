import { EuiEmptyPrompt, EuiFlexGroup, EuiFlexItem, EuiLoadingLogo } from '@elastic/eui';

import { Logo } from './logo';

export interface PageLoadingStateProps {
  title?: string;
}

export function PageLoadingState({ title = 'Loading…' }: PageLoadingStateProps) {
  return (
    <EuiFlexGroup
      direction={'column'}
      gutterSize={'s'}
      style={{ height: '100%' }}
      alignItems="center"
      justifyContent="center"
    >
      <EuiFlexItem grow={false}>
        <EuiEmptyPrompt
          icon={<EuiLoadingLogo logo={() => <Logo size={40} />} size="l" />}
          titleSize="xs"
          title={<h2>{title}</h2>}
        />
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
