---
sidebar_position: 1
sidebar_label: Certificate Templates
title: Digital Certificates ➔ Certificate templates
description: Learn how to create and use digital certificate templates in Secutils.dev.
---

# What is a digital certificate?

A digital certificate, also known as an SSL/TLS certificate or public key certificate, is a digital document that verifies the identity of a website, server, or other digital entity, and allows secure communication between two parties by encrypting data sent over the internet. It contains information about the identity of the certificate holder, such as their name and public key, and is issued by a trusted third-party Certificate Authority (CA).

There are different types of digital certificates that can be generated with various parameters. Certificates can be password-protected, can be bundled with the keys, can rely on different cryptographic algorithms, and eventually expire. Considering these factors, it can be challenging to develop and test web applications that rely on digital certificates.

On this page, you can find guides on creating digital certificate templates with parameters that match your specific needs.

## Generate a key pair for a HTTPS server

In this guide you'll create a template for generating a private key and self-signed certificate for a Node.js HTTPS server:

1. Navigate to [Digital Certificates → Certificate templates](https://secutils.dev/ws/certificates__certificate_templates) and click **Create certificate template** button
2. Configure a new certificate template with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
https-server
```
</td>
</tr>
<tr>
<td><b>Key algorithm</b></td>
<td>
```
RSA
```
</td>
</tr>
<tr>
<td><b>Key size</b></td>
<td>
```
2048 bit
```
</td>
</tr>
<tr>
<td><b>Signature algorithm</b></td>
<td>
```
SHA-256
```
</td>
</tr>
<tr>
<td><b>Certificate type</b></td>
<td>
```
End Entity
```
</td>
</tr>
<tr>
<td><b>Key usage</b></td>
<td>
```
Key encipherment, Digital signature
```
</td>
</tr>
<tr>
<td><b>Extended key usage</b></td>
<td>
```
TLS Web server authentication
```
</td>
</tr>
<tr>
<td><b>Common name (CN)</b></td>
<td>
```
localhost
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the certificate template
4. Once the template is set up, it will appear in the templates grid
5. Click the template's **Generate** button and use the following values for generation:

<table class="su-table">
<tbody>
<tr>
<td><b>Format</b></td>
<td>
```
PKCS#12
```
</td>
</tr>
<tr>
<td><b>Passphrase</b></td>
<td>
```
pass
```
</td>
</tr>
</tbody>
</table>

6. Click the **Generate** button to generate and download the certificate bundle
7. Use the downloaded `https-server.pfx` file to configure Node.js HTTPS server:

```js title="index.js"
(async function main() {
    const https = await import('node:https');
    const fs = await import('node:fs');
    
    const httpsOptions = {
        // highlight-start
        // The name of the certificate bundle and the passphrase that was set in the generation dialog
        pfx: fs.readFileSync('https-server.pfx'),
        passphrase: 'pass'
        // highlight-end
    };
    
    https.createServer(httpsOptions, (req, res) => {
        res.writeHead(200);
        res.end('Hello World\n');
    }).listen(8000);
    
    console.log(`Listening on https://localhost:8000`);
})();
```

8. Run the server with and query it with the **cURL** or similar HTTP client:

```bash title="Example commands"
// Start server
$ node index.js 
Listening on https://localhost:8000

