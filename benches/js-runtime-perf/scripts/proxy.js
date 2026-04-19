// Proxy script: invokes `op_proxy_request` against a local httpmock server.
// This isolates the cost of re-creating the reqwest client + TLS/DNS setup
// per call in Secutils' current implementation.
(async () => {
  const response = await Deno.core.ops.op_proxy_request({ url: context.url });
  return {
    status: response.statusCode,
    bytes: response.body.length,
  };
})();
