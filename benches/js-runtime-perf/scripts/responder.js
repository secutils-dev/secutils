// Responder-shaped script: reads `context.body`, does light JSON work, returns
// the standard `{ body, headers, statusCode }` envelope. Designed to exercise
// the same JS→Rust conversion path that `wrap_script_with_body_conversion`
// wraps around user-supplied responder scripts.
(async () => {
  const input = JSON.parse(Deno.core.decode(new Uint8Array(context.body)));
  const response = {
    ok: true,
    received: input.items.length,
    total: input.items.reduce((acc, item) => acc + item.value, 0),
  };
  return {
    body: response,
    headers: { 'content-type': 'application/json', 'x-responder': 'perf' },
    statusCode: 200,
  };
})();
