---
title: Detecting changes in JavaScript and CSS isn't an easy task, Part 1
description: "Detecting changes in JavaScript and CSS isn't an easy task, Part 1: web scraping, HTML, Playwright, hashes, and more"
slug: detecting-changes-in-js-css-part-1
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-11_web_page_weight.png
tags: [thoughts, overview, technology]
---
Hello!

[**In one of my previous posts**](https://secutils.dev/docs/blog/q2-2023-update-resources-tracker), I explained the concept behind the Resource Tracker utility in [**Secutils.dev**](https://secutils.dev) and who can benefit from it. Initially, I had planned to release it in the "Q2 2023 - Apr - Jun" feature update (around the first week of July). However, it has taken a bit more time than I initially anticipated. In this post and the following ones, I would like to explain why comparing JavaScript and CSS files is not as simple of a problem as it may appear at first glance, and I'll share the solution I developed for Secutils.dev.

<!--truncate-->

## Problem statement

As a web application developer, it is crucial to ensure that your deployed application loads only the intended web resources (JavaScript and CSS) during its lifetime. If, for any reason, such as a broken deployment or malicious activity, unintended changes occur, it is important to be notified as soon as possible.

Similarly, as a security researcher focused on discovering and understanding potential security vulnerabilities in third-party web applications, being notified when the application's resources change can be valuable. Such changes could indicate that the application has been upgraded, presenting an opportunity to re-examine the application and potentially identify any new vulnerabilities.

## Challenge #1: Inline and external resources

Modern web pages are often complex, utilizing both inline and external resources simultaneously. Inline resources are embedded directly within the HTML page using specific HTML tags such as `<script>` and `<style>`, while external resources are fetched separately from a remote location and referenced within the main HTML page.

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <script src="./i-am-external-javascript-resource.js" />
    <link href="./i-am-external-css-resource.css" rel="stylesheet" />
    <script>alert('I am an inline JavaScript resource!');</script>
    <style>
      a::before {
        content: 'I am an inline CSS resource!';
      }
    </style>
</head>
<body>Hello World</body>
</html>
```

Considering this, it is not sufficient to solely fetch the static HTML page and parse its content to extract resources for comparison. We also need to fetch all the external resources referenced within the page. Okay, this adds a slight complexity to the solution, but it can be considered as a routing web scraping task. Let's move on.

## Challenge #2: Dynamically loaded resources

Parsing a static HTML page and fetching external resources may not be overly complex, as there are numerous libraries available in various high-level programming languages to assist with these tasks. However, there is an additional challenge when it comes to dynamically loaded resources. JavaScript code can load or dynamically generate other JavaScript and CSS resources, and CSS resources can dynamically import additional CSS resources.

Accounting for dynamically loaded resources significantly increases the complexity of the problem we are addressing. It is no longer sufficient to simply parse HTML and fetch external resources. Now we need to **interpret or evaluate** these resources, similar to what web browsers do. Fortunately, there are mature and robust libraries that provide high-level APIs for automating web browsers in tasks like these. Two popular options are [**Puppeteer**](https://pptr.dev/) and [**Playwright**](https://playwright.dev/).

While both Puppeteer and Playwright have their own advantages and disadvantages, I have chosen Playwright for Secutils.dev. Playwright not only allows us to access all browser APIs within the web page context to easily detect and extract inline resources, but also enables us to **intercept** all external dynamically loaded web page resources. Here's an example of the code (full code can be found [**here**](https://github.com/secutils-dev/retrack/blob/main/components/retrack-web-scraper/src/api/web_page/execute.ts)):

```ts
const page = await browser.newPage();

// Intercept all responses with external resources.
page.on('response', async (response) => {
  const resourceType = response.request().resourceType() as 'script' | 'stylesheet';
  if (resourceType !== 'script' && resourceType !== 'stylesheet') {
    return;
  }

  // Extract and process resource content.
  const externalResourceContent = await response.body();
  ...
});

// Load the web page.
await page.goto(url, { waitUntil: 'domcontentloaded', timeout });

// Use `page.evaluate()` to evaluate JavaScript in the page context
// and extract all inline resources.
const inlineResources = await page.evaluate(async () => {
  // Extract inline JavaScript.
  for (const script of Array.from(document.querySelectorAll('script'))) {}
  // Extract inline CSS.
  for (const style of Array.from(document.querySelectorAll('style'))) {}
  ...
});
```

It doesn't appear overly complex, which is great!

:::tip NOTE
Running a full-blown browser is a resource-intensive task, and parsing external web pages can also have potential [**security concerns**](https://www.scmagazine.com/news/vulnerability-management/google-critical-rce-bug-chrome-browser). That is why I have chosen to run the Secutils.dev [**Retrack component**](https://github.com/secutils-dev/retrack) separately from the main Secutils.dev server, as a dedicated Kubernetes deployment. This allows me to scale it independently and isolate it from the main server.
:::

## Challenge #3: Large resources

Once we have detected all the inline and external resources, the next step is to determine how we will detect changes. The most straightforward approach is to compare the content of the resources. Secutils.dev allows users to store multiple revisions of web page resources, so if we want to use the original content to detect changes between revisions, we would need to store the full content of all detected resources for each revision. However, considering the size of web page resources, this approach becomes challenging. According to the [**Web Almanac 2022 report**](https://almanac.httparchive.org/en/2022/page-weight#javascript), the median desktop page loads around 1,026 KB of images, 509 KB of JavaScript, 72 KB of CSS, and 31 KB of HTML. Similarly, the median mobile page loads around 881 KB of images, 461 KB of JavaScript, 68 KB of CSS, and 30 KB of HTML.

![Web page resources size](https://secutils.dev/docs/img/blog/2023-07-11_web_page_weight.png)

With approximately 500-600 KB per page per revision (and even more for many web pages), storing all that data becomes prohibitively expensive, unless the costs are passed on to the users (perhaps through a premium subscription 😉).

Considering that most users are primarily interested in just detecting unexpected changes, storing the entire content of resources becomes less useful. Instead, we can store and compare a hash of the resource content. A SHA-1 digest should serve this purpose well!

## Conclusion

In this post, I have covered the most obvious challenges you may encounter when tracking changes in web page resources. [**In the next part**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-2) of this post, I will dive into more intricate and less obvious challenges, such as dealing with `blob:` and `data:` resources, and applying malware detection techniques to handle pesky resources that change with every page load. Stay tuned!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
