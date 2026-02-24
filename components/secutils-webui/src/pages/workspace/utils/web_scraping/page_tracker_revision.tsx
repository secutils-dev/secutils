import { EuiMarkdownFormat } from '@elastic/eui';

import { containsHTMLTags, detectLanguage, revisionDataToString } from './revision_utils';
import { PageTrackerRevisionCodeView } from './revision_views/page_tracker_revision_code_view';
import { PageTrackerRevisionDiffView } from './revision_views/page_tracker_revision_diff_view';
import {
  isPageTrackerRevisionTableViewData,
  PageTrackerRevisionTableView,
} from './revision_views/page_tracker_revision_table_view';
import type { TrackerDataRevision } from './tracker_data_revision';
import type { TrackerRevisionsViewMode } from './tracker_revisions';

export interface PageTrackerRevisionProps {
  revision: TrackerDataRevision;
  mode: TrackerRevisionsViewMode;
  previousRevision?: TrackerDataRevision;
}

export function PageTrackerRevision({ revision, mode, previousRevision }: PageTrackerRevisionProps) {
  const data = revision.data.original;
  if (mode !== 'source' && isPageTrackerRevisionTableViewData(data)) {
    return <PageTrackerRevisionTableView mode={mode} data={data} />;
  }

  if (mode === 'diff' && previousRevision) {
    const modifiedContent = revisionDataToString(data);
    const originalContent = revisionDataToString(previousRevision.data.original);
    const language = typeof data === 'string' ? detectLanguage(data) : 'json';

    return (
      <PageTrackerRevisionDiffView
        originalContent={originalContent}
        modifiedContent={modifiedContent}
        language={language}
      />
    );
  }

  if (typeof data !== 'string') {
    return <PageTrackerRevisionCodeView data={JSON.stringify(data, null, 2)} language={'json'} />;
  }

  const codeLanguage = mode === 'source' ? 'text' : containsHTMLTags(data) ? ('html' as const) : null;
  if (codeLanguage) {
    return <PageTrackerRevisionCodeView data={data} language={codeLanguage} />;
  }

  return <EuiMarkdownFormat textSize={'s'}>{data}</EuiMarkdownFormat>;
}
