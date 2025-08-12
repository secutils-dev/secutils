---
sidebar_position: 1
sidebar_label: Page Trackers
title: Web Scraping ➔ Page trackers
description: Learn how to create and use page trackers in Secutils.dev.
---

# What is a page tracker?

A page tracker is a utility that empowers developers to detect and monitor the content of any web page. Use cases range from ensuring that the deployed web application loads only the intended content throughout its lifecycle to tracking changes in arbitrary web content when the application lacks native tracking capabilities. In the event of a change, whether it's caused by a broken deployment or a legitimate content modification, the tracker promptly notifies the user.

:::caution NOTE
Currently, Secutils.dev doesn't support tracking content for web pages protected by application firewalls (WAF) or any form of CAPTCHA. If you require tracking content for such pages, please comment on [#secutils/34](https://github.com/secutils-dev/secutils/issues/34) to discuss your use case.
:::

On this page, you can find guides on creating and using page trackers.

:::note
The `Content extractor` script is essentially a [Playwright scenario](https://playwright.dev/docs/writing-tests) that allows you to extract almost anything from the web page as long as it doesn't exceed **1MB** in size. For instance, you can include text, links, images, or even JSON.
:::

## Create a page tracker

In this guide, you'll create a simple page tracker for the top post on [Hacker News](https://news.ycombinator.com/):

1. Navigate to [Web Scraping → Page trackers](https://secutils.dev/ws/web_scraping__page) and click **Track page** button
2. Configure a new tracker with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Hacker News Top Post
```
</td>
</tr>
<tr>
<td><b>Frequency</b></td>
<td>
```
Manually
```
</td>
</tr>
<tr>
<td><b>Content extractor</b></td>
<td>
```javascript
export async function execute(page) {
  // Navigate to the Hacker News homepage.
  await page.goto('https://news.ycombinator.com/');

  // Get the link to the top post.
  const titleLink = page.locator('css=.titleline a').first();

  // Return the title and link of the top post formatted as markdown.
  return `[${(await titleLink.textContent()).trim()}](${await titleLink.getAttribute('href')})`;
};
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the tracker
4. Once the tracker is set up, it will appear in the trackers grid
5. Expand the tracker's row and click the **Update** button to run it for the first time

After a few seconds, the tracker will fetch the content of the top post on Hacker News and display it below the tracker's row. The content includes only the title of the post. However, as noted at the beginning of this guide, the content extractor script allows you to return almost anything, even the entire HTML of the post.

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_scraping_page_tracker.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_page_tracker.mp4" type="video/mp4" />
</video>

## Detect changes with a page tracker

In this guide, you'll create a page tracker and test it with changing content:

1. Navigate to [Web Scraping → Page trackers](https://secutils.dev/ws/web_scraping__page) and click **Track page** button
2. Configure a new tracker with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
World Clock
```
</td>
</tr>
<tr>
<td><b>Frequency</b></td>
<td>
```
Hourly
```
</td>
</tr>
<tr>
<td><b>Content extractor</b></td>
<td>
```javascript
export async function execute(page) {
  // Navigate to the Berlin world clock page.
  await page.goto('https://www.timeanddate.com/worldclock/germany/berlin');

  // Wait for the time element to be visible and get its value.
  const time = await page.locator('css=#qlook #ct').textContent();

  // Return the time formatted as markdown with a link to the world clock page.
  return `Berlin time is [**${time}**](https://www.timeanddate.com/worldclock/germany/berlin)`;
};
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the tracker
4. Once the tracker is set up, it will appear in the trackers grid with bell and timer icons, indicating that the tracker is configured to regularly check content and send notifications when changes are detected
5. Expand the tracker's row and click the **Update** button to make the first snapshot of the web page content
6. After a few seconds, the tracker will fetch the current Berlin time and render a nice markdown with a link to a word clock website:

:::note EXAMPLE
Berlin time is [**01:02:03**](https://www.timeanddate.com/worldclock/germany/berlin)
:::

7. With this configuration, the tracker will check the content of the web page every hour and notify you if any changes are detected.

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_scraping_page_tracker_diff.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_page_tracker_diff.mp4" type="video/mp4" />
</video>

## Track web page resources

You can also use page tracker utility to detect and track resources of any web page. This functionality falls under the category of [synthetic monitoring](https://en.wikipedia.org/wiki/Synthetic_monitoring) tools and helps ensure that the deployed application loads only the intended web resources (JavaScript and CSS) during its lifetime. If any unintended changes occur, which could result from a broken deployment or malicious activity, the tracker will promptly notify developers or IT personnel about the detected anomalies.

Additionally, security researchers who focus on discovering potential vulnerabilities in third-party web applications can use page trackers to be notified when the application's resources change. This allows them to identify if the application has been upgraded, providing an opportunity to re-examine it and potentially discover new vulnerabilities.

:::note EXAMPLE
Extracting all page resources isn't as straightforward as it might seem, so it's recommended to use the utilities provided by Secutils.dev, as demonstrated in the examples in the following sections. Utilities return CSS and JS resource descriptors with the following interfaces:
```typescript
/**
 * Describes external or inline resource.
 */
interface WebPageResource {
  /**
   * Resource type, either 'script' or 'stylesheet'.
   */
  type: 'script' | 'stylesheet';

  /**
   * The URL resource is loaded from.
   */
  url?: string;

  /**
   * Resource content descriptor (size and digest), if available.
   */
  content: WebPageResourceContent;
}

/**
 * Describes resource content.
 */
interface WebPageResourceContent {
  /**
   * Resource content data.
   */
  data: { raw: string } | { tlsh: string } | { sha1: string };

  /**
   * Describes resource content data, it can either be the raw content data or a hash such as Trend Micro Locality
   * Sensitive Hash or simple SHA-1.
   */
  size: number;
}
```
:::

In this guide, you'll create a simple page tracker to track resources of the [Hacker News](https://news.ycombinator.com/):

1. Navigate to [Web Scraping → Page trackers](https://secutils.dev/ws/web_scraping__page) and click **Track page** button
2. Configure a new tracker with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Hacker News (resources)
```
</td>
</tr>
<tr>
<td><b>Content extractor</b></td>
<td>
```javascript
export async function execute(page, { previousContent }) {
  // Load built-in utilities for tracking resources.
  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(
    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()
  )}`);

  // Start tracking resources.
  utils.startTracking(page);

  // Navigate to the target page.
  await page.goto('https://news.ycombinator.com');
  await page.waitForTimeout(1000);

  // Stop tracking and return resources.
  const resources = await utils.stopTracking(page);

  // Format resources as a table, 
  // showing diff status if previous content is available.
  return utils.formatAsTable(
    previousContent
      ? utils.setDiffStatus(previousContent.original.source, resources)
      : resources
  );
};
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the tracker
4. Once the tracker is set up, it will appear in the trackers grid
5. Expand the tracker's row and click the **Update** button to make the first snapshot of the web page resources

It's hard to believe, but as of the time of writing, Hacker News continues to rely on just a single script and stylesheet!

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_scraping_page_resources_tracker.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_page_resources_tracker.mp4" type="video/mp4" />
</video>

## Filter web page resources

In this guide, you will create a page tracker for the GitHub home page and learn how to track only specific resources:

1. Navigate to [Web Scraping → Page trackers](https://secutils.dev/ws/web_scraping__page) and click **Track page** button
2. Configure a new tracker with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
GitHub
```
</td>
</tr>
<tr>
<td><b>Content extractor</b></td>
<td>
```javascript
export async function execute(page, { previousContent }) {
  // Load built-in utilities for tracking resources.
  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(
    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()
  )}`);

  // Start tracking resources.
  utils.startTracking(page);

  // Navigate to the target page.
  await page.goto('https://github.com');
  await page.waitForTimeout(1000);

  // Stop tracking and return resources.
  const resources = await utils.stopTracking(page);

  // Format resources as a table, 
  // showing diff status if previous content is available.
  return utils.formatAsTable(
    previousContent
      ? utils.setDiffStatus(previousContent.original.source, resources)
      : resources
  );
};
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the tracker
4. Once the tracker is set up, it will appear in the trackers grid
5. Expand the tracker's row and click the **Update** button to make the first snapshot of the web page resources
6. Once the tracker has fetched the resources, they will appear in the resources grid. You'll notice that there are nearly 100 resources used for the GitHub home page! In the case of large and complex pages like this one, it's recommended to have multiple separate trackers, e.g. one per logical functionality domain, to avoid overwhelming the developer with too many resources and consequently changes they might need to track. Let's say we're only interested in "vendored" resources.
7. To filter out all resources that are not "vendored", we'll adjust content extractor script. Click the pencil icon next to the tracker's name to edit the tracker and update the following properties:

<table class="su-table">
<tbody>
<tr>
<td><b>Content extractor</b></td>
<td>
```javascript
export async function execute(page, { previousContent }) {
  // Load built-in utilities for tracking resources.
  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(
    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()
  )}`);

  // Start tracking resources.
  utils.startTracking(page);

  // Navigate to the target page.
  await page.goto('https://github.com');
  await page.waitForTimeout(1000);

  // Stop tracking and return resources.
  const allResources = await utils.stopTracking(page);

  // Filter out all resources that are not "vendored".
  const resources = {
    scripts: allResources.scripts.filter((resource) => resource.url?.includes('vendors')),
    styles: allResources.styles.filter((resource) => resource.url?.includes('vendors')),
  };

  // Format resources as a table,
  // showing diff status if previous content is available.
  return utils.formatAsTable(
    previousContent
      ? utils.setDiffStatus(previousContent.original.source, resources)
      : resources
  );
};
```
</td>
</tr>
</tbody>
</table>

8. Now, click the **Save** button to save the tracker.
9. Click the **Update** button to re-fetch web page resources. Once the tracker has re-fetched resources, only about half of the previously extracted resources will appear in the resources grid.

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_scraping_page_resources_tracker_filter.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_page_resources_tracker_filter.mp4" type="video/mp4" />
</video>

## Detect changes in web page resources

In this guide, you will create a page tracker and test it using a custom HTML responder:

1. First, navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a few responders with the following values to emulate JavaScript files that we will track changes for across revisions:

This JavaScript **will remain unchanged** across revisions:
<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
no-changes.js
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/no-changes.js
```
</td>
</tr>
<tr>
    <td><b>Headers</b></td>
<td>

```http
Content-Type: application/javascript; charset=utf-8
```
</td>
</tr>
<tr>
    <td><b>Body</b></td>
<td>

```js
document.body.insertAdjacentHTML(
  'beforeend',
  'Source: no-changes.js<br>'
);
```
</td>
</tr>
</tbody>
</table>

This JavaScript **will change** across revisions:
<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
changed.js
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/changed.js
```
</td>
</tr>
<tr>
    <td><b>Headers</b></td>
<td>

```http
Content-Type: application/javascript; charset=utf-8
```
</td>
</tr>
<tr>
    <td><b>Body</b></td>
<td>

```js
document.body.insertAdjacentHTML(
  'beforeend',
  'Source: changed.js, Changed: no<br>'
);
```
</td>
</tr>
</tbody>
</table>

This JavaScript **will be removed** across revisions:
<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
removed.js
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/removed.js
```
</td>
</tr>
<tr>
    <td><b>Headers</b></td>
<td>

```http
Content-Type: application/javascript; charset=utf-8
```
</td>
</tr>
<tr>
    <td><b>Body</b></td>
<td>

```js
document.body.insertAdjacentHTML(
  'beforeend',
  'Source: removed.js<br>'
);
```
</td>
</tr>
</tbody>
</table>

This JavaScript **will be added** in a new revision:
<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
added.js
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/added.js
```
</td>
</tr>
<tr>
    <td><b>Headers</b></td>
<td>

```http
Content-Type: application/javascript; charset=utf-8
```
</td>
</tr>
<tr>
    <td><b>Body</b></td>
<td>

```js
document.body.insertAdjacentHTML(
  'beforeend',
  'Source: added.js<br>'
);
```
</td>
</tr>
</tbody>
</table>

3. Now, configure a new responder with the following values to respond with a simple HTML page that references previously created JavaScript responders (except for `added.js`):

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
track-me.html
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/track-me.html
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
  <title>Evaluate resources tracker</title>
  <script type="text/javascript" src="./no-changes.js" defer></script>
  <script type="text/javascript" src="./changed.js" defer></script>
  <script type="text/javascript" src="./removed.js" defer></script>
</head>
<body></body>
</html>
```
</td>
</tr>
</tbody>
</table>

4. Click the **Save** button to save the responder
5. Once the responder is set up, it will appear in the responders grid along with its unique URL
6. Click on the responder's URL and make sure that it renders the following content:

```html
Source: no-changes.js
Source: changed.js, Changed: no
Source: removed.js
```
7. Now, navigate to [Web Scraping → Page trackers](https://secutils.dev/ws/web_scraping__page) and click **Track page** button
8. Configure a new tracker for `track-me.html` responder with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Demo
```
</td>
</tr>
<tr>
<td><b>URL</b></td>
<td>
```
https://[YOUR UNIQUE ID].webhooks.secutils.dev/track-me.html
```
</td>
</tr>
<tr>
<td><b>Frequency</b></td>
<td>
```
Daily
```
</td>
</tr>
<tr>
<td><b>Notifications</b></td>
<td>
```
☑
```
</td>
</tr>
<tr>
<td><b>Content extractor</b></td>
<td>
```javascript
export async function execute(page, { previousContent }) {
  // Load built-in utilities for tracking resources.
  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(
    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()
  )}`);

  // Start tracking resources.
  utils.startTracking(page);

  // Navigate to the target page
  // Replace `[YOUR UNIQUE ID]` with your unique handle!.
  await page.goto('https://[YOUR UNIQUE ID].webhooks.secutils.dev/track-me.html');
  await page.waitForTimeout(1000);

  // Stop tracking, and return resources.
  const resources = await utils.stopTracking(page);

  // Format resources as a table,
  // showing diff status if previous content is available.
  return utils.formatAsTable(
    previousContent
      ? utils.setDiffStatus(previousContent.original.source, resources)
      : resources
  );
};
```
</td>
</tr>
</tbody>
</table>

:::tip TIP
Configured tracker will fetch the resources of the `track-me.html` responder once a day and notify you if any changes are detected. You can change the frequency and notification settings to suit your needs.
:::

9. Click the **Save** button to save the tracker
10. Once the tracker is set up, it will appear in the trackers grid
11. Expand the tracker's row and click the **Update** button to make the first snapshot of the web page resources
12. Once the tracker has fetched the resources, they will appear in the resources grid:

<table class="su-table">
<tbody>
<tr><th>Source</th><th>Diff</th><th>Type</th><th>Size</th></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/no-change.js`</td><td>-</td><td>Script</td><td>81</td></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/changed.js`</td><td>-</td><td>Script</td><td>91</td></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/removed.js`</td><td>-</td><td>Script</td><td>78</td></tr>
</tbody>
</table>

13. Now, navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and edit `track-me.html` responder to reference `added.js` responder, and remove reference to `removed.js`:

```diff
<!DOCTYPE html>
<html lang="en">
<head>
  <title>Evaluate resources tracker</title>
  <script type="text/javascript" src="./no-changes.js" defer></script>
  <script type="text/javascript" src="./changed.js" defer></script>
// highlight-next-line
- <script type="text/javascript" src="./removed.js" defer></script>
// highlight-next-line
+ <script type="text/javascript" src="./added.js" defer></script>
</head>
<body></body>
</html>
```

14. Next, change the body of the `changed.js` responder to something like this:

```diff
document.body.insertAdjacentHTML(
  'beforeend',
// highlight-next-line
- 'Source: changed.js, Changed: no<br>'
// highlight-next-line
+ 'Source: changed.js, Changed: yes<br>'
);
```

15. Finally, navigate to [Web Scraping → Page trackers](https://secutils.dev/ws/web_scraping__page) and expand the `Demo` tracker's row
16. Click **Update** button to fetch the next revision of the web page resources
17. Once the tracker has fetched updated resources, they will appear in the resources grid together with the diff status:

<table class="su-table">
<tbody>
<tr><th>Source</th><th>Diff</th><th>Type</th><th>Size</th></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/no-change.js`</td><td><b>-</b></td><td>Script</td><td>81</td></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/changed.js`</td><td><b>Changed</b></td><td>Script</td><td>91</td></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/added.js`</td><td><b>Added</b></td><td>Script</td><td>76</td></tr>
<tr><td>`https://[YOUR UNIQUE ID].webhooks.secutils.dev/removed.js`</td><td><b>Removed</b></td><td>Script</td><td>78</td></tr>
</tbody>
</table>

## Annex: Content extractor script examples

In this section, you can find examples of content extractor scripts that extract various content from web pages. Essentially, the script defines a function with the following signature:

```typescript
/**
 * Content extractor script that extracts content from a web page.
 * @param page - The Playwright Page object representing the web page.
 * See more details at https://playwright.dev/docs/api/class-page.
 * @param context.previousContent - The context extracted during 
 * the previous execution, if available.
 * @returns {Promise<unknown>} - The extracted content to be tracked.
 */
export async function execute(
  page: Page,
  context: { previousContent?: { original: unknown } }
)
```

### Track markdown-style content
The script can return any [**valid markdown-style content**](https://eui.elastic.co/#/editors-syntax/markdown-format#kitchen-sink) that Secutils.dev will happily render in preview mode.

```javascript
export async function execute() {
  return `
    ## Text
    ### h3 Heading
    #### h4 Heading
    
    **This is bold text**
    
    *This is italic text*
    
    ~~Strikethrough~~
    
    ## Lists
    
    * Item 1
    * Item 2
      * Item 2a
    
    ## Code
    
    \`\`\` js
    const foo = (bar) => {
      return bar++;
    };
    
    console.log(foo(5));
    \`\`\`
    
    ## Tables
    
    | Option   | Description   |
    | -------- | ------------- |
    | Option#1 | Description#1 |
    | Option#2 | Description#2 |
    
    ## Links
    
    [Link Text](https://secutils.dev)
    
    ## Emojies
    
    :wink: :cry: :laughing: :yum:
  `;
}
```

### Track API response
You can use page tracker to track API responses as well (until dedicated [`API tracker` utility](https://github.com/secutils-dev/secutils/issues/32) is released). For instance, you can track the response of the [JSONPlaceholder](https://jsonplaceholder.typicode.com/) API:

:::caution NOTE
Ensure that the web page from which you're making a fetch request allows cross-origin requests. Otherwise, you'll get an error.
:::

```javascript
export async function execute() {
  const {url, method, headers, body} = {
    url: 'https://jsonplaceholder.typicode.com/posts',
    method: 'POST',
    headers: {'Content-Type': 'application/json; charset=UTF-8'},
    body: JSON.stringify({title: 'foo', body: 'bar', userId: 1}),
  };
  const response = await fetch(url, {method, headers, body});
  return {
    status: response.status,
    headers: Object.fromEntries(response.headers.entries()),
    body: (await response.text()) ?? '',
  };
}
```

### Use previous content

In the content extract script, you can use the `context.previousContent.original` property to access the content extracted during the previous execution:

```javascript
export async function execute(page, { previousContent }) {
  // Update counter based on the previous content.
  return (previousContent?.original ?? 0) + 1;
}
```

### Use external content extractor script
Sometimes, your content extractor script can become large and complicated, making it hard to edit in the Secutils.dev UI. In such cases, you can develop and deploy the script separately in any development environment you prefer. Once the script is deployed, you can just use URL as the script content :

```javascript
// This code assumes your script exports a function named `execute` function.
https://secutils-dev.github.io/secutils-sandbox/content-extractor-scripts/markdown-table.js
```

You can find more examples of content extractor scripts at the [Secutils.dev Sandbox](https://github.com/secutils-dev/secutils-sandbox/tree/main/content-extractor-scripts) repository.

## Annex: Custom cron schedules

:::caution NOTE
Custom cron schedules are available only for [**Pro** subscription](https://secutils.dev/pricing) users.
:::

In this section, you can learn more about the supported cron expression syntax used to configure custom tracking schedules. A cron expression is a string consisting of six or seven subexpressions that describe individual details of the schedule. These subexpressions, separated by white space, can contain any of the allowed values with various combinations of the allowed characters for that subexpression:

| Subexpression  | Mandatory | Allowed values  | Allowed special characters |
|----------------|-----------|-----------------|----------------------------|
| `Seconds`      | Yes       | 0-59            | * / , -                    |
| `Minutes`      | Yes       | 0-59            | * / , -                    |
| `Hours`        | Yes       | 0-23            | * / , -                    |
| `Day of month` | Yes       | 1-31            | * / , - ?                  |
| `Month`        | Yes       | 0-11 or JAN-DEC | * / , -                    |
| `Day of week`  | Yes       | 1-7 or SUN-SAT  | * / , - ?                  |
| `Year`         | No        | 1970-2099       | * / , -                    |

Following the described cron syntax, you can create almost any schedule you want as long as the interval between two consecutive checks is **longer than 10 minutes**. Below are some examples of supported cron expressions:

| Expression            | Meaning                                             |
|-----------------------|-----------------------------------------------------|
| `0 0 12 * * ?`        | Run at 12:00 (noon) every day                       |
| `0 15 10 ? * *`       | Run at 10:15 every day                              |
| `0 15 10 * * ?`       | Run at 10:15 every day                              |
| `0 15 10 * * ? *`     | Run at 10:15 every day                              |
| `0 15 10 * * ? 2025`  | Run at 10:15 every day during the year 2025         |
| `0 0/10 14 * * ?`     | Run every 10 minutes from 14:00 to 14:59, every day |
| `0 10,44 14 ? 3 WED`  | Run at 14:10 and at 14:44 every Wednesday in March  |
| `0 15 10 ? * MON-FRI` | Run at 10:15 from Monday to Friday                  |
| `0 11 15 8 10 ?`      | Run every October 8 at 15:11                        |

To assist you in creating custom cron schedules, Secutils.dev lists five upcoming scheduled times for the specified schedule:

![Secutils.dev UI - Custom schedule](/img/docs/guides_custom_tracker_schedule.png)
