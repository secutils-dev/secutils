// Canonical registry of the seven free tools deployed under `{{TOOLS_HOST}}`.
// One source of truth used by:
//   - `og.spec.ts`        to render OG images
//   - `<slug>.spec.ts`    for per-tool SEO + functional assertions
//   - `registry.spec.ts`  to validate `llms.txt` and the index page list
//   - `make tools-check`  (via `scripts/tools-check.ts`) to assert that the
//                         promo home strip stays in sync with `promote: true`.
//
// Adding a new tool: drop a row here, add a `<slug>.spec.ts`, regenerate OG
// (`make tools-og`), and add the env-var responder ID. See dev/tools/AGENTS.md.

export const TOOLS_HOST = process.env.SECUTILS_TOOLS_PUBLIC_HOST?.trim() || 'tools.secutils.dev';

export interface Tool {
  /** Slug used in OG image filenames (`og-<slug>.png`) and inside the spec
   *  filename (`<slug>.spec.ts`). Path-derived so URLs stay self-evident. */
  slug: string;
  /** Source HTML filename in `dev/tools/`. */
  source: string;
  /** Human-readable name (matches `<meta name="su-tool-name">`). */
  name: string;
  /** URL path under the tools host (matches `<meta name="su-tool-path">`). */
  path: string;
  /** One-line description used in OG images, footer cards, llms.txt. */
  description: string;
  /** schema.org applicationCategory used in the WebApplication JSON-LD. */
  applicationCategory: 'SecurityApplication' | 'DeveloperApplication';
  /** Promoted on the marketing home page (chip strip, free-tools card). */
  promote: boolean;
  /** Per-tool accent color for OG images. Hex, lowercase. */
  accent: string;
  /** Symbolic icon id understood by `dev/tools/og-template.html`. */
  icon: 'key' | 'shield' | 'cert' | 'doc' | 'bolt' | 'id' | 'grid' | 'pdf' | 'chart';
}

export const TOOLS: readonly Tool[] = [
  {
    slug: 'jwt',
    source: 'jwt-debugger.html',
    name: 'JWT Debugger',
    path: '/jwt',
    description: 'Decode, verify, and sign HMAC JSON Web Tokens.',
    applicationCategory: 'SecurityApplication',
    promote: true,
    accent: '#7b6ff7',
    icon: 'key',
  },
  {
    slug: 'saml',
    source: 'saml-decoder.html',
    name: 'SAML Decoder',
    path: '/saml',
    description: 'Inspect SAML responses, requests, and metadata.',
    applicationCategory: 'SecurityApplication',
    promote: true,
    accent: '#2bb3bf',
    icon: 'shield',
  },
  {
    slug: 'pem',
    source: 'certificate-decoder.html',
    name: 'PEM Certificate Decoder',
    path: '/pem',
    description: 'Inspect PEM-encoded X.509 certificate chains.',
    applicationCategory: 'SecurityApplication',
    promote: true,
    accent: '#00bfb3',
    icon: 'cert',
  },
  {
    slug: 'md-to-html',
    source: 'markdown-to-html.html',
    name: 'Markdown to HTML',
    path: '/md-to-html',
    description: 'Self-contained HTML and PDF export from Markdown.',
    applicationCategory: 'DeveloperApplication',
    promote: true,
    accent: '#ff7e3a',
    icon: 'doc',
  },
  {
    slug: 'pdf',
    source: 'pdf-extractor.html',
    name: 'PDF Extractor',
    path: '/pdf',
    description: 'Extract spatial text and structured JSON from PDFs, in-browser.',
    applicationCategory: 'DeveloperApplication',
    promote: true,
    accent: '#e94f64',
    icon: 'pdf',
  },
  {
    slug: 'echo',
    source: 'echo.html',
    name: 'HTTP Echo / Mock Response',
    path: '/echo',
    description: 'Build a customizable HTTP response, served as a shareable URL.',
    applicationCategory: 'DeveloperApplication',
    promote: true,
    accent: '#fed047',
    icon: 'bolt',
  },
  {
    slug: 'webhook',
    source: 'webhook.html',
    name: 'Webhook Inspector',
    path: '/webhook',
    description: 'Ephemeral webhook URL that captures and decrypts incoming HTTP requests live.',
    applicationCategory: 'DeveloperApplication',
    promote: true,
    accent: '#34d399',
    icon: 'grid',
  },
  {
    slug: 'forecast',
    source: 'forecast.html',
    name: 'Forecast',
    path: '/forecast',
    description: 'Fit trendlines, forecast future values, and spot anomalies in numeric series.',
    applicationCategory: 'DeveloperApplication',
    promote: true,
    accent: '#3aa3ff',
    icon: 'chart',
  },
  {
    slug: 'mock-saml-idp',
    source: 'mock-saml-idp.html',
    name: 'Mock SAML IdP',
    path: '/elastic/saml/idp-login',
    description: 'Signed SAML responses for Elasticsearch and Kibana SSO testing.',
    applicationCategory: 'SecurityApplication',
    promote: false,
    accent: '#00b39b',
    icon: 'id',
  },
  {
    slug: 'index',
    source: 'index.html',
    name: 'Free Developer and Security Tools',
    path: '/',
    description: 'Index of free, no-signup single-page tools by Secutils.dev.',
    applicationCategory: 'DeveloperApplication',
    promote: false,
    accent: '#fed047',
    icon: 'grid',
  },
];

export const PROMOTED_TOOLS = TOOLS.filter((t) => t.promote);
