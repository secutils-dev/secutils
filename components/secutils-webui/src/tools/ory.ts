export async function getOryApi() {
  return await import('@ory/client').then(
    ({ Configuration, FrontendApi }) => new FrontendApi(new Configuration({ basePath: location.origin })),
  );
}
