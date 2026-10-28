import { createCipheriv, createECDH, createHash, hkdfSync, randomBytes } from 'node:crypto';
import * as vm from 'node:vm';

interface Response {
  statusCode: number;
  headers: Record<string, string>;
  body: string;
}

// Runs the real responder script with an in-memory KV and Node's sealed-box
// equivalent. The browser still generates keys and decrypts the captured records.
export function webhookBackend(script: string) {
  const values = new Map<string, string>();
  const expiries = new Map<string, number>();
  const live = (key: string) => values.has(key) && (expiries.get(key) ?? Infinity) > Date.now() / 1000;
  const kv = {
    get: async (key: string) => (live(key) ? values.get(key) : null),
    getEntry: async (key: string) =>
      live(key) ? { value: values.get(key), expiresAt: expiries.get(key) ?? null } : null,
    set: async (key: string, value: string, options: { ifAbsent?: boolean; ttlSec?: number } = {}) => {
      if (options.ifAbsent && live(key)) return;
      values.set(key, value);
      if (options.ttlSec) expiries.set(key, Math.floor(Date.now() / 1000) + options.ttlSec);
    },
    delete: async (key: string) => {
      expiries.delete(key);
      return values.delete(key);
    },
    list: async (options: { prefix?: string; after?: string; limit?: number; valuesIncluded?: boolean } = {}) => {
      const entries = [...values.keys()]
        .filter((key) => live(key) && key.startsWith(options.prefix ?? '') && (!options.after || key > options.after))
        .sort()
        .slice(0, options.limit ?? 200)
        .map((key) => ({ key, createdAt: 0, value: options.valuesIncluded === false ? null : values.get(key) }));
      return { entries, cursor: entries.at(-1)?.key ?? null, timedOut: false };
    },
    // UI checks use immediate lists instead of a server's parked long poll.
    watch: async (options: { prefix?: string; after?: string; limit?: number }) => kv.list(options),
  };

  const secutils = {
    kv,
    crypto: {
      sha256: async (value: string) => createHash('sha256').update(Buffer.from(value, 'base64url')).digest('hex'),
      seal: async (recipient: string, plaintext: string) => {
        const publicKey = Buffer.from(recipient, 'base64url');
        const ephemeral = createECDH('prime256v1');
        const ephemeralPublicKey = ephemeral.generateKeys();
        const key = Buffer.from(
          hkdfSync('sha256', ephemeral.computeSecret(publicKey), publicKey, ephemeralPublicKey, 32),
        );
        const iv = randomBytes(12);
        const cipher = createCipheriv('aes-256-gcm', key, iv);
        const ciphertext = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final(), cipher.getAuthTag()]);
        return Buffer.concat([ephemeralPublicKey, iv, ciphertext]).toString('base64url');
      },
    },
  };

  return {
    values,
    run: async (url: URL, method = 'GET', body = new Uint8Array(), headers: Record<string, string> = {}) => {
      const sandbox = {
        context: {
          method,
          path: url.pathname,
          query: Object.fromEntries(url.searchParams),
          rawQuery: url.search.slice(1),
          headers,
          body,
          clientAddress: '127.0.0.1:0',
        },
        secutils,
        Deno: {
          core: {
            decode: (bytes: Uint8Array) => new TextDecoder().decode(bytes),
            encode: (text: string) => new TextEncoder().encode(text),
          },
        },
      };
      return (await vm.runInNewContext(script, sandbox, { timeout: 1000 })) as Response | null;
    },
  };
}
