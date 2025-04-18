import { EuiCard, EuiFlexGroup, EuiFlexItem, EuiIcon, EuiLink, EuiText } from '@elastic/eui';

import { usePageMeta } from '../../../../hooks';
import HelpPageContent from '../../components/help_page_content';
import { GUIDE_CARD_STYLE } from '../../components/styles';

export default function Home() {
  usePageMeta('Welcome to Secutils.dev');

  return (
    <HelpPageContent>
      <EuiText size="relative">
        <h2>Welcome</h2>
        <p>
          Secutils.dev is an{' '}
          <EuiLink href="https://github.com/secutils-dev" target="_blank">
            open-source
          </EuiLink>{' '}
          toolbox that&apos;s versatile and simple, designed for both application security engineers and any other
          engineers looking to develop secure applications.
        </p>
        <p>
          The toolbox aims to simplify and streamline the process of developing and testing secure applications by
          providing a comprehensive collection of utilities commonly used by software engineers, all within a
          user-friendly and straightforward interface.
        </p>
        <p>
          If you have a question or idea, we encourage you to use the{' '}
          <EuiLink href="https://github.com/secutils-dev/secutils/discussions" target="_blank">
            Github Discussions
          </EuiLink>
          . For bug reports, please submit them directly to{' '}
          <EuiLink href="https://github.com/secutils-dev/secutils/issues" target="_blank">
            Github Issues
          </EuiLink>
          . If you need to contact us for anything else, feel free to do so using the &quot;Contact&quot; form.
        </p>
        <h2>Learn and get help</h2>
        <p>
          Whether you&apos;re a new or experienced user, these links will help you get the most out of Secutils.dev:
          learn how to use it, see what&apos;s new, and connect with other users and contributors.
        </p>
      </EuiText>
      <EuiFlexGroup gutterSize="xl" justifyContent={'center'} wrap alignItems={'center'}>
        <EuiFlexItem>
          <EuiCard
            css={GUIDE_CARD_STYLE}
            icon={<EuiIcon size="xxl" type={`training`} />}
            title="Getting Started"
            paddingSize="xl"
            description="Learn to use Secutils.dev through real-life scenarios"
            href={'/docs/category/guides'}
            target={'_blank'}
          />
        </EuiFlexItem>
        <EuiFlexItem>
          <EuiCard
            css={GUIDE_CARD_STYLE}
            icon={<EuiIcon size="xxl" type={`cheer`} />}
            title="What's New"
            paddingSize="xl"
            description="Discover what's new in the latest Secutils.dev version"
            href={'/docs/project/changelog'}
            target={'_blank'}
          />
        </EuiFlexItem>
        <EuiFlexItem>
          <EuiCard
            css={GUIDE_CARD_STYLE}
            icon={<EuiIcon size="xxl" type={`discuss`} />}
            title="Contribute"
            paddingSize="xl"
            description="Engage with the Secutils.dev community or become a contributor"
            href={'https://github.com/secutils-dev/secutils'}
          />
        </EuiFlexItem>
      </EuiFlexGroup>
    </HelpPageContent>
  );
}
