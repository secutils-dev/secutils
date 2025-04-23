---
title: Detecting changes in JavaScript and CSS isn't an easy task, Part 2
description: "Detecting changes in JavaScript and CSS isn't an easy task: web scraping, Playwright, data and blob URLs, fuzzy hashing, TLSH, Locality Sensitive Hashing!"
slug: detecting-changes-in-js-css-part-2
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-13_web_page_resources.png
tags: [thoughts, overview, technology]
---
Hello!

This is the second part of [**my previous post**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-1) where I started discussing the challenges related to tracking changes in JavaScript and CSS resources, and how I address these challenges in the Resources Tracker utility in [**Secutils.dev**](https://secutils.dev).

In the previous part, I talked about handling inline and external resources, dealing with dynamically loaded resources, and comparing large-sized resources. Now, let's explore the next set of challenges you need to consider when comparing JavaScript and CSS resources.

<!--truncate-->

## Challenge #4: Data and blob URLs

Distinguishing between inline and external resources is usually straightforward based on the presence of the `src` attribute in `<script>` elements or the use of separate `<link[rel=stylesheet]>` elements for external CSS resources. As I mentioned in the previous post, to obtain the content of external JavaScript and CSS resources, I leverage Playwright's capability to intercept network requests. However, not all seemingly external resources are genuinely external!

If a resource's URL is a data URL (e.g., `data:application/javascript;base64,YWxlcnQoMSk=`) or a blob URL (e.g., `blob:aac12324xxxxxx`), it does not trigger a network request that can be intercepted. This essentially makes the resource internal. To retrieve the content of a data URL, we need to parse the URL itself, while for a blob URL, we use the "fetch" API:

```ts
// Split the data URL to extract the content.
const [/* data URL header */, dataUrlContent] = dataUrl.split(',');

// Fetch the blob URL to retrieve the content.
const blobUrlContent = await fetch(url).then((res) => res.text());
```

## Challenge #5: Constantly changing resources

Suppose we overcome the challenges we have discussed so far. In that case, we will have quite a solid solution to track changes in web page resources. However, detecting changes in inline resources is a non-trivial challenge on its own: how do we reliably distinguish between changed inline resources and newly added ones? Unlike external resources, inline resources don't have a unique identifier like a URL. The only identifying information we have is the content digest (e.g., SHA1 hash). Even a slight change in the content results in a completely different digest. This means that distinguishing changed inline resources from added ones is not straightforward. Here's an example:

```html
// Inline script
<script>alert(1)</script>
// SHA1 digest of the content (echo 'alert(1)' | sha1sum)
739033c41a7b1047bb6a63240cbe240cd06597cd

// Changed inline script
<script>alert(2)</script>
// SHA1 digest of the changed content (echo 'alert(2)' | sha1sum)
c3d954707f1c1043158a4ed38f52776e7859e80c
```

As you can see, the scripts are almost the same, but even a minor change results in a completely different digest. Thus, a naive program would treat the case as one inline resource being removed, while another has been added.

The behavior is often observed in user tracking scripts and other bloatware-resources. They are sometimes generated with the random identifiers for each unique user, resulting in frequent changes that make it challenging to track and detect modifications accurately.

To address this challenge, we can turn to malware detection techniques! One such technique is [**fuzzy hashing**](https://en.wikipedia.org/wiki/Fuzzy_hashing), which helps to detect similar but not exactly identical data. Cryptographic hash functions, in contrast, generate significantly different hashes even for minor differences. Fuzzy hashing is particularly useful for malware detection, as malware authors often make slight tweaks to the code to change the hash fingerprint and evade detection by naive malware systems.

There are various fuzzy hashing approaches, such as Context Triggered Piecewise Hashing (CTPH) and Locality Sensitive Hashing (LSH). For Secutils.dev, I use [**Trend Micro Locality Sensitive Hashing (TLSH)**](https://tlsh.org/) due to its relatively low content requirements for generating hashes. It only needs content that is 50 bytes or larger and has sufficient randomness, which is typically the case for 99.99% of inline web page resources. If, for some reason, these requirements are not met, we can fall back to storing the entire content if it is small enough or using the SHA1 digest if it lacks sufficient randomness (I'd be rather surprised though):

```bash
$ cat inline-revision-0.js 
alert(1);
alert(2);
alert(3);
alert(4);
alert(5);
alert(6);
alert(7);

cat inline-revision-1.js 
alert(1);
alert(2);
alert(3);
alert(400); <-- changed value
alert(500); <-- changed value
alert(6);
alert(7);

$ tlsh -f inline-revision-0.js 
T1C4A0025D65B74CD0C3B69F48020CD01304000118314F0D42000F81DC1019342C001404 <-- TLS hash #0

$ tlsh -f inline-revision-1.js 
T1B9A0024D65730CC0D77A9F48012CD00746000018318F0D42000F80DC1019342E003404 <-- TLS hash #1

$ tlsh -c inline-revision-0.js -f inline-revision-1.js <-- compare files/hashes
25 <-- "distance" between two files, files are very similar!
```

Fuzzy hashing is indeed a perfect solution for our use case, allowing us to detect changes in inline resources without the need to store the entire content!

## Conclusion

In this post, I have highlighted a few more challenges you may encounter when tracking changes in web page resources. However, I realized that I couldn't cover everything I wanted to share in just two parts. So, stay tuned for [**the third and final part**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-3), where I will explore the **security-oriented** (finally!) aspects of web page resource tracking. Stay tuned for more insights!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
