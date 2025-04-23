---
title: Exploring third-party services with webhooks
description: "Exploring third-party services with webhooks: reconnaissance, request bin, iframely, embedding, notion."
slug: exploring-services-with-webhooks
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-06-23_web_bookmark.png
tags: [technology, application-security, guides]
---
Hello!

Today, I'd like to show you how you can leverage the ["Webhooks" feature](https://secutils.dev/docs/guides/webhooks) of Secutils.dev to explore third-party web services, or as a security researcher would say, perform a basic active reconnaissance. Reconnaissance is just a fancy word for gathering information about a target system to identify exploitable vulnerabilities and potential attack vectors. In this post, we'll focus on learning how a specific web service implements functionality that interests us. Our intention is purely innocent — we simply want to understand how it works. However, the technique we'll use is quite similar to what security researchers employ during routine reconnaissance.

<!--truncate-->

For our exploration, we'll be using [Notion](https://www.notion.so/) as our target. Notion has an extensive API surface, but I'm particularly interested in how it handles the embedding of external content, such as links, images, and other web pages.

I'm an avid user of Notion. — it's my go-to tool for everything. I collect numerous links within Notion and heavily rely on their "Web Bookmark" functionality, which provides neat previews and allows me to navigate through the links quickly.

![Bookmark in Notion](https://secutils.dev/docs/img/blog/2023-06-23_web_bookmark.png)

This preview look awesome, and I'd love for my website links to appear similarly in Notion. But how does Notion achieve that? Let's find out!

:::tip NOTE

If you're interested in learning more about Webhooks in Secutils.dev, please refer to the [“Webhook Guides”](https://secutils.dev/docs/guides/webhooks) page.

:::

First and foremost, let's create a simple auto-responder that simulates a web page link:

1. Navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a new responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
web-bookmark-v1
```
</td>
</tr>
<tr>
<td><b>Tracking</b></td>
<td>
```
10
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

Here's the result:

![Auto-responder as Web bookmark in Notion](https://secutils.dev/docs/img/blog/2023-06-23_webhook_v1.png)

Hmm, not very impressive. There's no title and no image preview. Let's see what our auto-responder was able to track when Notion attempted to create a bookmark for our link:

![Auto-responder as Web bookmark in Notion](https://secutils.dev/docs/img/blog/2023-06-23_webhook_v1_requests.png)

Aha! By examining the HTTP headers of the request received by our responder, we can see that Notion relies on [Iframely](https://iframely.com) to generate previews for web page links.

If I were a security researcher, I'd definitely subscribe to updates on the [Iframely GitHub repository](https://github.com/itteco/iframely) and thoroughly go through recent open issues. Who knows what useful information I might stumble upon for my research? I would also do the same for the crucial dependencies of Iframely. Unfortunately, the version mentioned in the header (v1.3.1), released more than four years ago in 2019, turns out to be a false lead. It seems they continue to use the same header even for the latest Iframely versions, so there's no way to exploit any old and disclosed vulnerabilities. What a bummer!

Another potentially valuable piece of information I extracted from the tracked headers is the client IP, which is allocated somewhere in the AWS territory [according to ipinfo.io](https://ipinfo.io/3.94.90.129). It may not be super useful at the moment, but you never know when it might become a valuable clue.

Alright, enough with the security researcher mindset. In my case, I simply want to make my link look nice in Notion, and the `user-agent` HTTP header helpfully provides a link where I can figure this out: https://iframely.com/docs/about. According to the documentation, all we need to do is add a few HTML meta tags to define the title and thumbnail for our link:

```html
<meta property="iframely:description" content="Hello from Secutils.dev Webhooks!" />
<meta property="iframely:image" content="https://avatars.githubusercontent.com/u/1713708?v=4" />
<title>My Mega Title</title>
```

Unfortunately, it appears that Notion/Iframely caches web page metadata. So, in order to force Notion/Iframely to fetch the web page metadata again, we'll need to create another responder with the same content plus additional `meta` tags.

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
web-bookmark-v2
```
</td>
</tr>
<tr>
<td><b>Tracking</b></td>
<td>
```
10
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
  <meta property="iframely:description" content="Hello from Secutils.dev Webhooks!" />
  <meta property="iframely:image" content="https://avatars.githubusercontent.com/u/1713708?v=4" />
  <title>My Mega Title</title>
</head>
<body>Hello World</body>
</html>
```
</td>
</tr>
</tbody>
</table>

And here's the result — let's call it a success!

![Auto-responder as rich Web bookmark in Notion](https://secutils.dev/docs/img/blog/2023-06-23_webhook_v2.png)

Upon inspecting the created bookmark using browser dev tools, we can observe that Notion directly renders the image using the URL from `iframely:image`. This means we have the ability to change the image content at any time, and it will be reflected in the user's bookmark! For some reason, the concept of [PNG steganography](https://decoded.avast.io/martinchlumecky/png-steganography/) immediately comes to mind :wink:

Alright, let's continue exploring what we can learn about Notion. This time, let's attempt to embed our web page directly as an `iframe` instead of a web bookmark. To bypass the Notion/Iframely cache once again, let's create another responder with the same content and select the "Create embed" option in Notion.

If we visit Secutils.dev and inspect the requests received by our new responder, we'll notice something interesting. This time, it received three requests instead of just one:

![Auto-responder as Web bookmark in Notion](https://secutils.dev/docs/img/blog/2023-06-23_webhook_v3_iframe.png)

1. We see the same familiar `GET` request from Iframely, likely fetching the link metadata to automatically detect the type of content being embedded.
2. There's a `HEAD` request coming from a user agent named `NotionEmbedder`. Hmm, what exactly is it? 
3. Finally, there's a `GET` request from our browser, fetching the content of the embedded page and rendering it within the iframe.

Let's dive into the HTTP headers of that new `HEAD` request:

![Auto-responder as Web bookmark in Notion](https://secutils.dev/docs/img/blog/2023-06-23_webhook_v3_headers.png)

Although the purpose of this request isn't entirely clear, it's likely that Notion uses it to verify the accessibility of the embedded content. If the content is not accessible, Notion displays a warning to the user indicating that it's unavailable. Additionally, based on the presence of the `x-datadog-*` HTTP headers, we can infer that the request has been made using [Synthetic APM from DataDog](https://docs.datadoghq.com/synthetics/apm/). With our security researcher hat on, it might be worth subscribing to updates on DataDog security advisories and checking if there are any known vulnerabilities in the Synthetic APM product!

Okay, I believe we've gained enough understanding of how Notion embeds content for now. In general, when a service allows us to embed external content or triggers communication with another web service under our control, we can gather valuable information about that service that usually isn't publicly available. This information can be highly useful for research purposes or to simply satisfy our curiosity.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
