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
  EuiTextArea,
  EuiTitle,
} from '@elastic/eui';
import axios from 'axios';
import { customAlphabet, urlAlphabet } from 'nanoid';
import { useCallback, useMemo, useState } from 'react';
import type { ChangeEvent } from 'react';

import type { Responder } from './responder';
import { useRangeTicks } from '../../../../hooks';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage, isClientError } from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { ScriptEditor } from '../../components/script_editor';
import { useWorkspaceContext } from '../../hooks';

export interface ResponderEditFlyoutProps {
  responder?: Responder;
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

export function ResponderEditFlyout({ onClose, responder }: ResponderEditFlyoutProps) {
  const { addToast, uiState } = useWorkspaceContext();
  const maxTicks = useRangeTicks();

  const httpMethods = useMemo(() => HTTP_METHODS.map((method) => ({ value: method, text: method })), []);

  const [isAdvancedMode, setIsAdvancedMode] = useState(!!responder);

  const [name, setName] = useState<string>(responder?.name ?? '');
  const onNameChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
  }, []);

  const defaultRandom = useMemo(() => (!responder ? nanoidCustom().toLowerCase() : ''), [responder]);
  const supportsCustomSubdomainPrefixes =
    uiState.webhookUrlType === 'subdomain' && !!uiState.subscription?.features?.webhooks.responderCustomSubdomainPrefix;
  const [subdomainPrefix, setSubdomainPrefix] = useState<string>(
    responder?.location.subdomainPrefix ?? (supportsCustomSubdomainPrefixes ? defaultRandom : ''),
  );
  const onSubdomainPrefixChange = (e: ChangeEvent<HTMLInputElement>) => {
    setSubdomainPrefix(e.target.value.toLowerCase());
  };

  const [path, setPath] = useState<string>(
    // If custom subdomain prefixes are supported, then when a creating a new responder a random prefix will be
    // generated, so we safely default path to `/` (prefix).
    responder?.location.path ?? (supportsCustomSubdomainPrefixes ? '/' : `/${defaultRandom}`),
  );
  const onPathChange = (e: ChangeEvent<HTMLInputElement>) => {
    setPath(e.target.value.toLowerCase());
  };
  const isPathValid = path.startsWith('/') && (path.length === 1 || !path.endsWith('/'));

  const [pathType, setPathType] = useState<string>(
    responder?.location.pathType ?? (supportsCustomSubdomainPrefixes ? '^' : '='),
  );
  const onPathTypeChange = (e: ChangeEvent<HTMLSelectElement>) => {
    setPathType(e.target.value);
  };

  const [requestsToTrack, setRequestsToTrack] = useState<number>(
    responder?.settings.requestsToTrack ??
      Math.min(uiState.subscription?.features?.webhooks.responderRequests ?? 0, 10),
  );

  const [statusCode, setStatusCode] = useState<number>(responder?.settings.statusCode ?? 200);
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
    responder?.settings.headers?.map(([header, value]) => ({ label: `${header}: ${value}` })) ?? [
      { label: 'Content-Type: text/html; charset=utf-8' },
    ],
  );
  const [areHeadersInvalid, setAreHeadersInvalid] = useState(false);

  const [script, setScript] = useState<string | undefined>(responder?.settings.script);
  const onUserScriptChange = useCallback((value?: string) => {
    setScript(value);
  }, []);

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
    responder?.settings.body ?? 'Hello from <a href="https://secutils.dev">Secutils.dev</a>!',
  );
  const onBodyChange = useCallback((e: ChangeEvent<HTMLTextAreaElement>) => {
    setBody(e.target.value);
  }, []);

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();
  const onSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

    const locationSubdomainPrefix = supportsCustomSubdomainPrefixes ? subdomainPrefix || undefined : undefined;
    let location;
    if (responder) {
      location =
        responder.location.path !== path ||
        responder.location.pathType !== pathType ||
        responder.location.subdomainPrefix !== locationSubdomainPrefix
          ? { pathType, path: path.trim(), subdomainPrefix: locationSubdomainPrefix }
          : null;
    } else {
      location = { pathType, path: path.trim(), subdomainPrefix: locationSubdomainPrefix };
    }

    const responderToUpdate = {
      name: responder ? (responder.name !== name ? name.trim() : null) : name.trim(),
      location,
      method: responder ? (responder.method !== method ? method : null) : method,
      enabled: responder ? (responder.enabled !== isEnabled ? isEnabled : null) : isEnabled,
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
      },
    };

    const [requestPromise, successMessage, errorMessage] = responder
      ? [
          axios.put(
            getApiUrl(`/api/utils/webhooks/responders/${responder.id}`),
            responderToUpdate,
            getApiRequestConfig(),
          ),
          `Successfully updated "${name}" responder`,
          `Unable to update "${name}" responder, please try again later`,
        ]
      : [
          axios.post(getApiUrl('/api/utils/webhooks/responders'), responderToUpdate, getApiRequestConfig()),
          `Successfully saved "${name}" responder`,
          `Unable to save "${name}" responder, please try again later`,
        ];
    requestPromise.then(
      () => {
        setUpdatingStatus({ status: 'succeeded', data: undefined });

        addToast({
          id: `success-save-responder-${name}`,
          iconType: 'check',
          color: 'success',
          title: successMessage,
        });

        onClose(true);
      },
      (err: Error) => {
        const remoteErrorMessage = getErrorMessage(err);
        setUpdatingStatus({ status: 'failed', error: remoteErrorMessage });

        addToast({
          id: `failed-save-responder-${name}`,
          iconType: 'warning',
          color: 'danger',
          title: isClientError(err) ? remoteErrorMessage : errorMessage,
        });
      },
    );
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
    responder,
    updatingStatus,
    supportsCustomSubdomainPrefixes,
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
      canSave={
        name.trim().length > 0 &&
        !areHeadersInvalid &&
        isPathValid &&
        (!subdomainPrefix || isSubdomainPrefixValid(subdomainPrefix)) &&
        requestsToTrack >= 0 &&
        requestsToTrack <= 100
      }
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm id="update-form" component="form" fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description={'General properties of the responder'}>
          <EuiFormRow label="Name" helpText="Arbitrary responder name." fullWidth>
            <EuiFieldText autoFocus value={name} required type={'text'} onChange={onNameChange} />
          </EuiFormRow>
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
          {supportsCustomSubdomainPrefixes && (
            <EuiFormRow
              label="Subdomain prefix"
              helpText={
                <>
                  Responder will only respond to requests with the&nbsp;
                  <b>
                    {subdomainPrefix || '<subdomain-prefix>'}-{uiState.user?.handle ?? '<user-handle>'}
                    .webhooks.secutils.dev
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
          )}
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
            <EuiTextArea value={body} onChange={onBodyChange} />
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
                  &quot;context&quot; object argument.
                </span>
              }
            >
              <ScriptEditor onChange={onUserScriptChange} defaultValue={script} />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
