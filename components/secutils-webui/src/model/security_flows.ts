import type {
  GenericError,
  LoginFlow,
  RegistrationFlow,
  SettingsFlow,
  UiNodeInputAttributes,
  VerificationFlow,
} from '@ory/client';

import type { OryError } from '../tools/ory';

export function getCsrfToken(flow: LoginFlow | RegistrationFlow | SettingsFlow | VerificationFlow) {
  const csrfNode = flow.ui.nodes.find(
    (node) => node.attributes.node_type === 'input' && node.attributes.name === 'csrf_token',
  );
  return csrfNode ? ((csrfNode.attributes as UiNodeInputAttributes).value as string) : undefined;
}

export function getSecurityErrorMessage(err: unknown) {
  const response = isOryUiError(err) ? err.response : isOryResponse(err) ? err : undefined;
  if (!response) {
    return isOryGenericError(err) ? err.response?.data?.reason || err.response?.data?.message : undefined;
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

function isOryUiError(err: unknown): err is OryError<LoginFlow | RegistrationFlow> {
  const forceCastedError = err as OryError<LoginFlow | RegistrationFlow>;
  return forceCastedError.isAxiosError && Array.isArray(forceCastedError.response?.data?.ui?.nodes);
}

function isOryGenericError(err: unknown): err is OryError<GenericError> {
  const forceCastedError = err as OryError<GenericError>;
  return forceCastedError.isAxiosError && !!forceCastedError.response?.data?.message;
}

function isOryResponse(err: unknown): err is OryError<LoginFlow | RegistrationFlow> {
  const forceCastedError = err as { data?: LoginFlow | RegistrationFlow };
  return Array.isArray(forceCastedError.data?.ui?.nodes);
}
