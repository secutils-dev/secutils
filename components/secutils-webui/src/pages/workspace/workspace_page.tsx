import type { EuiBreadcrumb, EuiSideNavItemType } from '@elastic/eui';
import { EuiFlexGroup, EuiFlexItem, EuiIcon, EuiSideNav, EuiSpacer } from '@elastic/eui';
import { css } from '@emotion/react';
import type { MouseEvent, ReactNode } from 'react';
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Navigate, useNavigate, useParams } from 'react-router';

import { SiteSearchBar } from './components/site_search_bar';
import { TagScopeSelector } from './components/tag_scope_selector';
import { getUtilIcon, UTIL_HANDLES, UtilsComponents, UtilsShareComponents } from './utils';
import { getWorkspaceUtilLink } from './utils/workspace_links';
import { WorkspaceContext } from './workspace_context';
import { PageLoadingState } from '../../components';
import { useAppContext, usePageHeaderActions, usePageMeta } from '../../hooks';
import type { Util } from '../../model';
import {
  parseSidebarCollapsed,
  USER_SETTINGS_KEY_COMMON_GLOBAL_SCOPE_TAG_IDS,
  USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED,
} from '../../model';
import { Page } from '../page';

/** Maps each util handle to its parent util handle (tree-based; not derived from handle string). */
function buildUtilParentMap(utils: Util[], parentHandle?: string): Map<string, string | undefined> {
  const map = new Map<string, string | undefined>();
  for (const u of utils) {
    map.set(u.handle, parentHandle);
    if (u.utils?.length) {
      buildUtilParentMap(u.utils, u.handle).forEach((v, k) => map.set(k, v));
    }
  }
  return map;
}

const DEFAULT_COMPONENT = lazy(() => import('../../components/page_under_construction_state'));
const SettingsFlyout = lazy(() => import('../../app_container/settings_flyout'));

