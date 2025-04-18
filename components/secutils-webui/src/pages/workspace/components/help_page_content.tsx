import type { ReactNode } from 'react';

import { EuiFlexGroup } from '@elastic/eui';
import { css } from '@emotion/react';

import { useFontSizes } from '../hooks';

export interface Props {
  children: ReactNode;
}

export default function HelpPageContent({ children }: Props) {
  const fontSizes = useFontSizes();
  const pageStyle = css`
    ${fontSizes.text}
    width: 100%;
    padding: 1% 5% 0;
  `;
  return (
    <EuiFlexGroup direction={'column'} css={pageStyle}>
      {children}
    </EuiFlexGroup>
  );
}
