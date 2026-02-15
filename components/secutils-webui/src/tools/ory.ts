export async function getOryApi() {
  return await import('@ory/kratos-client-fetch').then(
    ({ Configuration, FrontendApi }) =>
      new FrontendApi(
        new Configuration({ basePath: location.origin, headers: { accept: 'application/json, text/plain, */*' } }),
      ),
  );
}