export function WorkspacePage() {
  usePageMeta('Workspace');

  const navigate = useNavigate();

  const { actions, isSettingsOpen, hideSettings, pendingImportUrl, clearPendingImportUrl } = usePageHeaderActions();

  const { uiState, settings, setSettings } = useAppContext();
  const { util: utilIdFromParam = UTIL_HANDLES.workspaceOverview, deepLink: deepLinkFromParam } = useParams<{
    util?: string;
    deepLink?: string;
  }>();

  const globalScopeTagIds = useMemo(
    () => (settings?.[USER_SETTINGS_KEY_COMMON_GLOBAL_SCOPE_TAG_IDS] as string[] | undefined) ?? [],
    [settings],
  );
  const onGlobalScopeTagIdsChange = useCallback(
    (tagIds: string[]) => {
      setSettings({ [USER_SETTINGS_KEY_COMMON_GLOBAL_SCOPE_TAG_IDS]: tagIds.length > 0 ? tagIds : null });
    },
    [setSettings],
  );

  const getBreadcrumbs = useCallback(
    (util: Util, utilsMap: Map<string, Util>, utilParentMap: Map<string, string | undefined>, deepLink?: string) => {
      const breadcrumbs: EuiBreadcrumb[] = [];
      let utilToBreadcrumb: Util | undefined = util;
      while (utilToBreadcrumb) {
        const utilUrl = `/ws/${utilToBreadcrumb.handle}`;
        const shouldIncludeURL =
          (utilToBreadcrumb.handle !== util.handle || deepLink != null) && !utilToBreadcrumb.utils?.length;
        breadcrumbs.unshift({
          text: utilToBreadcrumb.name,
          onClick: shouldIncludeURL
            ? (e) => {
                e.preventDefault();
                navigate(utilUrl);
              }
            : undefined,
          href: shouldIncludeURL ? utilUrl : undefined,
        });

        const parentHandle = utilParentMap.get(utilToBreadcrumb.handle);
        utilToBreadcrumb = parentHandle ? utilsMap.get(parentHandle) : undefined;
      }

      return deepLink ? [...breadcrumbs, { text: deepLink }] : breadcrumbs;
    },
    [navigate],
  );

  const onSidebarClickCapture = useCallback(
    (e: MouseEvent<HTMLElement>) => {
      // Preserve native behavior for non-primary or modified clicks.
      if (e.defaultPrevented || e.button !== 0 || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) {
        return;
      }

      const targetElement = e.target as HTMLElement | null;
      const linkElement = targetElement?.closest('a[href^="/ws"]');
      if (!linkElement) {
        return;
      }

      const href = linkElement.getAttribute('href');
      if (!href) {
        return;
      }

      e.preventDefault();
      navigate(href);
    },
    [navigate],
  );

  const [titleActions, setTitleActions] = useState<ReactNode | null>(null);
  const [title, setTitle] = useState<string | null>(null);

  const [selectedUtil, setSelectedUtil] = useState<Util | null>(null);
  const [navigationBar, setNavigationBar] = useState<{ breadcrumbs: EuiBreadcrumb[]; deepLink?: string }>({
    breadcrumbs: [],
    deepLink: deepLinkFromParam,
  });

  // EuiSideNav root items (depth 0) are always open and have no toggle caret.
  // To make them collapsible, we manage their open/closed state manually and
  // conditionally pass `items` only when the section is expanded.
  // Collapsed section handles are persisted in user settings.
  const sidebarState = useMemo(
    () => parseSidebarCollapsed(settings?.[USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED]),
    [settings],
  );
  const collapsedSections = useMemo(() => new Set(sidebarState.sections), [sidebarState.sections]);
  const toggleSection = useCallback(
    (handle: string) => {
      const updated = collapsedSections.has(handle)
        ? sidebarState.sections.filter((h) => h !== handle)
        : [...sidebarState.sections, handle];
      setSettings({ [USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED]: { ...sidebarState, sections: updated } });
    },
    [collapsedSections, sidebarState, setSettings],
  );

  const [sideNavItems, utilsMap, utilParentMap] = useMemo(() => {
    const utilsMap = new Map<string, Util>();
    const createItem = (util: Util, isRoot = false): EuiSideNavItemType<unknown> => {
      utilsMap.set(util.handle, util);
      const utilUrl = getWorkspaceUtilLink(util.handle);
      const childUtils = util.utils ?? [];
      const childItems = childUtils.length > 0 ? childUtils.map((nestedUtil) => createItem(nestedUtil)) : undefined;

      if (isRoot && childItems) {
        const isOpen = !collapsedSections.has(util.handle);
        return {
          id: util.handle,
          name: util.name,
          onClick: () => toggleSection(util.handle),
          icon: <EuiIcon type={isOpen ? 'arrowDown' : 'arrowRight'} size="s" />,
          items: isOpen ? childItems : undefined,
          isSelected: false,
        };
      }

      const utilIcon = selectedUtil ? getUtilIcon(util.handle) : undefined;
      return {
        id: util.handle,
        name: util.name,
        href: childItems ? undefined : utilUrl,
        icon: utilIcon ? <EuiIcon type={utilIcon} /> : undefined,
        isSelected: selectedUtil?.handle === util.handle && !deepLinkFromParam,
        items: childItems,
        forceOpen: childItems ? false : undefined,
      };
    };

    return [uiState.utils.map((u) => createItem(u, true)), utilsMap, buildUtilParentMap(uiState.utils)];
  }, [uiState, selectedUtil, deepLinkFromParam, collapsedSections, toggleSection]);

  useEffect(() => {
    const newSelectedUtil =
      utilIdFromParam && utilIdFromParam !== selectedUtil?.handle
        ? (utilsMap.get(utilIdFromParam) ?? selectedUtil)
        : selectedUtil;
    if (newSelectedUtil && (newSelectedUtil !== selectedUtil || navigationBar.deepLink !== deepLinkFromParam)) {
      setSelectedUtil(newSelectedUtil);
      setTitle(newSelectedUtil.name);
      setNavigationBar({
        breadcrumbs: getBreadcrumbs(newSelectedUtil, utilsMap, utilParentMap, deepLinkFromParam),
        deepLink: deepLinkFromParam,
      });
      setTitleActions(null);
    }
  }, [utilIdFromParam, selectedUtil, utilsMap, utilParentMap, deepLinkFromParam, navigationBar, getBreadcrumbs]);

  // Auto-expand the section that contains the selected util, but only when navigation changes.
  const prevUtilHandle = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!selectedUtil || selectedUtil.handle === prevUtilHandle.current) {
      return;
    }
    prevUtilHandle.current = selectedUtil.handle;

    for (const root of uiState.utils) {
      const contains =
        root.handle === selectedUtil.handle ||
        root.utils?.some(
          (c) => c.handle === selectedUtil.handle || c.utils?.some((gc) => gc.handle === selectedUtil.handle),
        );
      if (contains && collapsedSections.has(root.handle)) {
        const updated = sidebarState.sections.filter((h) => h !== root.handle);
        setSettings({ [USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED]: { ...sidebarState, sections: updated } });
        break;
      }
    }
  }, [selectedUtil, uiState.utils, collapsedSections, sidebarState, setSettings]);

  const content = useMemo(() => {
    // Check if URL is invalid.
    if (utilIdFromParam && !utilsMap.has(utilIdFromParam)) {
      return <Navigate to="/ws" />;
    }

    // Check if the user tries to access known utility.
    if (!selectedUtil) {
      return <DEFAULT_COMPONENT />;
    }

    // Check if utility has a UI component defined.
    const ResolvedComponent =
      (uiState.userShare ? UtilsShareComponents.get(selectedUtil.handle) : null) ||
      UtilsComponents.get(selectedUtil.handle) ||
      DEFAULT_COMPONENT;
    // eslint-disable-next-line react-hooks/static-components -- Component is resolved from a static map, not dynamically created.
    return <ResolvedComponent />;
  }, [selectedUtil, utilsMap, utilIdFromParam, uiState]);

  const utilIcon = selectedUtil ? getUtilIcon(selectedUtil.handle) : undefined;
  const titleIcon = utilIcon ? (
    <EuiIcon
      css={css`
        margin: 4px;
        padding: 3px;
      `}
      type={utilIcon}
      size={'xl'}
    />
  ) : null;

  // Sidebar is only available to authenticated users.
  const sidebar = uiState.user ? (
    <aside onClickCapture={onSidebarClickCapture}>
      <SiteSearchBar />
      <EuiSpacer size="m" />
      <EuiSideNav items={sideNavItems} mobileBreakpoints={[]} />
    </aside>
  ) : null;

  // Authenticated and unauthenticated users have different header actions.
  const headerActions = uiState.user
    ? [
        <TagScopeSelector key="hdr-tags" selectedTagIds={globalScopeTagIds} onChange={onGlobalScopeTagIdsChange} />,
        ...actions,
      ]
    : actions;

  return (
    <Page
      pageTitle={
        <EuiFlexGroup justifyContent="spaceBetween" alignItems="center">
          <EuiFlexItem>
            <EuiFlexGroup responsive={false} gutterSize="s" alignItems="center">
              <EuiFlexItem grow={false}>{titleIcon}</EuiFlexItem>
              <EuiFlexItem>{title}</EuiFlexItem>
            </EuiFlexGroup>
          </EuiFlexItem>
          {titleActions ? <EuiFlexItem grow={false}>{titleActions}</EuiFlexItem> : null}
        </EuiFlexGroup>
      }
      sideBar={sidebar}
      headerBreadcrumbs={navigationBar.breadcrumbs}
      headerActions={headerActions}
      contentProps={{
        css: css`
          min-height: 100%;
        `,
      }}
    >
      <Suspense fallback={<PageLoadingState />}>
        <WorkspaceContext.Provider
          value={{ setTitleActions, setTitle, globalScopeTagIds, setGlobalScopeTagIds: onGlobalScopeTagIdsChange }}
        >
          {content}
          {isSettingsOpen ? (
            <SettingsFlyout
              onClose={hideSettings}
              importUrl={pendingImportUrl}
              onImportUrlConsumed={clearPendingImportUrl}
            />
          ) : null}
        </WorkspaceContext.Provider>
      </Suspense>
    </Page>
  );
}
