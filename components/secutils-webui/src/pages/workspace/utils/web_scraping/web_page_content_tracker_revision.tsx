import { EuiCodeBlock, EuiMarkdownFormat } from '@elastic/eui';

import type { WebPageContentRevision } from './web_page_data_revision';
import type { WebPageTrackerHistoryMode } from './web_page_tracker_history';

export interface WebPageContentTrackerRevisionProps {
  revision: WebPageContentRevision;
  mode: WebPageTrackerHistoryMode;
}

function containHTMLTags(data: string) {
  try {
    const doc = new DOMParser().parseFromString(data, 'text/html');
    return Array.from(doc.body.childNodes).some((node) => node.nodeType === Node.ELEMENT_NODE);
  } catch {
    return false;
  }
}

export function WebPageContentTrackerRevision({ revision, mode }: WebPageContentTrackerRevisionProps) {
  let dataToRender;
  let codeBlockType = null;
  try {
    dataToRender = JSON.parse(revision.data) as string | object;
    if (typeof dataToRender === 'object' && dataToRender) {
      dataToRender = JSON.stringify(dataToRender, null, 2);
      codeBlockType = 'json';
    } else if (typeof dataToRender !== 'string') {
      dataToRender = JSON.stringify(dataToRender, null, 2);
    } else if (containHTMLTags(dataToRender)) {
      codeBlockType = 'html';
    }
  } catch {
    dataToRender = revision.data;
  }

  if (mode === 'source' || (mode === 'default' && codeBlockType)) {
    return (
      <EuiCodeBlock
        fontSize="m"
        transparentBackground
        isCopyable
        paddingSize={'none'}
        language={codeBlockType ?? undefined}
      >
        {dataToRender}
      </EuiCodeBlock>
    );
  }

  if (mode === 'diff' && dataToRender.startsWith('@@')) {
    return (
      <EuiCodeBlock fontSize="m" transparentBackground isCopyable paddingSize={'s'} language={'diff'}>
        {dataToRender}
      </EuiCodeBlock>
    );
  }

  return <EuiMarkdownFormat textSize={'s'}>{dataToRender}</EuiMarkdownFormat>;
}
