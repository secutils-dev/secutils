import {
  EuiHeader,
  EuiHeaderBreadcrumbs,
  EuiHeaderSection,
  EuiHeaderSectionItem,
  EuiHorizontalRule,
  EuiIcon,
  EuiLink,
  EuiPage,
  EuiPageBody,
  EuiPageSection,
  EuiPageSidebar,
  EuiText,
} from '@elastic/eui';
import type { EuiPageSectionProps, IconType } from '@elastic/eui';
import type { EuiBreadcrumbProps } from '@elastic/eui/src/components/breadcrumbs/types';
import { css } from '@emotion/react';
import { useCallback, useState } from 'react';
import type { MouseEventHandler, ReactElement, ReactNode } from 'react';
import { Navigate, useLocation, useSearchParams } from 'react-router';

import { PageHeader } from './page_header';
import { ContactFormModal } from '../app_container/contact_form_modal';
import { LogoWithName, PageErrorState, PageLoadingState } from '../components';
import { useAppContext } from '../hooks';

export interface PageProps {
  children: ReactElement | ReactElement[];
  contentAlignment?: 'top' | 'center' | 'horizontalCenter';
  contentProps?: EuiPageSectionProps['contentProps'];
  sideBar?: ReactNode;
  headerBreadcrumbs?: EuiBreadcrumbProps[];
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
  const { uiState } = useAppContext();
  const location = useLocation();
  const [searchParams] = useSearchParams();

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
          <EuiHeaderSection
            grow={false}
            css={css`
              line-height: 0.5rem;
              margin-right: 0.5rem;
            `}
          >
            <EuiHeaderSectionItem>
              <EuiLink href="/">
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

      {sideBar ? (
        <EuiPageSidebar paddingSize="m" sticky={{ offset: 48 }} minWidth={300}>
          {sideBar}
        </EuiPageSidebar>
      ) : null}

      <EuiPageBody paddingSize="none" panelled>
        {header}
        <EuiPageSection color="plain" alignment={contentAlignment} contentProps={contentProps} grow>
          {children}
        </EuiPageSection>
        <EuiPageSection color="plain" paddingSize="m">
          <EuiHorizontalRule size={'half'} margin="m" />
          <EuiText textAlign={'center'} size={'xs'}>
            <EuiLink target="_blank" href="/about" color={'success'}>
              About
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/docs/blog" color={'success'}>
              Blog
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/docs" color={'success'}>
              Docs
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/pricing" color={'success'}>
              Pricing
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/privacy" color={'success'}>
              Privacy
            </EuiLink>{' '}
            ·{' '}
            <EuiLink target="_blank" href="/terms" color={'success'}>
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
