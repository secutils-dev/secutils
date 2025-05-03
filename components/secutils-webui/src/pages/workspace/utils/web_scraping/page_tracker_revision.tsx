import { EuiMarkdownFormat } from '@elastic/eui';

import { PageTrackerRevisionCodeView } from './revision_views/page_tracker_revision_code_view';
import {
  isPageTrackerRevisionTableViewData,
  PageTrackerRevisionTableView,
} from './revision_views/page_tracker_revision_table_view';
import type { TrackerDataRevision } from './tracker_data_revision';
import type { TrackerRevisionsViewMode } from './tracker_revisions';

export interface PageTrackerRevisionProps {
  revision: TrackerDataRevision;
  mode: TrackerRevisionsViewMode;
}

function containHTMLTags(data: string) {
  if (!data) {
    return false;
  }

  try {
    const doc = new DOMParser().parseFromString(data, 'text/html');
    return Array.from(doc.body.childNodes).some((node) => node.nodeType === Node.ELEMENT_NODE);
  } catch {
    return false;
  }
}

export function PageTrackerRevision({ revision, mode }: PageTrackerRevisionProps) {
  const data = revision.data.original;
  if (mode !== 'source' && isPageTrackerRevisionTableViewData(data)) {
    return <PageTrackerRevisionTableView mode={mode} data={data} />;
  }

  if (typeof data !== 'string') {
    return <PageTrackerRevisionCodeView data={JSON.stringify(data, null, 2)} language={'json'} />;
  }

  const codeLanguage =
    mode == 'source'
      ? 'text'
      : mode === 'diff' && data.startsWith('@@')
        ? 'diff'
        : containHTMLTags(data)
          ? 'html'
          : null;
  if (codeLanguage) {
    return <PageTrackerRevisionCodeView data={data} language={codeLanguage} />;
  }

  return <EuiMarkdownFormat textSize={'s'}>{data}</EuiMarkdownFormat>;
}
