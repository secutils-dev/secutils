import type { EuiSwitchEvent } from '@elastic/eui';
import {
  EuiButtonIcon,
  EuiComboBox,
  EuiDescribedFormGroup,
  EuiFieldNumber,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiForm,
  EuiFormRow,
  EuiLink,
  EuiRange,
  EuiSelect,
  EuiSwitch,
  EuiText,
  EuiTitle,
} from '@elastic/eui';
import { customAlphabet, urlAlphabet } from 'nanoid';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ChangeEvent } from 'react';

import type { Responder } from './responder';
import { useFormChanges, useRangeTicks, useUserTags } from '../../../../hooks';
import type { AsyncData } from '../../../../model';
import { apiFetch, getErrorMessage, isClientError, ResponseError } from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { ScriptEditor } from '../../components/script_editor';
import type { ImportAction, ScriptSnippet } from '../../components/script_editor';
import { ScriptImportSelector } from '../../components/script_import_selector';
import { TagsComboBox } from '../../components/tags_combo_box';
import { useWorkspaceContext } from '../../hooks';

export interface ResponderEditFlyoutProps {
  responder?: Partial<Responder>;
  onClose: (success?: boolean) => void;
}

const HTTP_METHODS = ['ANY', 'GET', 'POST', 'PUT', 'DELETE', 'HEAD', 'OPTIONS', 'CONNECT', 'TRACE', 'PATCH'];
const PATH_TYPES = [
  { value: '=', text: 'Exact' },
  { value: '^', text: 'Prefix' },
];

const SUBDOMAIN_PREFIX_REGEX = /^[a-z0-9-]+$/i;

const isHeaderValid = (header: string) => {
  return header.length >= 3 && header.includes(':') && !header.startsWith(':') && !header.endsWith(':');
};

// Only basic validation to assist the user in entering a valid subdomain prefix.
// The full validation is done on the server.
const isSubdomainPrefixValid = (subdomainPrefix: string) => {
  const hostname = `${subdomainPrefix}-handle.example.com`;
  let url;
  try {
    url = new URL(`https://${hostname}`);
  } catch {
    return false;
  }

  if (url.hostname !== hostname) {
    return false;
  }

  return SUBDOMAIN_PREFIX_REGEX.test(subdomainPrefix) && !subdomainPrefix.includes('.') && subdomainPrefix.length < 45;
};

const nanoidCustom = customAlphabet(urlAlphabet.replace('_', '').replace('-', ''), 7);

function getBodyLanguage(headers: Array<{ label: string }>): string {
  const contentType = headers
    .find((h) => h.label.toLowerCase().startsWith('content-type:'))
    ?.label.split(':')[1]
    ?.trim()
    .toLowerCase();
  if (!contentType) {
    return 'plaintext';
  }
  if (contentType.includes('html')) {
    return 'html';
  }
  if (contentType.includes('json')) {
    return 'json';
  }
  if (contentType.includes('javascript')) {
    return 'javascript';
  }
  if (contentType.includes('css')) {
    return 'css';
  }
  return 'plaintext';
}

