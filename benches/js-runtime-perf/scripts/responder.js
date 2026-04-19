// Responder-shaped script: reads `context.body`, does light JSON work, returns
// the standard `{ body, headers, statusCode }` envelope. The body is encoded
// to a `Uint8Array` inside the script so this scenario compiles against both
// the pre-change `execute_script` (no body normalisation) and the post-change
// runtime (where the built-in normalisation is a pass-through for Uint8Array).
(async () => {
  const input = JSON.parse(Deno.core.decode(new Uint8Array(context.body)));
  const response = {
    ok: true,
    received: input.items.length,
    total: input.items.reduce((acc, item) => acc + item.value, 0),
  };
  return {
    body: Deno.core.encode(JSON.stringify(response)),
    headers: { 'content-type': 'application/json', 'x-responder': 'perf' },
    statusCode: 200,
  };
})();
