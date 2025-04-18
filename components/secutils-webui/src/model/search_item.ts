export interface SerializedSearchItem {
  l: string;
  c: string;
  s?: string;
  m?: Record<string, string>;
  t: number;
}

export interface SearchItem {
  label: string;
  category: string;
  subCategory?: string;
  meta?: Record<string, string>;
  timestamp: number;
}
export function deserializeSearchItem(searchItem: SerializedSearchItem): SearchItem {
  return {
    label: searchItem.l,
    category: searchItem.c,
    subCategory: searchItem.s,
    meta: searchItem.m,
    timestamp: searchItem.t,
  };
}
