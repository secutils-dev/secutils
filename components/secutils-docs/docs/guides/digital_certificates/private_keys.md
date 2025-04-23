---
sidebar_position: 2
sidebar_label: Private Keys
title: Digital Certificates ➔ Private keys
description: Learn how to create and use private keys in Secutils.dev.
---

# What is a private key?

A private key is a sensitive piece of cryptographic information that is used in asymmetric encryption systems, such as RSA or ECC (Elliptic Curve Cryptography). In these systems, a pair of keys is used: a public key and a private key.

The private key is kept secret and is known only to the owner. It's used to decrypt data that has been encrypted with its corresponding public key. Additionally, the private key is used to sign digital messages, ensuring that they came from the owner of the private key and have not been tampered with.

On this page, you can find guides on creating private keys with parameters that match your specific needs.

## Generate an RSA private key

In this guide, you'll create the simplest possible RSA key and verify its validity with the OpenSSL command-line tool:

1. Navigate to [Digital Certificates → Private keys](https://secutils.dev/ws/certificates__private_keys) and click **Create private key** button
2. Configure a new private key with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
RSA
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
<td><b>Encryption</b></td>
<td>
```
None
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the private key
4. Once the key is set up, it will appear in the private keys grid
5. Click the key's **Export** button and use the following values for the export:

<table class="su-table">
<tbody>
<tr>
<td><b>Format</b></td>
<td>
```
PEM
```
</td>
</tr>
<tr>
<td><b>Encryption</b></td>
<td>
```
None
```
</td>
</tr>
</tbody>
</table>

6. Click the **Export** button to generate and download the private key as `RSA.pem`
7. Use the OpenSSL command-line tool to view the key's content and verify its validity:

```bash title="View the RSA key's content"
$ openssl rsa -in ~/Downloads/RSA.pem | openssl pkey -inform PEM -text -noout
writing RSA key
Private-Key: (2048 bit, 2 primes)
modulus:
    00:c4:96:a7:80:e4:45:19:47:3f:55:48:0e:eb:da:
    ...
publicExponent: 65537 (0x10001)
privateExponent:
    2d:c0:94:3e:4a:a2:0c:46:89:26:5b:6d:61:95:cd:
    ...
prime1:
    00:f9:9f:52:03:48:2d:bf:a7:c1:9a:e5:68:51:7d:
    ...
prime2:
    00:c9:9c:75:f6:ab:49:4a:6b:85:6b:61:cc:04:20:
    ...
exponent1:
    00:be:75:85:49:e3:c4:a4:3b:07:49:7c:48:40:05:
    ...
exponent2:
    00:94:db:de:49:8b:fc:e8:62:ed:36:f5:15:92:f2:
    ...
coefficient:
    27:bf:26:e8:31:41:0c:2f:88:c7:5e:2d:af:46:c4:
    ...
```

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/digital_certificates_private_keys_rsa.webm" type="video/webm" />
  <source src="../../video/guides/digital_certificates_private_keys_rsa.mp4" type="video/mp4" />
</video>

## Generate an ECDSA elliptic curve private key

In this guide, you'll generate an ECDSA elliptic curve private key protected by a passphrase:

1. Navigate to [Digital Certificates → Private keys](https://secutils.dev/ws/certificates__private_keys) and click **Create private key** button
2. Configure a new private key with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
ECC
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
<td><b>Encryption</b></td>
<td>
```
Passphrase
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
<tr>
<td><b>Repeat passphrase</b></td>
<td>
```
pass
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the private key
4. Once the key is set up, it will appear in the private keys grid
5. Click the key's **Export** button and use the following values for the export:

<table class="su-table">
<tbody>
<tr>
<td><b>Format</b></td>
<td>
```
PKCS#8
```
</td>
</tr>
<tr>
<td><b>Encryption</b></td>
<td>
```
Passphrase
```
</td>
</tr>
<tr>
<td><b>Current passphrase</b></td>
<td>
```
pass
```
</td>
</tr>
<tr>
<td><b>Export passphrase</b></td>
<td>
```
pass-export
```
</td>
</tr>
<tr>
<td><b>Repeat export passphrase</b></td>
<td>
```
pass-export
```
</td>
</tr>
</tbody>
</table>

6. Click the **Export** button to generate and download the private key as `ECC.p8`
7. Use the OpenSSL command-line tool to view the key's content and verify its validity:

```bash title="View the ECDSA key's content"
$ openssl pkcs8 -inform DER -in ~/Downloads/ECC.p8 -passin pass:pass-export | \
    openssl pkey -inform PEM -text -noout
Private-Key: (384 bit)
priv:
    8c:30:d7:b2:df:7c:9d:75:cb:a0:ec:93:53:ea:91:
    ...
pub:
    04:f8:94:f2:28:f7:be:e7:75:ff:8d:3a:0d:c9:d3:
    ...
ASN1 OID: secp384r1
NIST CURVE: P-384
```

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/digital_certificates_private_keys_ecdsa.webm" type="video/webm" />
  <source src="../../video/guides/digital_certificates_private_keys_ecdsa.mp4" type="video/mp4" />
</video>
