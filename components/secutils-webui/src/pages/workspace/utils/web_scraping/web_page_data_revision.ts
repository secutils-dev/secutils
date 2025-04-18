import type { WebPageResource } from './web_page_resource';

export interface WebPageDataRevision<D = unknown> {
  id: string;
  data: D;
  createdAt: number;
}

export interface WebPageResourcesRevision
  extends WebPageDataRevision<{
    scripts?: WebPageResource[];
    styles?: WebPageResource[];
  }> {}

export interface WebPageContentRevision extends WebPageDataRevision<string> {}
