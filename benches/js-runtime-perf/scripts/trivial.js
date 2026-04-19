// Trivial script: minimal work, used to measure runtime/isolate startup and
// per-call overhead in the Secutils JsRuntime.
(async () => {
  return 1 + 1;
})();
