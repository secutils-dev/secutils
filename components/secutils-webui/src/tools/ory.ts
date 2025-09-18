export async function getOryApi() {
  return await import('@ory/client').then(
    ({ Configuration, FrontendApi }) => new FrontendApi(new Configuration({ basePath: location.origin })),
  );
}

export interface OryError<T = { message: string }> {
  name?: string;
  data?: T;
  response?: { status: number; data?: T };
  isAxiosError: boolean;
}
