// Defensive empty stub for Node-only modules we don't expect to be touched at
// runtime in the browser (node:fs, node:child_process, ...). Accessing any
// property other than the ESM marker throws a clearly-scoped error so we can
// surface unexpected Node code paths instead of a confusing "x is undefined"
// later in liteparse. Identical strategy to simonw/liteparse@web.
const handler: ProxyHandler<object> = {
  get(_t, prop) {
    if (prop === "__esModule") return true;
    if (prop === Symbol.toPrimitive || prop === Symbol.toStringTag) return undefined;
    throw new Error(
      `[liteparse-browser] accessed stubbed Node module property: ${String(prop)}`,
    );
  },
};
const stub = new Proxy({}, handler);
export default stub;
export const promises = stub;
export const constants = stub;
export const createReadStream = () => {
  throw new Error("createReadStream is not available in the browser");
};
export const readFile = () => {
  throw new Error("fs.readFile is not available in the browser");
};
export const writeFile = () => {
  throw new Error("fs.writeFile is not available in the browser");
};
export const spawn = () => {
  throw new Error("child_process.spawn is not available in the browser");
};
