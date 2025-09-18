export async function getOryApi() {
  return await import('@ory/client').then(
    ({ Configuration, FrontendApi }) => new FrontendApi(new Configuration({ basePath: location.origin })),
  );
}

export interface OryError<TData> {
  name?: string;
  message?: string;
  response?: { status: number; data?: TData };
  isAxiosError: boolean;
}

export interface OryResponse<T> {
  data?: T;
}
