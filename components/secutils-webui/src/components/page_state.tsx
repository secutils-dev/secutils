import type { BACKGROUND_COLORS } from '@elastic/eui';
import { EuiEmptyPrompt, EuiFlexGroup, EuiFlexItem } from '@elastic/eui';
import type { ReactNode } from 'react';

export interface PageStateProps {
  title: string;
  color?: (typeof BACKGROUND_COLORS)[number];
  icon?: ReactNode;
  content?: ReactNode;
  action?: ReactNode;
}

export function PageState({ title, content = null, action, color, icon }: PageStateProps) {
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
          icon={icon}
          color={color}
          title={<h2>{title}</h2>}
          titleSize="s"
          body={
            <div>
              {content}
              {action}
            </div>
          }
        />
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
