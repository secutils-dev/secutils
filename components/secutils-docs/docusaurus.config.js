// @ts-check
// Note: type annotations allow type checking and IDEs autocompletion

const { themes } = require('prism-react-renderer');
const lightTheme = themes.vsLight;
const darkTheme = themes.vsDark;

const SECUTILS_ENV = process.env.SECUTILS_ENV?.toLowerCase();
const BASE_URL =
    SECUTILS_ENV === 'dev' ? 'https://dev.secutils.dev'
    : SECUTILS_ENV === 'e2e' ? 'http://localhost:7171'
    : 'https://secutils.dev';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Secutils.dev',
  tagline: 'Documentation, user guides, step-by-step instructions and more…',
  favicon: 'img/favicon.ico',
  url: BASE_URL,
  baseUrl: '/docs/',
  baseUrlIssueBanner: true,
  customFields: {
    baseUrl: BASE_URL,
  },

  organizationName: 'secutils-dev',
  projectName: 'secutils-docs',

  markdown: { mermaid: true },
  themes: ['@docusaurus/theme-mermaid'],
  plugins: [
    'docusaurus-plugin-sass',
    [
      'docusaurus-plugin-llms',
      {
        generateLLMsTxt: true,
        generateLLMsFullTxt: true,
        // The default plugin layout puts only a link index in `llms.txt`. Many LLM crawlers fetch that URL alone and
        // see nothing useful. Put the full concatenated docs at `llms.txt` and keep the compact TOC as `llms-index.txt`.
        llmsTxtFilename: 'llms-index.txt',
        llmsFullTxtFilename: 'llms.txt',
        rootContent:
          'Compact index with links to each guide on the site. For all documentation in one file, use `llms.txt` in this directory.',
        fullRootContent:
          'Full Secutils.dev documentation (all guides) in one markdown document for LLM and offline use.',
        title: 'Secutils.dev Documentation',
        description:
          'Documentation and guides for Secutils.dev - an open-source, versatile toolbox for security-minded engineers.',
        includeBlog: false,
        excludeImports: true,
      },
    ],
  ],

  scripts: [{ src: '/js/script.js', async: true, defer: true, 'data-domain': 'secutils.dev' }],

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'throw',

  // Even if you don't use internalization, you can use this field to set useful
  // metadata like html lang. For example, if your site is Chinese, you may want
  // to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      '@docusaurus/preset-classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: require.resolve('./sidebars.js'),
          routeBasePath: '/',
          editUrl: 'https://github.com/secutils-dev/secutils-docs/tree/main/',
        },
        blog: {
          blogTitle: 'Blog',
          blogDescription: 'Blog - an open-source, versatile, yet simple toolbox for security-minded engineers built by application security engineers.',
          postsPerPage: 6,
          feedOptions: {
            type: 'all',
            title: 'Blog',
            copyright: `Copyright © ${new Date().getFullYear()} Secutils.dev`,
          },
        },
        theme: {
          customCss: require.resolve('./src/scss/index.scss'),
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      docs: {
        sidebar: {
          hideable: false,
        },
      },
      navbar: {
        logo: {
          alt: 'Navigate to Secutils.dev',
          src: 'img/logo.svg',
          srcDark: 'img/logo_dark.svg',
          target: '_self',
          height: 24,
          width: 147,
          href: BASE_URL,
        },
        items: [
          {
            sidebarId: 'docsSidebar',
            position: 'left',
            label: 'Docs',
            to: '/docs',
            className: 'su-navbar-breadcrumb su-navbar-docs'
          },
          {
            sidebarId: 'blogSidebar',
            position: 'left',
            label: 'Blog',
            to: '/blog',
            className: 'su-navbar-breadcrumb su-navbar-blog'
          },
          {
            type: 'html',
            value: `<div class="su-navbar-buttons"><a href="/signin" class="su-navbar-link">Sign in</a><a href="/signup"><button class="su-navbar-button">Start free trial</button></a></div>`,
            position: 'right',
          },
        ],
      },
      footer: {
        links: [
          {
            label: 'About',
            href: `${BASE_URL}/about`,
          },
          {
            label: 'Blog',
            href: '/blog',
          },
          {
            label: 'Docs',
            href: '/docs',
          },
          {
            label: 'Pricing',
            href: `${BASE_URL}/pricing`,
          },
          {
            label: 'Privacy',
            href: `${BASE_URL}/privacy`,
          },
          {
            label: 'Terms',
            href: `${BASE_URL}/terms`,
          },
        ],
        copyright: `Copyright © ${new Date().getFullYear()} Secutils.dev`,
      },
      prism: {
        theme: lightTheme,
        darkTheme: darkTheme,
        additionalLanguages: ['diff', 'json', 'yaml', 'bash'],
      },
      colorMode: {
        disableSwitch: false,
        respectPrefersColorScheme: false,
        defaultMode: 'light',
      },
    })
};

module.exports = config;
