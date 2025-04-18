import type {
  GenericError,
  LoginFlow,
  RegistrationFlow,
  SettingsFlow,
  UiNodeInputAttributes,
  VerificationFlow,
} from '@ory/client';
import type { AxiosError, AxiosResponse } from 'axios';

export function getCsrfToken(flow: LoginFlow | RegistrationFlow | SettingsFlow | VerificationFlow) {
  const csrfNode = flow.ui.nodes.find(
    (node) => node.attributes.node_type === 'input' && node.attributes.name === 'csrf_token',
  );
  return csrfNode ? ((csrfNode.attributes as UiNodeInputAttributes).value as string) : undefined;
}

export function getSecurityErrorMessage(err: unknown) {
  const response = isKratosUiError(err) ? err.response : isKratosResponse(err) ? err : undefined;
  if (!response) {
    return isKratosGenericError(err) ? err.response?.data.reason || err.response?.data.message : undefined;
  }

  for (const node of response.data.ui.nodes ?? []) {
    for (const message of node.messages) {
      if (message.type === 'error') {
        return message.text;
      }
    }
  }

  return response.data.ui.messages?.find((message) => message.type === 'error')?.text;
}

function isKratosUiError(err: unknown): err is AxiosError<LoginFlow | RegistrationFlow> {
  const forceCastedError = err as AxiosError<LoginFlow | RegistrationFlow>;
  return forceCastedError.isAxiosError && Array.isArray(forceCastedError.response?.data?.ui?.nodes);
}

function isKratosGenericError(err: unknown): err is AxiosError<GenericError> {
  const forceCastedError = err as AxiosError<GenericError>;
  return forceCastedError.isAxiosError && !!forceCastedError.response?.data?.message;
}

function isKratosResponse(err: unknown): err is AxiosResponse<LoginFlow | RegistrationFlow> {
  const forceCastedError = err as AxiosResponse<LoginFlow | RegistrationFlow>;
  return Array.isArray(forceCastedError.data?.ui?.nodes);
}
