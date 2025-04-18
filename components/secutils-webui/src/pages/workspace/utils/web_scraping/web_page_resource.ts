export interface WebPageResource {
  url?: string;
  content?: WebPageResourceContent;
  diffStatus?: 'added' | 'removed' | 'changed';
}

export interface WebPageResourceContent {
  digest: string;
  size: number;
}
