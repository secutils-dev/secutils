import { EuiIcon } from '@elastic/eui';

import { PageState, type PageStateProps } from './page_state';

export type PageSuccessStateProps = Omit<PageStateProps, 'color' | 'icon'>;

export function PageSuccessState({ title, content, action }: PageSuccessStateProps) {
  return (
    <PageState
      title={title}
      content={content}
      action={action}
      color={'success'}
      icon={<EuiIcon type={'check'} color={'success'} size={'xl'} />}
    />
  );
}
