---
name: pem-certificate-decoder
description: >-
  Decode and inspect PEM-encoded X.509 certificate chains in the browser with
  the Secutils.dev PEM Decoder. Build a one-click prefilled URL the user can
  open by encoding the raw PEM string into the fragment of
  https://tools.secutils.dev/pem#{encoded}. Trigger when the user asks to
  "decode this certificate", "inspect a PEM chain", "show the SANs in this
  cert", "what's the issuer of this PEM", or anything that names
  secutils.dev/pem.
---

# PEM Certificate Decoder (Secutils.dev)

Hand the user a URL that opens the PEM Decoder pre-loaded with their chain.
Parsing happens in the browser - the fragment is never sent to the server, so
even a privacy-leaking certificate (or a misplaced private key, see Caveats)
stays client-side. The tool auto-sorts chain order (leaf first) and surfaces
subject / issuer / SANs / validity / key algorithm and size / signature
algorithm / SHA-1 + SHA-256 fingerprints per cert.

## Inputs

| Field | Type   | Default  | Notes                                                                                                                       |
|-------|--------|----------|-----------------------------------------------------------------------------------------------------------------------------|
| `pem` | string | required | One or more concatenated `-----BEGIN CERTIFICATE-----` ... `-----END CERTIFICATE-----` blocks. Chain order does not matter. |

State shape: a **raw string** - no JSON wrapper. The state is exactly the
PEM text the user supplied (including newlines between blocks).

## Wire format

After URL-safe base64 decoding:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 string |
```

Pipeline: take the PEM string verbatim → UTF-8 bytes → `deflate-raw` →
prepend the 4-byte LE u32 of the **uncompressed** length → base64url
(`+`→`-`, `/`→`_`, strip `=`).

## How to produce the URL

Run this on any machine with Node ≥ 18 (no deps):

```bash
node -e '
const zlib = require("node:zlib");
const fs   = require("node:fs");
const pem  = process.argv[1] === "-" ? fs.readFileSync(0, "utf8") : process.argv[1];
const utf8 = Buffer.from(pem, "utf8");
const out  = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc  = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/pem#" + enc);
' -  <<'EOF'
-----BEGIN CERTIFICATE-----
MIIDXTCCAkWgAwIBAgIJAKl...
-----END CERTIFICATE-----
EOF
```

Read PEM from stdin (heredoc) so the multi-line block with embedded `\n`
survives untouched. The fragment is opaque - print the full URL.

The same `tools.secutils.dev/pem` URL with **no fragment** loads an empty
decoder if you'd rather just direct the user to paste themselves.

## Decoding without opening the tool (optional)

If the user only wants a textual summary in chat (no UI), parse the PEM
locally with any X.509 library: `openssl x509 -in cert.pem -text -noout`,
Node `crypto.X509Certificate`, Python `cryptography`, etc. The shareable URL
is most useful when the user wants the visual chain ladder + SAN list.

## After producing

Hand the URL back in a fenced block or inline code. Keep summaries to one
sentence; don't paraphrase certificate fields.

## Caveats

- The PEM travels **in the URL fragment**. The fragment never reaches the
  Secutils server but is fully visible to anyone with the link. Rare but
  catastrophic: if the user accidentally includes a `-----BEGIN PRIVATE KEY-----`
  block, refuse to encode and tell them to strip it first.
- DER-encoded (binary) certs are not supported - base64-wrap them in PEM
  envelopes first (`openssl x509 -inform DER -in cert.der -out cert.pem`).
- Signature verification against the issuer is **out of scope for this tool**.
  Use a server-side library (or `openssl verify`) for trust-chain validation.
