import type { ReactNode } from 'react';

import { EuiPageHeader, EuiPageSection, useEuiTheme } from '@elastic/eui';
import { css } from '@emotion/react';

export interface PageHeaderProps {
  title: ReactNode;
}

export function PageHeader({ title }: PageHeaderProps) {
  const theme = useEuiTheme();

  return (
    <EuiPageSection
      paddingSize={'none'}
      bottomBorder
      css={css`
        background-color: ${theme.euiTheme.colors.lightestShade};
      `}
    >
      <EuiPageHeader paddingSize={'s'} pageTitle={title} />
    </EuiPageSection>
  );
}
