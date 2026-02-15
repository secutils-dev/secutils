import type {
  GenericError,
  LoginFlow,
  RecoveryFlow,
  RegistrationFlow,
  SettingsFlow,
  UiContainer,
  UiNodeInputAttributes,
  VerificationFlow,
} from '@ory/kratos-client-fetch';
import { FetchError, ResponseError } from '@ory/kratos-client-fetch';

export function getCsrfToken(flow: LoginFlow | RegistrationFlow | SettingsFlow | VerificationFlow) {
  const csrfNode = flow.ui.nodes.find(
    (node) => node.attributes.node_type === 'input' && node.attributes.name === 'csrf_token',
  );
  return csrfNode ? ((csrfNode.attributes as UiNodeInputAttributes).value as string) : undefined;
}

export async function getSecurityErrorMessage(resOrErr: unknown) {
  const error = resOrErr instanceof ResponseError ? await resOrErr.response.json() : resOrErr;
  if (!isOryUiResponse(error)) {
    if (error instanceof FetchError) {
      return error.message ?? 'Network error occurred';
    }

    if (isOryGenericError(error)) {
      return error.reason || error.message || 'Unknown error occurred';
    }

    return (error as Error)?.message || 'Unknown error occurred';
  }

  const uiContainer = (error as { ui?: UiContainer })?.ui;
  for (const node of uiContainer?.nodes ?? []) {
    for (const message of node.messages) {
      if (message.type === 'error') {
        return message.text;
      }
    }
  }

  return uiContainer?.messages?.find((message) => message.type === 'error')?.text;
}

function isOryUiResponse(
  res: unknown,
): res is LoginFlow | RegistrationFlow | RecoveryFlow | VerificationFlow | SettingsFlow {
  const forceCastedRes = res as { ui?: UiContainer };
  return Array.isArray(forceCastedRes?.ui?.nodes);
}

function isOryGenericError(err: unknown): err is GenericError {
  return !!err && typeof err === 'object' && 'reason' in err;
}
