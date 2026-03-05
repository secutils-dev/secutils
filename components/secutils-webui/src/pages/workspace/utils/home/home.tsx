import {
  EuiButtonEmpty,
  EuiCard,
  EuiFlexGroup,
  EuiFlexItem,
  EuiHorizontalRule,
  EuiIcon,
  EuiPanel,
  EuiProgress,
  EuiSpacer,
  EuiText,
  EuiTitle,
} from '@elastic/eui';
import { css } from '@emotion/react';
import type { MouseEvent } from 'react';
import { useNavigate } from 'react-router';

import { useWorkspaceSummary } from './use_workspace_summary';
import { usePageMeta } from '../../../../hooks';
import HelpPageContent from '../../components/help_page_content';
import { useWorkspaceContext } from '../../hooks';

interface ToolSubPath {
  label: string;
  path: string;
}

interface ToolDefinition {
  id: string;
  title: string;
  icon: string;
  description: string;
  path: string;
  guideUrl: string;
  subPaths?: ToolSubPath[];
  checklistPrompt: string;
}

const TOOLS: ToolDefinition[] = [
  {
    id: 'webhooks',
    title: 'Webhooks',
    icon: 'node',
    description: 'Create mock HTTP APIs, test webhook integrations, and set up honeypot endpoints.',
    path: '/ws/webhooks__responders',
    guideUrl: '/docs/guides/webhooks',
    checklistPrompt: 'Create your first webhook responder',
  },
  {
    id: 'certificates',
    title: 'Digital Certificates',
    icon: 'securityApp',
    description: 'Generate X.509 certificate templates and manage private keys for HTTPS and code signing.',
    path: '/ws/certificates__certificate_templates',
    guideUrl: '/docs/category/digital-certificates',
    checklistPrompt: 'Generate a certificate template',
  },
  {
    id: 'csp',
    title: 'Content Security Policy',
    icon: 'globe',
    description: 'Create, import, and test Content Security Policies for your web applications.',
    path: '/ws/web_security__csp__policies',
    guideUrl: '/docs/guides/web_security/csp',
    checklistPrompt: 'Set up a content security policy',
  },
  {
    id: 'webScraping',
    title: 'Web Scraping',
    icon: 'cut',
    description: 'Track changes in web pages and API responses over time with scheduled checks.',
    path: '/ws/web_scraping__page',
    guideUrl: '/docs/category/web-scraping',
    subPaths: [
      { label: 'Pages', path: '/ws/web_scraping__page' },
      { label: 'APIs', path: '/ws/web_scraping__api' },
    ],
    checklistPrompt: 'Track your first web page or API',
  },
];

const CARD_MIN_WIDTH = 280;

const toolCardStyle = css`
  flex: 1;
  cursor: pointer;
  min-height: 180px;
  transition: box-shadow 150ms ease-in-out;

  &:hover {
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  }

  .euiCard__content {
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .euiCard__description {
    flex: 1;
  }
`;

function countLabel(count: number): string {
  return count === 1 ? '1 item' : `${count} items`;
}

function formatRelativeTime(unixSeconds: number): string {
  const diffMs = Date.now() - unixSeconds * 1000;
  const minutes = Math.floor(diffMs / 60000);
  const hours = Math.floor(diffMs / 3600000);
  const days = Math.floor(diffMs / 86400000);

  if (minutes < 1) return 'Just now';
  if (minutes === 1) return '1 minute ago';
  if (minutes < 60) return `${minutes} minutes ago`;
  if (hours === 1) return '1 hour ago';
  if (hours < 24) return `${hours} hours ago`;
  if (days === 1) return 'Yesterday';
  if (days < 7) return `${days} days ago`;

  return new Date(unixSeconds * 1000).toLocaleDateString();
}

const sidePanelStyle = css`
  height: 100%;
`;

const TOOL_BY_ID = new Map(TOOLS.map((t) => [t.id, t]));

