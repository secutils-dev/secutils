import { EuiCodeBlock } from '@elastic/eui';

export interface PageTrackerRevisionCodeViewProps {
  data: string;
  language: 'json' | 'html' | 'diff' | 'text';
}

export function PageTrackerRevisionCodeView({ data, language }: PageTrackerRevisionCodeViewProps) {
  return (
    <EuiCodeBlock fontSize="m" transparentBackground isCopyable paddingSize={'s'} language={language}>
      {data}
    </EuiCodeBlock>
  );
}