const SCRIPT_SNIPPETS: ScriptSnippet[] = [
  {
    id: 'responder-script-basic',
    label: 'Insert Example: Responder Script',
    template: [
      '(() => {',
      '    const { method, path, body } = context;',
      '',
      '    return {',
      '        statusCode: 200,',
      "        headers: { 'Content-Type': 'application/json' },",
      '        body: { method, path, body },',
      '    };',
      '})();',
    ].join('\n'),
  },
  {
    id: 'responder-script-forwarder',
    label: 'Insert Example: Request Forwarder',
    template: [
      '(async () => {',
      '    return await Deno.core.ops.op_proxy_request({',
      '        url: context.secrets.upstreamUrl + context.path,',
      '        method: context.method,',
      '        headers: context.headers,',
      '        body: context.body,',
      '    });',
      '})()',
    ].join('\n'),
  },
  {
    id: 'responder-script-forwarder-advanced',
    label: 'Insert Example: Advanced Request Forwarder',
    template: [
      '(async () => {',
      '    const resp = await Deno.core.ops.op_proxy_request({',
      '        url: context.secrets.upstreamUrl + context.path,',
      '        method: context.method,',
      '        headers: context.headers,',
      '        body: context.body,',
      '    });',
      '',
      '    const body = JSON.parse(Deno.core.decode(new Uint8Array(resp.body)));',
      '    body._proxied = true;',
      '',
      '    return {',
      '        statusCode: resp.statusCode === 404 ? 200 : resp.statusCode,',
      "        headers: { ...resp.headers, 'X-Proxied': 'true' },",
      '        body,',
      '        trackResponse: true,',
      "        skipRequest: context.path === '/healthz',",
      '    };',
      '})()',
    ].join('\n'),
  },
  {
    id: 'responder-script-basic-auth',
    label: 'Insert Example: Protect with HTTP Basic Auth',
    template: `(() => {
    // === Configuration ===
    // Set REQUIRE_USERNAME to true to also require a username.
    // - false (default): any username is accepted; only APP_PASSWORD is checked.
    // - true: the snippet additionally reads the expected username from
    //   the secret APP_USER. Create it in Workspace → Secrets.
    const REQUIRE_USERNAME = false;

    const expectedPassword = context.secrets.APP_PASSWORD;
    const expectedUsername = REQUIRE_USERNAME ? context.secrets.APP_USER : null;

    if (!expectedPassword || (REQUIRE_USERNAME && !expectedUsername)) {
        return {
            statusCode: 500,
            headers: { 'Content-Type': 'text/plain; charset=utf-8' },
            body: REQUIRE_USERNAME
                ? 'Missing required secrets APP_USER and/or APP_PASSWORD. Create them in Workspace → Secrets and grant this responder access to them.'
                : 'Missing required secret APP_PASSWORD. Create it in Workspace → Secrets and grant this responder access to it.',
        };
    }

    // Constant-time string comparison.
    const ctEq = (a, b) => {
        if (a.length !== b.length) return false;
        let r = 0;
        for (let i = 0; i < a.length; i++) r |= a.charCodeAt(i) ^ b.charCodeAt(i);
        return r === 0;
    };

    // Pure-JS base64 decoder (no atob in the sandbox).
    const fromBase64 = (b64) => {
        const C = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
        const s = b64.replace(/=+$/, '');
        const out = [];
        for (let i = 0; i < s.length; i += 4) {
            const a = C.indexOf(s[i]);
            const b = C.indexOf(s[i + 1]);
            const c = C.indexOf(s[i + 2]);
            const d = C.indexOf(s[i + 3]);
            out.push((a << 2) | (b >> 4));
            if (c >= 0) out.push(((b & 15) << 4) | (c >> 2));
            if (d >= 0) out.push(((c & 3) << 6) | d);
        }
        return Deno.core.decode(new Uint8Array(out));
    };

    const auth = context.headers['authorization'] || '';
    const match = /^Basic\\s+(\\S+)$/.exec(auth);
    if (match) {
        const decoded = fromBase64(match[1]);
        const colon = decoded.indexOf(':');
        const username = colon >= 0 ? decoded.slice(0, colon) : '';
        const password = colon >= 0 ? decoded.slice(colon + 1) : '';
        const passwordOk = ctEq(password, expectedPassword);
        const usernameOk = !REQUIRE_USERNAME || ctEq(username, expectedUsername);
        if (passwordOk && usernameOk) {
            // Authenticated — fall through to the responder's default response.
            return null;
        }
    }

    return {
        statusCode: 401,
        headers: {
            'WWW-Authenticate': 'Basic realm="Protected", charset="UTF-8"',
            'Content-Type': 'text/plain; charset=utf-8',
        },
        body: 'Authentication required',
    };
})();`,
  },
  {
    id: 'responder-script-cookie-session',
    label: 'Insert Example: Protect with Login Form (Cookie Session)',
    template: `(() => {
    // === Configuration ===
    // Set REQUIRE_USERNAME to true to also show a Username field on the login form.
    // - false (default): only APP_PASSWORD is required.
    // - true: the snippet additionally reads the expected username from
    //   the secret APP_USER. Create it in Workspace → Secrets.
    // NOTE: set the responder's HTTP method to ANY so this script can handle
    // both the GET (form render) and the POST (login submission).
    const REQUIRE_USERNAME = false;

    const PASSWORD = context.secrets.APP_PASSWORD;
    const USERNAME = REQUIRE_USERNAME ? context.secrets.APP_USER : null;
    const COOKIE_NAME = 'sec_auth';
    const MAX_AGE_SEC = 86400; // 24 hours

    if (!PASSWORD || (REQUIRE_USERNAME && !USERNAME)) {
        return {
            statusCode: 500,
            headers: { 'Content-Type': 'text/plain; charset=utf-8' },
            body: REQUIRE_USERNAME
                ? 'Missing required secrets APP_USER and/or APP_PASSWORD. Create them in Workspace → Secrets and grant this responder access to them.'
                : 'Missing required secret APP_PASSWORD. Create it in Workspace → Secrets and grant this responder access to it.',
        };
    }

    // Constant-time string comparison.
    const ctEq = (a, b) => {
        if (a.length !== b.length) return false;
        let r = 0;
        for (let i = 0; i < a.length; i++) r |= a.charCodeAt(i) ^ b.charCodeAt(i);
        return r === 0;
    };

    // Parse application/x-www-form-urlencoded body (URLSearchParams may not exist in the sandbox).
    const parseForm = (raw) => {
        const out = {};
        for (const pair of raw.split('&')) {
            if (!pair) continue;
            const eq = pair.indexOf('=');
            const rk = eq < 0 ? pair : pair.slice(0, eq);
            const rv = eq < 0 ? '' : pair.slice(eq + 1);
            out[decodeURIComponent(rk.replace(/\\+/g, ' '))] =
                decodeURIComponent(rv.replace(/\\+/g, ' '));
        }
        return out;
    };

    const sessionCookie = \`\${COOKIE_NAME}=\${encodeURIComponent(PASSWORD)}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=\${MAX_AGE_SEC}\`;
    const clearCookie = \`\${COOKIE_NAME}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0\`;

    const usernameField = REQUIRE_USERNAME
        ? '<label for="user">Username</label>\\n    <input type="text" name="username" id="user" autocomplete="username" autofocus required>'
        : '';
    const pwdAutofocus = REQUIRE_USERNAME ? '' : 'autofocus';

    const renderLogin = (errorHtml) => ({
        statusCode: 401,
        headers: { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' },
        body: \`<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sign in</title>
<link rel="preconnect" href="https://rsms.me/">
<link rel="stylesheet" href="https://rsms.me/inter/inter.css">
<style>
:root {
  color-scheme: light dark;
  --bg:#F6F9FC; --fg:#1D2A3E; --muted:#516381; --title:#111C2C;
  --card:#FFFFFF; --card-border:#E3E8F2; --border:#CAD3E2; --input-bg:#FFFFFF;
  --primary:#0B64DD; --primary-fg:#FFFFFF; --primary-hover:#0A59C7;
  --error:#A71627; --error-bg:#FFE8E5;
  --shadow:0 0 2px hsl(216.67 29.51% 23.92%/.16),
          0 3px 10px hsl(216.67 29.51% 23.92%/.1),
          0 6px 14px hsl(216.67 29.51% 23.92%/.06);
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg:#07101F; --fg:#CAD3E2; --muted:#98A8C3; --title:#E3E8F2;
    --card:#0B1628; --card-border:#2B394F; --border:#485975; --input-bg:#0B1628;
    --primary:#61A2FF; --primary-fg:#07101F; --primary-hover:#84B8FF;
    --error:#F6726A; --error-bg:#351721;
    --shadow:0 3px 10px hsla(0,0%,0%,.52), 0 6px 14px hsla(0,0%,0%,.28);
  }
}
*,*::before,*::after { box-sizing: border-box; }
html,body { margin:0; height:100%; background:var(--bg); color:var(--fg);
  font-family:'Inter','-apple-system',BlinkMacSystemFont,'Segoe UI','Helvetica','Arial',sans-serif;
  font-size:14px; line-height:1.4286; }
main { min-height:100%; display:flex; align-items:center; justify-content:center; padding:24px; }
.card { width:320px; background:var(--card); border:1px solid var(--card-border);
  border-radius:6px; padding:24px; box-shadow:var(--shadow); }
h1 { margin:0 0 4px; font-size:20px; font-weight:600; line-height:1.7143rem; color:var(--title); }
.subtitle { margin:0 0 20px; color:var(--muted); font-size:14px; }
label { display:block; margin:14px 0 4px; font-size:12px; font-weight:600; color:var(--title); }
.card > label:first-of-type, form > label:first-of-type { margin-top:0; }
input { width:100%; height:40px; padding:0 12px; border:1px solid var(--border); border-radius:4px;
  background:var(--input-bg); color:var(--fg); font-family:inherit; font-size:14px; outline:none;
  transition:border-color .15s, box-shadow .15s; }
input:focus { border-color:var(--primary); box-shadow:0 0 0 1px var(--primary); }
button { margin-top:20px; width:100%; height:40px; padding:0 12px; background:var(--primary);
  color:var(--primary-fg); border:0; border-radius:4px; font-family:inherit; font-size:14px;
  font-weight:500; cursor:pointer; transition:background .15s; }
button:hover { background:var(--primary-hover); }
.error { margin:0 0 16px; padding:8px 12px; border-radius:4px;
  background:var(--error-bg); color:var(--error); font-size:13px; }
</style>
</head>
<body>
<main>
  <form class="card" method="POST" action="?_auth=1">
    <h1>Sign in</h1>
    <p class="subtitle">Enter the password to access this page.</p>
    \${errorHtml}
    \${usernameField}
    <label for="pwd">Password</label>
    <input type="password" name="password" id="pwd" autocomplete="current-password" \${pwdAutofocus} required>
    <button type="submit">Sign in</button>
  </form>
</main>
</body>
</html>\`,
    });

    // Logout: ?_logout=1 clears the cookie and shows the login form again.
    if (context.query._logout === '1') {
        return {
            statusCode: 303,
            headers: {
                'Set-Cookie': clearCookie,
                'Location': context.path,
                'Content-Type': 'text/plain; charset=utf-8',
            },
            body: 'Signed out',
        };
    }

    // Login form submission.
    if (context.method === 'POST' && context.query._auth === '1') {
        const form = parseForm(Deno.core.decode(new Uint8Array(context.body)));
        const submittedPassword = form.password || '';
        const submittedUsername = form.username || '';
        const passwordOk = ctEq(submittedPassword, PASSWORD);
        const usernameOk = !REQUIRE_USERNAME || ctEq(submittedUsername, USERNAME);
        if (passwordOk && usernameOk) {
            return {
                statusCode: 303,
                headers: {
                    'Set-Cookie': sessionCookie,
                    'Location': context.path,
                    'Content-Type': 'text/plain; charset=utf-8',
                },
                body: 'Authenticated',
            };
        }
        return renderLogin('<p class="error">Incorrect ' + (REQUIRE_USERNAME ? 'username or password' : 'password') + '.</p>');
    }

    // Existing session cookie?
    const cookieHeader = context.headers['cookie'] || '';
    const matched = cookieHeader
        .split(';')
        .map((s) => s.trim())
        .find((c) => c.startsWith(COOKIE_NAME + '='));
    if (matched) {
        const value = decodeURIComponent(matched.slice(COOKIE_NAME.length + 1));
        if (ctEq(value, PASSWORD)) {
            // Authenticated — fall through to the responder's default response.
            return null;
        }
    }

    return renderLogin('');
})();`,
  },
];