export default function Home() {
  usePageMeta('Welcome to Secutils.dev');

  const navigate = useNavigate();
  const { uiState } = useWorkspaceContext();
  const summary = useWorkspaceSummary(!!uiState.user);
  const counts = summary.counts;

  const activeToolCount = TOOLS.filter((t) => {
    const c = counts[t.id as keyof typeof counts];
    return c !== null && c > 0;
  }).length;

  const progressValue = summary.status === 'succeeded' ? (activeToolCount / TOOLS.length) * 100 : 0;

  const stopPropagation = (e: MouseEvent) => {
    e.stopPropagation();
  };

  const showChecklist = summary.status === 'succeeded' && activeToolCount < TOOLS.length;
  const showRecentItems = summary.status === 'succeeded' && summary.recentItems.length > 0;

  return (
    <HelpPageContent>
      {/* Hero panel */}
      <EuiPanel color="subdued" hasBorder paddingSize="l">
        <EuiTitle size="m">
          <h2>Welcome</h2>
        </EuiTitle>
        <EuiSpacer size="xs" />
        <EuiText size="s" color="subdued">
          <p>Your open-source security toolbox. Pick a tool to get started.</p>
        </EuiText>
        {summary.status === 'succeeded' && (
          <>
            <EuiSpacer size="m" />
            <EuiProgress value={progressValue} max={100} size="m" color="primary" />
            <EuiSpacer size="xs" />
            <EuiText size="xs" color="subdued">
              You&apos;re using {activeToolCount} of {TOOLS.length} tools
            </EuiText>
          </>
        )}
      </EuiPanel>

      <EuiSpacer size="l" />

      {/* Tool cards */}
      <EuiFlexGroup gutterSize="l" wrap>
        {TOOLS.map((tool) => {
          const count = counts[tool.id as keyof typeof counts];
          const isActive = count !== null && count > 0;

          return (
            <EuiFlexItem key={tool.id} style={{ minWidth: CARD_MIN_WIDTH }}>
              <EuiCard
                css={toolCardStyle}
                icon={<EuiIcon size="xl" type={tool.icon} />}
                title={tool.title}
                titleSize="xs"
                paddingSize="l"
                description={tool.description}
                onClick={() => navigate(tool.path)}
                betaBadgeProps={isActive ? { label: countLabel(count!) } : undefined}
                footer={
                  <EuiFlexGroup
                    justifyContent={tool.subPaths ? 'spaceBetween' : 'flexEnd'}
                    alignItems="center"
                    responsive={false}
                    wrap
                  >
                    {tool.subPaths && (
                      <EuiFlexItem grow={false}>
                        <EuiFlexGroup gutterSize="s" responsive={false}>
                          {tool.subPaths.map((sub) => (
                            <EuiFlexItem key={sub.path} grow={false}>
                              <EuiButtonEmpty
                                size="s"
                                onClick={(e: MouseEvent) => {
                                  e.stopPropagation();
                                  navigate(sub.path);
                                }}
                              >
                                {sub.label}
                              </EuiButtonEmpty>
                            </EuiFlexItem>
                          ))}
                        </EuiFlexGroup>
                      </EuiFlexItem>
                    )}
                    <EuiFlexItem grow={false}>
                      <EuiButtonEmpty
                        size="s"
                        iconType="training"
                        href={tool.guideUrl}
                        target="_blank"
                        onClick={stopPropagation}
                      >
                        Guide
                      </EuiButtonEmpty>
                    </EuiFlexItem>
                  </EuiFlexGroup>
                }
              />
            </EuiFlexItem>
          );
        })}
      </EuiFlexGroup>

      {/* Checklist + Recent items (side by side) */}
      {(showChecklist || showRecentItems) && (
        <>
          <EuiSpacer size="l" />
          <EuiFlexGroup gutterSize="l">
            {showChecklist && (
              <EuiFlexItem>
                <EuiPanel hasBorder paddingSize="m" css={sidePanelStyle}>
                  <EuiTitle size="xxs">
                    <h3>Get started</h3>
                  </EuiTitle>
                  <EuiSpacer size="s" />
                  <EuiFlexGroup direction="column" gutterSize="xs">
                    {TOOLS.map((tool) => {
                      const count = counts[tool.id as keyof typeof counts];
                      const completed = count !== null && count > 0;
                      return (
                        <EuiFlexItem key={tool.id}>
                          <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
                            <EuiFlexItem grow={false}>
                              <EuiIcon
                                size="s"
                                type={completed ? 'checkInCircleFilled' : 'plusInCircle'}
                                color={completed ? 'success' : 'subdued'}
                              />
                            </EuiFlexItem>
                            <EuiFlexItem>
                              <EuiText size="xs" color={completed ? 'subdued' : 'default'}>
                                {completed ? tool.title : tool.checklistPrompt}
                              </EuiText>
                            </EuiFlexItem>
                            {!completed && (
                              <EuiFlexItem grow={false}>
                                <EuiButtonEmpty size="xs" onClick={() => navigate(tool.path)}>
                                  Try it &rarr;
                                </EuiButtonEmpty>
                              </EuiFlexItem>
                            )}
                          </EuiFlexGroup>
                        </EuiFlexItem>
                      );
                    })}
                  </EuiFlexGroup>
                </EuiPanel>
              </EuiFlexItem>
            )}
            {showRecentItems && (
              <EuiFlexItem>
                <EuiPanel hasBorder paddingSize="m" css={sidePanelStyle}>
                  <EuiTitle size="xxs">
                    <h3>Recent items</h3>
                  </EuiTitle>
                  <EuiSpacer size="s" />
                  <EuiFlexGroup direction="column" gutterSize="xs">
                    {summary.recentItems.map((item, i) => {
                      const tool = TOOL_BY_ID.get(item.toolId);
                      return (
                        <EuiFlexItem key={i}>
                          <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
                            <EuiFlexItem grow={false}>
                              <EuiIcon size="s" type={tool?.icon ?? 'document'} color="subdued" />
                            </EuiFlexItem>
                            <EuiFlexItem grow={false}>
                              <EuiButtonEmpty
                                size="xs"
                                flush="left"
                                onClick={() => navigate(item.path)}
                              >
                                {item.name}
                              </EuiButtonEmpty>
                            </EuiFlexItem>
                            <EuiFlexItem grow />
                            <EuiFlexItem grow={false}>
                              <EuiText size="xs" color="subdued">
                                {formatRelativeTime(item.updatedAt)}
                              </EuiText>
                            </EuiFlexItem>
                          </EuiFlexGroup>
                        </EuiFlexItem>
                      );
                    })}
                  </EuiFlexGroup>
                </EuiPanel>
              </EuiFlexItem>
            )}
          </EuiFlexGroup>
        </>
      )}

      <EuiSpacer size="l" />
      <EuiHorizontalRule margin="none" />
      <EuiSpacer size="s" />

      {/* Learn & community links */}
      <EuiFlexGroup gutterSize="l" justifyContent="center" wrap responsive={false}>
        <EuiFlexItem grow={false}>
          <EuiButtonEmpty iconType="training" href="/docs/category/guides" target="_blank">
            Getting Started
          </EuiButtonEmpty>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButtonEmpty iconType="cheer" href="/docs/project/changelog" target="_blank">
            What&apos;s New
          </EuiButtonEmpty>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButtonEmpty iconType="logoGithub" href="https://github.com/secutils-dev/secutils" target="_blank">
            Contribute
          </EuiButtonEmpty>
        </EuiFlexItem>
      </EuiFlexGroup>
    </HelpPageContent>
  );
}

