import { EuiIcon, EuiLink } from '@elastic/eui';
import { useCallback } from 'react';

import { PageState, type PageStateProps } from './page_state';

export type PageErrorStateProps = Omit<PageStateProps, 'color' | 'icon'>;

export function PageErrorState({ title, content, action }: PageErrorStateProps) {
  const onPageRefresh = useCallback(() => {
    window.location.reload();
  }, []);

  const actionNode = action ? (
    action
  ) : (
    <p>
      <EuiLink onClick={onPageRefresh}>Refresh the page</EuiLink>
    </p>
  );

  return (
    <PageState
      title={title}
      content={content}
      action={actionNode}
      color={'danger'}
      icon={<EuiIcon type={'warning'} color={'danger'} size={'xl'} />}
    />
  );
}
