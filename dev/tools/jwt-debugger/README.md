# JWT browser bundle

The JWT Debugger embeds `jose` 6.2.18 for HMAC signing and verification.
The bundle includes the MIT license notice and requires no CDN request.
Version 6 uses ES modules instead of the previous UMD build.

## Build

```bash
npm ci
npm test
```

The build produces `dist/jose.js` with `esbuild` 0.28.2.
The existing `data-su-bundle="jose"` mechanism embeds this file into the HTML response.
The page imports the bundle on the first signing or verification action.
