import type {
  GenericError,
  LoginFlow,
  RegistrationFlow,
  SettingsFlow,
  UiNodeInputAttributes,
  VerificationFlow,
} from '@ory/client';

import type { OryError, OryResponse } from '../tools/ory';

export function getCsrfToken(flow: LoginFlow | RegistrationFlow | SettingsFlow | VerificationFlow) {
  const csrfNode = flow.ui.nodes.find(
    (node) => node.attributes.node_type === 'input' && node.attributes.name === 'csrf_token',
  );
  return csrfNode ? ((csrfNode.attributes as UiNodeInputAttributes).value as string) : undefined;
}

export function getSecurityErrorMessage(resOrErr: unknown) {
  const response = isOryUiError(resOrErr) ? resOrErr.response : isOryResponse(resOrErr) ? resOrErr : undefined;
  if (!response) {
    return isOryGenericError(resOrErr)
      ? resOrErr.response?.data?.reason || resOrErr.response?.data?.message
      : undefined;
  }

  for (const node of response.data?.ui.nodes ?? []) {
    for (const message of node.messages) {
      if (message.type === 'error') {
        return message.text;
      }
    }
  }

  return response.data?.ui.messages?.find((message) => message.type === 'error')?.text;
}

function isOryResponse(res: unknown): res is OryResponse<LoginFlow | RegistrationFlow> {
  const forceCastedRes = res as { data?: LoginFlow | RegistrationFlow };
  return Array.isArray(forceCastedRes.data?.ui?.nodes);
}

export function isOryError<TData = { error?: { id: string; message: string; reason: string } }>(
  err: unknown,
): err is OryError<TData> {
  return !!err && typeof err === 'object' && 'isAxiosError' in err;
}

function isOryUiError(err: unknown): err is OryError<LoginFlow | RegistrationFlow> {
  return isOryError<LoginFlow | RegistrationFlow>(err) && Array.isArray(err.response?.data?.ui?.nodes);
}

function isOryGenericError(err: unknown): err is OryError<GenericError> {
  return isOryError<GenericError>(err) && !!err.response?.data?.message;
}
