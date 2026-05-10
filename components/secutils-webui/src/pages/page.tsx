import {
  EuiCollapsibleNav,
  EuiFlexGroup,
  EuiFlexItem,
  EuiHeader,
  EuiHeaderBreadcrumbs,
  EuiHeaderSection,
  EuiHeaderSectionItem,
  EuiHeaderSectionItemButton,
  EuiHorizontalRule,
  EuiIcon,
  EuiLink,
  EuiPage,
  EuiPageBody,
  EuiPageSection,
  EuiText,
} from '@elastic/eui';
import type { EuiBreadcrumb, EuiPageSectionProps, IconType } from '@elastic/eui';
import { css } from '@emotion/react';
import { useCallback, useState } from 'react';
import type { MouseEventHandler, ReactElement, ReactNode } from 'react';
import { Navigate, useLocation, useSearchParams } from 'react-router';

import { PageHeader } from './page_header';
import { ContactFormModal } from '../app_container/contact_form_modal';
import { LogoWithName, PageErrorState, PageLoadingState } from '../components';
import { useAppContext } from '../hooks';
import { parseSidebarCollapsed, USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED } from '../model';

export interface PageProps {
  children: ReactElement | ReactElement[];
  contentAlignment?: 'top' | 'center' | 'horizontalCenter';
  contentProps?: EuiPageSectionProps['contentProps'];
  sideBar?: ReactNode;
  headerBreadcrumbs?: EuiBreadcrumb[];
  headerActions?: ReactNode[];
  pageTitle?: ReactNode;
}

export interface PageToast {
  id: string;
  title?: ReactNode;
  text?: ReactElement;
  iconType?: IconType;
  color?: 'primary' | 'success' | 'warning' | 'danger';
}

function isUnauthenticatedPage(pathname: string) {
  return ['/signin', '/signup'].some((unauthenticatedPagePathname) => pathname.startsWith(unauthenticatedPagePathname));
}

export function Page({
  children,
  contentAlignment,
  contentProps,
  sideBar,
  headerBreadcrumbs,
  headerActions,
  pageTitle,
}: PageProps) {
  const { uiState, settings, setSettings } = useAppContext();
  const location = useLocation();
  const [searchParams] = useSearchParams();

  const sidebarState = parseSidebarCollapsed(settings?.[USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED]);
  const isNavOpen = !sidebarState.nav;
  const onToggleNav = useCallback(
    (open: boolean) => {
      const current = parseSidebarCollapsed(settings?.[USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED]);
      setSettings({ [USER_SETTINGS_KEY_COMMON_SIDEBAR_COLLAPSED]: { ...current, nav: !open } });
    },
    [setSettings, settings],
  );

  const [isContactFormOpen, setIsContactFormOpen] = useState<boolean>(false);
  const onToggleContactForm = useCallback(() => {
    setIsContactFormOpen(!isContactFormOpen);
  }, [isContactFormOpen, setIsContactFormOpen]);

  const contactFormModal = isContactFormOpen ? <ContactFormModal onClose={onToggleContactForm} /> : null;
  const onContactForm: MouseEventHandler<HTMLAnchorElement> = useCallback(
    (e) => {
      e.preventDefault();
      onToggleContactForm();
    },
    [onToggleContactForm],
  );

  if (!uiState.synced) {
    return <PageLoadingState />;
  }

  if (uiState?.status?.level === 'unavailable') {
    return (
      <PageErrorState
        title="Cannot connect to the server"
        content={
          <p>
            The <strong>Secutils.dev</strong> server is temporary not available.
          </p>
        }
      />
    );
  }

  if (!uiState.user && !uiState.userShare && !isUnauthenticatedPage(location.pathname)) {
    return (
      <Navigate
        to={
          location.pathname !== '/' && location.pathname !== '/ws'
            ? `/signin?next=${encodeURIComponent(`${location.pathname}?${searchParams.toString()}`)}`
            : '/signin'
        }
      />
    );
  }

  const header = pageTitle ? <PageHeader title={pageTitle} /> : null;
  return (
    <EuiPage grow direction={'row'}>
      <header aria-label="Top bar">
        <EuiHeader position="fixed">
          <EuiHeaderSection grow={false}>
            <EuiHeaderSectionItem>
              {sideBar ? (
                <EuiHeaderSectionItemButton
                  aria-label={isNavOpen ? 'Close navigation' : 'Open navigation'}
                  onClick={() => onToggleNav(!isNavOpen)}
                >
                  <EuiIcon type="menu" size="l" />
                </EuiHeaderSectionItemButton>
              ) : null}
            </EuiHeaderSectionItem>
            <EuiHeaderSectionItem>
              <EuiLink className="su-topbar-logo" href="/">
                <EuiIcon type={LogoWithName} size={'xl'} aria-label="Go to home page" />
              </EuiLink>
            </EuiHeaderSectionItem>
          </EuiHeaderSection>

          {headerBreadcrumbs && headerBreadcrumbs.length > 0 ? (
            <EuiHeaderBreadcrumbs
              css={css`
                @media screen and (max-width: 380px) {
                  display: none;
                }
              `}
              aria-label="Breadcrumbs"
              breadcrumbs={headerBreadcrumbs}
              lastBreadcrumbIsCurrentPage={true}
            />
          ) : undefined}

          {headerActions && headerActions.length > 0 ? (
            <EuiHeaderSection side="right">
              {headerActions.map((action, index) => (
                <EuiHeaderSectionItem key={`header-action-${index}`}>{action}</EuiHeaderSectionItem>
              ))}
            </EuiHeaderSection>
          ) : null}
        </EuiHeader>
      </header>

      {sideBar && isNavOpen ? (
        <EuiCollapsibleNav isOpen={true} isDocked={true} onClose={() => onToggleNav(false)} hideCloseButton={true}>
          <EuiFlexGroup direction="column" gutterSize="none" style={{ height: '100%' }}>
            <EuiFlexItem
              css={css`
                padding: 16px;
                overflow-y: auto;
              `}
            >
              {sideBar}
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiCollapsibleNav>
      ) : null}

      <EuiPageBody
        paddingSize="none"
        panelled
        css={css`
          min-width: 0;
        `}
      >
        {header}
        <EuiPageSection
          color="plain"
          alignment={contentAlignment}
          contentProps={contentProps}
          grow
          bottomBorder={false}
        >
          {children}
        </EuiPageSection>
        <EuiPageSection color="plain" paddingSize="m">
          <EuiHorizontalRule size={'half'} margin="m" />
          <EuiText textAlign={'center'} size={'xs'}>
            <EuiLink target="_blank" href="/about" color={'success'} external={false}>
              About
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/docs/blog" color={'success'} external={false}>
              Blog
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/docs" color={'success'} external={false}>
              Docs
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/pricing" color={'success'} external={false}>
              Pricing
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/privacy" color={'success'} external={false}>
              Privacy
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/terms" color={'success'} external={false}>
              Terms
            </EuiLink>{' '}
            ·{' '}
            <EuiLink onClick={onContactForm} color={'success'}>
              Contact
            </EuiLink>
          </EuiText>
          <EuiText
            textAlign={'center'}
            size="xs"
            css={css`
              margin-top: 8px;
            `}
          >
            Copyright © {new Date().getFullYear()} Secutils.dev
          </EuiText>
        </EuiPageSection>
      </EuiPageBody>
      {contactFormModal}
    </EuiPage>
  );
}