// Query the server with cURL
$ curl -kv https://localhost:8000
*   Trying 127.0.0.1:8000...
...
* Server certificate:
*  subject: CN=localhost; C=US; ST=California; L=San Francisco; O=CA Issuer, Inc
*  ...
*  issuer: CN=localhost; C=US; ST=California; L=San Francisco; O=CA Issuer, Inc
*  SSL certificate verify result: self-signed certificate (18), continuing anyway.
...
> GET / HTTP/1.1
> Host: localhost:8000
> User-Agent: curl/7.88.1
> ...
< HTTP/1.1 200 OK
< ....
< 
Hello World
```

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/digital_certificates_certificate_templates_https_server.webm" type="video/webm" />
  <source src="../../video/guides/digital_certificates_certificate_templates_https_server.mp4" type="video/mp4" />
</video>

## Export a private key as a JSON Web Key (JWK)

In this guide, you will generate a private key in PKCS#8 format and then export it to a JSON Web Key (JWK) using a custom responder and the browser's built-in Web Crypto API:

1. Navigate to [Digital Certificates → Certificate templates](https://secutils.dev/ws/certificates__certificate_templates) and click **Create certificate template** button
2. Configure a new certificate template with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
jwk
```
</td>
</tr>
<tr>
<td><b>Key algorithm</b></td>
<td>
```
ECDSA
```
</td>
</tr>
<tr>
<td><b>Curve name</b></td>
<td>
```
secp384r1 / NIST P-384
```
</td>
</tr>
<tr>
<td><b>Signature algorithm</b></td>
<td>
```
SHA-256
```
</td>
</tr>
<tr>
<td><b>Certificate type</b></td>
<td>
```
End Entity
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the certificate template
4. Once the template is set up, it will appear in the templates grid
5. Click the template's **Generate** button and use the following values for generation:

<table class="su-table">
<tbody>
<tr>
<td><b>Format</b></td>
<td>
```
PKCS#8 (private key only)
```
</td>
</tr>
</tbody>
</table>

6. Click the **Generate** button to generate and download the private key as `jwk.p8`
7. Now, navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
8. Configure a new responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Subtle Crypto
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/subtle-crypto
```
</td>
</tr>
<tr>
<td><b>Method</b></td>
<td>
```
GET
```
</td>
</tr>
<tr>
    <td><b>Headers</b></td>
<td>

```http
Content-Type: text/html; charset=utf-8
```
</td>
</tr>
<tr>
    <td><b>Body</b></td>
<td>

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <title>Subtle Crypto</title>
  <style>
    .center { text-align: center }
    pre {
      outline: 1px solid #ccc;
      padding: 5px;
      margin: 1em auto;
      width: 30%;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  </style>
  <script type="text/javascript">
    document.addEventListener("DOMContentLoaded", async function main() {
      document.getElementById("p8_upload").addEventListener("change", (e) => {
        if (e.target.files.length === 0) {
          return;
        }

        const reader = new FileReader();
        reader.onload = async () => {
          // Import generated PKCS#8 key as SubtleCrypto's CryptoKey.
          const cryptoKey = await window.crypto.subtle.importKey(
              "pkcs8",
              new Uint8Array(reader.result),
              { name: "ECDSA", namedCurve: "P-384" },
              true,
              ["sign"]
          )

          // Export CryptoKey as JWK and render it.
          document.getElementById("jwk").textContent = JSON.stringify(
              await window.crypto.subtle.exportKey('jwk', cryptoKey),
              null,
              2
          );
        };
        reader.readAsArrayBuffer(e.target.files[0]);
      });
    });
  </script>
</head>
<body>
<h1 class="center">PKCS#8 ➡ JSON Web Key (JWK)</h1>
<div class="center">
  <label for="p8_upload">Choose PKCS#8 key (*.p8)</label>
  <input
      type="file"
      id="p8_upload"
      name="p8_upload"
      accept=".p8" />
  <br />
</div>
<pre id="jwk">No PKCS#8 key is loaded yet...</pre>
</body>
</html>
```
</td>
</tr>
</tbody>
</table>

9. Click the **Save** button to save the responder
10. Once the responder is set up, it will appear in the responders grid along with its unique URL
11. Click on the responder's URL, upload the `jwk.p8` file downloaded at the step 6, and observe that it renders a JSON Web Key (JWK) derived from your ECDSA key

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/digital_certificates_certificate_templates_jwk_export.webm" type="video/webm" />
  <source src="../../video/guides/digital_certificates_certificate_templates_jwk_export.mp4" type="video/mp4" />
</video>

## Share a certificate template

This guide will walk you through sharing a certificate template publicly, allowing anyone on the internet to view it:

1. Navigate to [Digital Certificates → Certificate templates](https://secutils.dev/ws/certificates__certificate_templates) and pick the template you'd like to share
2. Click the template's **Share template** button and toggle **Share template** switch to **on** position
3. Once the template is shared, the dialog will show a **Copy link** button
4. Click the **Copy link** button to copy a unique shared template link to your clipboard
5. To stop sharing the template, click the **Share template** button again, and switch the **Share template** toggle to the **off** position.

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/digital_certificates_certificate_templates_template_share.webm" type="video/webm" />
  <source src="../../video/guides/digital_certificates_certificate_templates_template_share.mp4" type="video/mp4" />
</video>