export default function ResponderEditFlyout({ onClose, responder }: ResponderEditFlyoutProps) {
  const { addToast, uiState } = useWorkspaceContext();
  const maxTicks = useRangeTicks();

  const newResponder = !responder?.id;

  const httpMethods = useMemo(() => HTTP_METHODS.map((method) => ({ value: method, text: method })), []);

  const [isAdvancedMode, setIsAdvancedMode] = useState(
    !newResponder &&
      (responder?.method !== 'ANY' ||
        responder?.enabled === false ||
        !!responder?.settings?.script ||
        (!!responder?.settings?.secrets && responder.settings.secrets.type !== 'none')),
  );

  const [name, setName] = useState<string>(responder?.name ?? '');
  const onNameChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
  }, []);

  const defaultRandom = useMemo(() => (!responder ? nanoidCustom().toLowerCase() : ''), [responder]);
  const [subdomainPrefix, setSubdomainPrefix] = useState<string>(responder?.location?.subdomainPrefix ?? defaultRandom);
  const onSubdomainPrefixChange = (e: ChangeEvent<HTMLInputElement>) => {
    setSubdomainPrefix(e.target.value.toLowerCase());
  };

  const [path, setPath] = useState<string>(responder?.location?.path ?? '/');
  const onPathChange = (e: ChangeEvent<HTMLInputElement>) => {
    setPath(e.target.value.toLowerCase());
  };
  const isPathValid = path.startsWith('/') && (path.length === 1 || !path.endsWith('/'));

  const [pathType, setPathType] = useState<string>(responder?.location?.pathType ?? '^');
  const onPathTypeChange = (e: ChangeEvent<HTMLSelectElement>) => {
    setPathType(e.target.value);
  };

  const [requestsToTrack, setRequestsToTrack] = useState<number>(
    responder?.settings?.requestsToTrack ??
      Math.min(uiState.subscription?.features?.webhooks.responderRequests ?? 0, 10),
  );

  const [statusCode, setStatusCode] = useState<number>(responder?.settings?.statusCode ?? 200);
  const onStatusCodeChange = (e: ChangeEvent<HTMLInputElement>) => {
    setStatusCode(+e.target.value);
  };

  const [method, setMethod] = useState<string>(responder?.method ?? 'ANY');
  const onMethodChange = (e: ChangeEvent<HTMLSelectElement>) => {
    setMethod(e.target.value);
  };

  const [isEnabled, setIsEnabled] = useState<boolean>(responder?.enabled ?? true);
  const onIsEnabledChange = useCallback((e: EuiSwitchEvent) => {
    setIsEnabled(e.target.checked);
  }, []);

  const [headers, setHeaders] = useState<Array<{ label: string }>>(
    responder?.settings?.headers?.map(([header, value]) => ({ label: `${header}: ${value}` })) ?? [
      { label: 'Content-Type: text/html; charset=utf-8' },
    ],
  );
  const [areHeadersInvalid, setAreHeadersInvalid] = useState(false);

  const [script, setScript] = useState<string | undefined>(responder?.settings?.script);
  const onUserScriptChange = useCallback((value?: string) => {
    setScript(value);
  }, []);

  const [isImportModalVisible, setIsImportModalVisible] = useState(false);
  const handleImportScript = useCallback((content: string) => {
    setScript(content);
    setIsImportModalVisible(false);
  }, []);

  const scriptImportActions: ImportAction[] = useMemo(
    () => [
      {
        id: 'import-predefined-script',
        label: 'Import from predefined scripts',
        description: 'Select a responder script from your library',
        transform: (input: string) => input,
        onTrigger: () => setIsImportModalVisible(true),
      },
    ],
    [],
  );

  const existingSecrets = responder?.settings?.secrets;
  const [secretsMode, setSecretsMode] = useState<'none' | 'all' | 'selected'>(existingSecrets?.type ?? 'none');
  const [selectedSecretNames, setSelectedSecretNames] = useState<Array<{ label: string }>>(
    existingSecrets?.type === 'selected' ? (existingSecrets.secrets ?? []).map((s) => ({ label: s })) : [],
  );
  const [availableSecrets, setAvailableSecrets] = useState<Array<{ label: string }>>([]);
  const [secretsLoaded, setSecretsLoaded] = useState(false);

  const { allTags, setAllTags } = useUserTags();
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(responder?.tags?.map((t) => t.id) ?? []);

  useEffect(() => {
    if (secretsMode !== 'selected' || secretsLoaded) {
      return;
    }
    apiFetch('/api/user/secrets')
      .then(async (res) => {
        if (res.ok) {
          const data: Array<{ name: string }> = await res.json();
          setAvailableSecrets(data.map((s) => ({ label: s.name })));
        }
      })
      .catch(() => {})
      .finally(() => setSecretsLoaded(true));
  }, [secretsMode, secretsLoaded]);

  const onCreateHeader = (headerValue: string) => {
    if (!isHeaderValid(headerValue)) {
      return false;
    }

    setHeaders([...headers, { label: headerValue }]);
  };

  const onHeadersSearchChange = (headerValue: string) => {
    if (!headerValue) {
      setAreHeadersInvalid(false);
      return;
    }

    setAreHeadersInvalid(!isHeaderValid(headerValue));
  };

  const onHeadersChange = (selectedHeaders: Array<{ label: string }>) => {
    setHeaders(selectedHeaders);
    setAreHeadersInvalid(false);
  };

  const [body, setBody] = useState<string>(
    responder?.settings?.body ?? 'Hello from <a href="https://secutils.dev">Secutils.dev</a>!',
  );
  const isDuplicate = !!responder && !responder.id;
  const hasFormChanges = useFormChanges({
    name,
    subdomainPrefix,
    path,
    pathType,
    requestsToTrack,
    statusCode,
    method,
    isEnabled,
    headers,
    body,
    script,
    secretsMode,
    selectedSecretNames,
    selectedTagIds,
  });
  const hasChanges = isDuplicate || hasFormChanges;

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();
  const onSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

    const locationSubdomainPrefix = subdomainPrefix || undefined;
    let location;
    if (!newResponder) {
      location =
        responder.location?.path !== path ||
        responder.location?.pathType !== pathType ||
        responder.location?.subdomainPrefix !== locationSubdomainPrefix
          ? { pathType, path: path.trim(), subdomainPrefix: locationSubdomainPrefix }
          : null;
    } else {
      location = { pathType, path: path.trim(), subdomainPrefix: locationSubdomainPrefix };
    }

    const responderToUpdate = {
      name: newResponder ? name.trim() : responder.name !== name ? name.trim() : null,
      location,
      method: newResponder ? method : responder.method !== method ? method : null,
      enabled: newResponder ? isEnabled : responder.enabled !== isEnabled ? isEnabled : null,
      settings: {
        requestsToTrack,
        statusCode,
        body: body && method !== 'HEAD' ? body : undefined,
        headers:
          headers.length > 0
            ? headers.map((headerValue) => {
                const separatorIndex = headerValue.label.indexOf(':');
                return [
                  headerValue.label.substring(0, separatorIndex).trim(),
                  headerValue.label.substring(separatorIndex + 1).trim(),
                ] as [string, string];
              })
            : undefined,
        script: script?.trim() ? script.trim() : undefined,
        secrets:
          secretsMode === 'none'
            ? { type: 'none' as const }
            : secretsMode === 'all'
              ? { type: 'all' as const }
              : { type: 'selected' as const, secrets: selectedSecretNames.map((s) => s.label) },
      },
      tagIds: selectedTagIds,
    };

    const [requestPromise, successMessage, errorMessage] = !newResponder
      ? [
          apiFetch(`/api/webhooks/responders/${responder.id}`, {
            method: 'PUT',
            body: JSON.stringify(responderToUpdate),
          }),
          `Successfully updated "${name}" responder`,
          `Unable to update "${name}" responder, please try again later`,
        ]
      : [
          apiFetch('/api/webhooks/responders', {
            method: 'POST',
            body: JSON.stringify(responderToUpdate),
          }),
          `Successfully saved "${name}" responder`,
          `Unable to save "${name}" responder, please try again later`,
        ];
    requestPromise
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        setUpdatingStatus({ status: 'succeeded', data: undefined });

        addToast({
          id: `success-save-responder-${name}`,
          iconType: 'check',
          color: 'success',
          title: successMessage,
        });

        onClose(true);
      })
      .catch((err: Error) => {
        const remoteErrorMessage = getErrorMessage(err);
        setUpdatingStatus({ status: 'failed', error: remoteErrorMessage });

        addToast({
          id: `failed-save-responder-${name}`,
          iconType: 'warning',
          color: 'danger',
          title: isClientError(err) ? remoteErrorMessage : errorMessage,
        });
      });
  }, [
    name,
    method,
    path,
    subdomainPrefix,
    pathType,
    isEnabled,
    requestsToTrack,
    statusCode,
    body,
    headers,
    script,
    secretsMode,
    selectedSecretNames,
    selectedTagIds,
    responder,
    updatingStatus,
    newResponder,
    addToast,
    onClose,
  ]);

  const maxResponderRequests = uiState.subscription?.features?.webhooks.responderRequests ?? 0;
  const tickInterval = Math.ceil(maxResponderRequests / maxTicks);
  return (
    <EditorFlyout
      title={
        <EuiFlexGroup>
          <EuiFlexItem>
            <EuiTitle size="s">
              <h1>{`${responder ? 'Edit' : 'Add'} responder`}</h1>
            </EuiTitle>
          </EuiFlexItem>
          <EuiFlexItem grow={false}>
            <EuiSwitch
              label={
                <EuiText color={'subdued'} size={'s'}>
                  Advanced mode
                </EuiText>
              }
              checked={isAdvancedMode}
              onChange={(e) => setIsAdvancedMode(e.target.checked)}
            />
          </EuiFlexItem>
        </EuiFlexGroup>
      }
      onClose={() => onClose()}
      onSave={onSave}
      hasChanges={hasChanges}
      canSave={
        name.trim().length > 0 &&
        !areHeadersInvalid &&
        isPathValid &&
        (!subdomainPrefix || isSubdomainPrefixValid(subdomainPrefix)) &&
        requestsToTrack >= 0 &&
        requestsToTrack <= (uiState.subscription?.features?.webhooks.responderRequests ?? 100) &&
        (hasFormChanges || isDuplicate)
      }
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm id="update-form" component="form" fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description={'General properties of the responder'}>
          <EuiFormRow label="Name" helpText="Arbitrary responder name." fullWidth>
            <EuiFieldText autoFocus value={name} required type={'text'} onChange={onNameChange} />
          </EuiFormRow>
          <TagsComboBox
            allTags={allTags}
            selectedTagIds={selectedTagIds}
            onChange={setSelectedTagIds}
            onTagCreated={(tag) => setAllTags((prev) => [...prev, tag])}
          />
          {isAdvancedMode ? (
            <EuiFormRow label="Tracking" helpText="Responder will track only specified number of incoming requests">
              <EuiRange
                min={0}
                max={maxResponderRequests}
                value={requestsToTrack}
                fullWidth
                onChange={(e) => setRequestsToTrack(+e.currentTarget.value)}
                showRange
                showTicks
                tickInterval={tickInterval > 1 ? Math.ceil(tickInterval / 5) * 5 : tickInterval}
                showValue={maxResponderRequests > maxTicks}
              />
            </EuiFormRow>
          ) : null}
          {isAdvancedMode ? (
            <EuiFormRow
              label={'Enable'}
              helpText={'Instructs the responder whether it should process incoming requests or not.'}
            >
              <EuiSwitch showLabel={false} label="Enable" checked={isEnabled} onChange={onIsEnabledChange} />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup
          title={<h3>Request</h3>}
          description={'Properties of the responder related to the HTTP requests it handles'}
        >
          <EuiFormRow
            label="Subdomain prefix"
            helpText={
              <>
                Responder will only respond to requests with the&nbsp;
                <b>
                  {subdomainPrefix || '<subdomain-prefix>'}-{uiState.user?.handle ?? '<user-handle>'}
                  .webhooks.{location.host}
                </b>
                &nbsp;domain
              </>
            }
          >
            <EuiFieldText
              value={subdomainPrefix}
              isInvalid={subdomainPrefix.length > 0 && !isSubdomainPrefixValid(subdomainPrefix)}
              placeholder={`If not specified, ${uiState.user?.handle ?? '<user-handle>'} subdomain will be used`}
              type={'text'}
              onChange={onSubdomainPrefixChange}
              append={
                <EuiButtonIcon
                  iconType="refresh"
                  title={'Generate random prefix'}
                  aria-label="Generate random prefix"
                  onClick={() => setSubdomainPrefix(nanoidCustom().toLowerCase())}
                />
              }
            />
          </EuiFormRow>
          <EuiFormRow label="Path" helpText="Responder path should start with a '/', and should not end with a '/'">
            <EuiFieldText
              value={path}
              isInvalid={path.length > 0 && !isPathValid}
              required
              type={'text'}
              onChange={onPathChange}
              append={
                <EuiButtonIcon
                  iconType="refresh"
                  title={'Generate random path'}
                  aria-label="Generate random path"
                  onClick={() => setPath(`/${nanoidCustom().toLowerCase()}`)}
                />
              }
            />
          </EuiFormRow>
          {isAdvancedMode ? (
            <EuiFormRow
              label="Path type"
              helpText="Responder will respond to requests with the path that either matches the specified `Path` exactly or starts with it"
            >
              <EuiSelect options={PATH_TYPES} value={pathType} onChange={onPathTypeChange} />
            </EuiFormRow>
          ) : null}
          {isAdvancedMode ? (
            <EuiFormRow
              label="Method"
              helpText="Responder will only respond to requests with the specified HTTP method"
            >
              <EuiSelect options={httpMethods} value={method} onChange={onMethodChange} />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup
          title={<h3>Response</h3>}
          description={'Properties of the responder related to the HTTP response it generates'}
        >
          <EuiFormRow label="Status code" helpText="The HTTP status code to use for the response">
            <EuiFieldNumber fullWidth min={100} max={999} step={1} value={statusCode} onChange={onStatusCodeChange} />
          </EuiFormRow>
          <EuiFormRow
            label="Headers"
            helpText="Optional list of the HTTP response headers to use for the response, e.g `X-Header: X-Value`"
            fullWidth
          >
            <EuiComboBox
              fullWidth
              options={[
                { label: 'Cache-Control: no-cache, no-store, max-age=0, must-revalidate' },
                { label: 'Content-Type: application/javascript; charset=utf-8' },
                { label: 'Content-Type: application/json' },
                { label: 'Content-Type: text/css; charset=utf-8' },
                { label: 'Content-Type: text/html; charset=utf-8' },
                { label: 'Content-Type: text/plain; charset=utf-8' },
              ]}
              selectedOptions={headers}
              onCreateOption={onCreateHeader}
              onChange={onHeadersChange}
              onSearchChange={onHeadersSearchChange}
              isInvalid={areHeadersInvalid}
            />
          </EuiFormRow>
          <EuiFormRow label="Body" isDisabled={method === 'HEAD'}>
            <ScriptEditor
              onChange={(value) => setBody(value ?? '')}
              defaultValue={body}
              language={getBodyLanguage(headers)}
            />
          </EuiFormRow>
          {isAdvancedMode ? (
            <EuiFormRow
              label="Script"
              helpText={
                <span>
                  The script is executed within a constrained version of the{' '}
                  <EuiLink target="_blank" href="https://deno.com/">
                    <b>Deno JavaScript runtime</b>
                  </EuiLink>{' '}
                  for every received request. It returns an object that can override the default response status code,
                  headers, or body. Request information is available through the global &quot;context&quot; variable.
                  Refer to the{' '}
                  <EuiLink target="_blank" href="/docs/guides/webhooks#annex-responder-script-examples">
                    <b>documentation</b>
                  </EuiLink>{' '}
                  for a list of script examples, expected return value and properties available in the
                  &quot;context&quot; object argument. User secrets are available via
                  <b>context.secrets.MY_KEY</b>. For static body and headers, use <b>{'${secrets.MY_KEY}'}</b> syntax.
                </span>
              }
            >
              <ScriptEditor
                onChange={onUserScriptChange}
                defaultValue={script}
                snippets={SCRIPT_SNIPPETS}
                importActions={scriptImportActions}
              />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
        {isImportModalVisible ? (
          <ScriptImportSelector
            context="responder"
            onSelect={handleImportScript}
            onClose={() => setIsImportModalVisible(false)}
          />
        ) : null}
        {isAdvancedMode ? (
          <EuiDescribedFormGroup
            title={<h3>Secrets</h3>}
            description="Control which user secrets are available to this responder's script and template interpolation."
          >
            <EuiFormRow label="Access mode" helpText="Choose which secrets to expose to this responder." fullWidth>
              <EuiSelect
                fullWidth
                options={[
                  { value: 'none', text: 'No secrets' },
                  { value: 'all', text: 'All secrets' },
                  { value: 'selected', text: 'Selected secrets' },
                ]}
                value={secretsMode}
                onChange={(e) => setSecretsMode(e.target.value as 'none' | 'all' | 'selected')}
              />
            </EuiFormRow>
            {secretsMode === 'selected' ? (
              <EuiFormRow label="Secrets" helpText="Select the secrets to expose." fullWidth>
                <EuiComboBox
                  fullWidth
                  options={availableSecrets}
                  selectedOptions={selectedSecretNames}
                  onChange={setSelectedSecretNames}
                  isLoading={!secretsLoaded}
                />
              </EuiFormRow>
            ) : null}
          </EuiDescribedFormGroup>
        ) : null}
      </EuiForm>
    </EditorFlyout>
  );
}
