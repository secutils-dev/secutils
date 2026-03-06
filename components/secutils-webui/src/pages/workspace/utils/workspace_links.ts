import { FILTER_PARAM_QUERY } from '../components/items_table_filter';

export function getWorkspaceUtilLink(utilHandle: string): string {
  return utilHandle === 'home' ? '/ws' : `/ws/${utilHandle}`;
}

export function getWorkspaceEntityLink(utilHandle: string, entityId: string): string {
  const queryParams = new URLSearchParams([[FILTER_PARAM_QUERY, entityId]]);
  return `${getWorkspaceUtilLink(utilHandle)}?${queryParams.toString()}`;
}

export function getWorkspaceEntityAbsoluteLink(utilHandle: string, entityId: string): string {
  const relativeEntityLink = getWorkspaceEntityLink(utilHandle, entityId);
  return typeof location !== 'undefined' ? new URL(relativeEntityLink, location.origin).toString() : relativeEntityLink;
}
