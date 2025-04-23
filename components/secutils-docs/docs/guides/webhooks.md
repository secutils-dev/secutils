---
sidebar_position: 1
sidebar_label: Webhooks
title: Webhooks
description: Learn how to create and use webhooks in Secutils.dev.
---

# What is a webhook?

A **webhook** is a mechanism that enables an application to receive automatic notifications or data updates by sending a request to a specified URL when a particular event or trigger occurs.

There are various types of webhooks that serve different purposes. One such type is the responder, which is a special webhook that responds to requests with a certain predefined response. A responder is a handy tool when you need to simulate an HTTP endpoint that's not yet implemented or even create a quick ["honeypot"](https://en.wikipedia.org/wiki/Honeypot_(computing)) endpoint. Responders can also serve as a quick and easy way to test HTML, JavaScript, and CSS code.

On this page, you can find several guides on how to create different types of responders.

:::tip NOTE

Each user on  [**secutils.dev**](https://secutils.dev) is assigned a randomly generated dedicated subdomain. This subdomain can host user-specific responders at any path, including the root path. For instance, if your dedicated subdomain is `abcdefg`, creating a responder at `/my-responder` would make it accessible via `https://abcdefg.webhooks.secutils.dev/my-responder`.

:::

## Return a static HTML page

In this guide you'll create a simple responder that returns a static HTML page:

1. Navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a new responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
HTML Responder
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/html-responder
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
    <title>My HTML responder</title>
</head>
<body>Hello World</body>
</html>
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the responder
4. Once the responder is set up, it will appear in the responders grid along with its unique URL
5. Click the responder's URL and observe that it renders text `Hello World`

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../video/guides/webhooks_html_responder.webm" type="video/webm" />
  <source src="../video/guides/webhooks_html_responder.mp4" type="video/mp4" />
</video>

## Emulate a JSON API endpoint

In this guide you'll create a simple responder that returns a JSON value:

1. Navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a new responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
JSON Responder
```
</td></tr>
<tr>
<td><b>Path</b></td>
<td>
```
/json-responder
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
Content-Type: application/json
```
</td>
</tr>
<tr>
    <td><b>Body</b></td>
<td>

```json
{
  "message": "Hello World"
}
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the responder
4. Once the responder is set up, it will appear in the responders grid along with its unique URL
5. Click the responder's URL and use an HTTP client, like **cURL**, to verify that it returns a JSON value

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../video/guides/webhooks_json_responder.webm" type="video/webm" />
  <source src="../video/guides/webhooks_json_responder.mp4" type="video/mp4" />
</video>

## Use the honeypot endpoint to inspect incoming requests

In this guide, you'll create a responder that returns an HTML page with custom Iframely meta-tags, providing a rich preview in Notion. Additionally, the responder will track the five most recent incoming requests, allowing you to see exactly how Notion communicates with the responder's endpoint:

1. Navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a new responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Notion Honeypot
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/notion-honeypot
```
</td>
</tr>
<tr>
<td><b>Tracking</b></td>
<td>
```
5
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
  <meta property="iframely:image"
        content="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" />
  <meta property="iframely:description"
        content="Inspect incoming HTTP request headers and body with the honeypot endpoint" />
  <title>My HTML responder</title>
</head>
<body>Hello World</body>
</html>
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the responder
4. Once the responder is set up, it will appear in the responders grid along with its unique URL
5. Copy responder's URL and try to create a bookmark for it in Notion
6. Note that the bookmark includes both the description and image retrieved from the rich meta-tags returned by the responder
7. Go back to the responder's grid and expand the responder's row to view the incoming requests it has already tracked

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../video/guides/webhooks_tracking_responder.webm" type="video/webm" />
  <source src="../video/guides/webhooks_tracking_responder.mp4" type="video/mp4" />
</video>

## Generate a dynamic response

In this guide, you'll build a responder that uses a custom JavaScript script to generate a dynamic response based on the request's query string parameter:

:::info NOTE

The script should be provided in the form of an [Immediately Invoked Function Expression (IIFE)](https://developer.mozilla.org/en-US/docs/Glossary/IIFE). It runs within a restricted version of the [Deno JavaScript runtime](https://deno.com/) for each incoming request, producing an object capable of modifying the default response's status code, headers, or body. Request details are accessible through the global `context` variable. Refer to the [Annex: Responder script examples](/docs/guides/webhooks#annex-responder-script-examples) for a list of script examples, expected return value and properties available in the global `context` variable.

:::

1. Navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a new responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Dynamic
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/dynamic
```
</td>
</tr>
<tr>
<td><b>Tracking</b></td>
<td>
```
5
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
    <td><b>Script</b></td>
<td>

```javascript
(async () => {
  return {
    // Encode body as binary data.
    body: Deno.core.encode(
      context.query.arg ?? 'Query string does not include `arg` parameter'
    )
  };
})();
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the responder
4. Once the responder is set up, it will appear in the responders grid along with its unique URL
5. Click the responder's URL and observe that it renders text `Query string does not include `arg` parameter`
6. Change the URL to include a query string parameter `arg` and observe that it renders the value of the parameter
7. Go back to the responder's grid and expand the responder's row to view the incoming requests it has already tracked
8. Notice that all requests are tracked, including queries with and without the `arg` parameter

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../video/guides/webhooks_dynamic_responder.webm" type="video/webm" />
  <source src="../video/guides/webhooks_dynamic_responder.mp4" type="video/mp4" />
</video>

## Annex: Responder script examples

In this section, you'll discover examples of responder scripts capable of constructing dynamic responses based on incoming request properties. Essentially, each script defines a JavaScript function running within a restricted version of the [Deno JavaScript runtime](https://deno.com/). This function has access to incoming request properties through the global `context` variable. The returned value can override default responder's status code, headers, and body.

The `context` argument has the following interface:

```typescript
interface Context {
  // An internet socket address of the client that made the request, if available.
  clientAddress?: string;
  // HTTP method of the received request.
  method: string;
  // HTTP headers of the received request.
  headers: Record<string, string>;
  // HTTP path of the received request.
  path: string;
  // Parsed query string of the received request.
  query: Record<string, string>;
  // HTTP body of the received request in binary form.
  body: number[];
}
```

The returned value has the following interface:

```typescript
interface ScriptResult {
  // HTTP status code to respond with. If not specified, the default status code of responder is used.
  statusCode?: number;
  // Optional HTTP headers of the response. If not specified, the default headers of responder are used.
  headers?: Record<string, string>;
  // Optional HTTP body of the response. If not specified, the default body of responder is used.
  body?: Uint8Array;
}
```

### Override response properties
The script overrides responder's response with a custom status code, headers, and body:

```javascript
(async () => {
  return {
    statusCode: 201,
    headers: {
      "Content-Type": "application/json"
    },
    // Encode body as binary data.
    body: Deno.core.encode(
      JSON.stringify({ a: 1, b: 2 })
    )
  };
})();
```

### Inspect request properties
This script inspects the incoming request properties and returns them as a JSON value:

```javascript
(async () => {
  // Decode request body as JSON.
  const parsedJsonBody = context.body.length > 0
    ? JSON.parse(Deno.core.decode(new Uint8Array(context.body)))
    : {};

  // Override response with a custom HTML body.
  return {
    body: Deno.core.encode(`
      <h2>Request headers</h2>
      <table>
        <tr><th>Header</th><th>Value</th></tr>
        ${Object.entries(context.headers).map(([key, value]) => `
        <tr>
          <td>${key}</td>
          <td>${value}</td>
        </tr>`
        ).join('')}
      </table>
      
      <h2>Request query</h2>
      <table>
        <tr><th>Key</th><th>Value</th></tr>
        ${Object.entries(context.query ?? {}).map(([key, value]) => `
        <tr>
          <td>${key}</td>
          <td>${value}</td>
        </tr>`
        ).join('')}
      </table>
      
      <h2>Request body</h2>
      <pre>${JSON.stringify(parsedJsonBody, null, 2)}</pre>
    `)
  };
})();
``` 

### Generate images and other binary content

Responders can return not only JSON, HTML, or plain text, but also binary data, such as images. This script demonstrates how you can generate a simple PNG image on the fly. PNG generation requires quite a bit of code, so for brevity, this guide assumes that you have already downloaded and edit the [`png-generator.js` script](https://secutils-dev.github.io/secutils-sandbox/responder-scripts/png-generator.js) from the [Secutils.dev Sandbox repository](https://github.com/secutils-dev/secutils-sandbox) (you can find the full source code [here](https://github.com/secutils-dev/secutils-sandbox/blob/6fe5bdf0ad8df23ea67a46e6624c8d6975f96f6a/responder-scripts/src/png-generator.ts)). The part you might want to edit is located at the bottom of the script:

```javascript
(() => {
  // …[Skipping definition of the `PngImage` class for brevity]…

  // Generate a custom 100x100 PNG image with a white background and red rectangle in the center.
  const png = new PngImage(100, 100, 10, {
    r: 255,
    g: 255,
    b: 255,
    a: 1
  });

  const color = png.createRGBColor({
    r: 255,
    g: 0,
    b: 0,
    a: 1
  });
  
  png.drawRect(25, 25, 75, 75, color);

  return {
    body: png.getBuffer()
  };
})();
``` 
