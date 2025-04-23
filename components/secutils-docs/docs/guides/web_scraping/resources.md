---
sidebar_position: 1
sidebar_label: Resources Trackers
title: Web Scraping ➔ Web page resources trackers
description: Learn how to create and use web page resources trackers in Secutils.dev.
---

# What is a web page resources tracker?

The web page resources tracker is a utility that gives developers the ability to detect and track resources of any web page. It falls under the category of [synthetic monitoring](https://en.wikipedia.org/wiki/Synthetic_monitoring) tools and helps ensure that the deployed application loads only the intended web resources (JavaScript and CSS) during its lifetime. If any unintended changes occur, which could result from a broken deployment or malicious activity, the tracker will promptly notify developers or IT personnel about the detected anomalies.

Additionally, security researchers focused on discovering potential security vulnerabilities in third-party web applications can use web page resources trackers. By being notified when the application's resources change, researchers can identify if the application has been upgraded, providing an opportunity to re-examine the application and potentially discover new vulnerabilities.

:::caution NOTE
Currently, Secutils.dev doesn't support tracking resources for web pages protected by application firewalls (WAF) or any form of CAPTCHA. If you require tracking resources for such pages, please comment on [#secutils/34](https://github.com/secutils-dev/secutils/issues/34) to discuss your use case.
:::

On this page, you can find guides on creating and using web page resources trackers.

## Create a web page resources tracker

In this guide, you'll create a simple resources tracker for the [Hacker News](https://news.ycombinator.com/):

1. Navigate to [Web Scraping → Resources trackers](https://secutils.dev/ws/web_scraping__resources) and click **Track resources** button
2. Configure a new tracker with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
Hacker News
```
</td>
</tr>
<tr>
<td><b>URL</b></td>
<td>
```
https://news.ycombinator.com
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
  <source src="../../video/guides/web_scraping_resources_tracker.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_resources_tracker.mp4" type="video/mp4" />
</video>

## Detect changes with a web page resources tracker

In this guide, you will create a web page resources tracker and test it using a custom HTML responder:

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
7. Now, navigate to [Web Scraping → Resources trackers](https://secutils.dev/ws/web_scraping__resources) and click **Track resources** button
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

15. Finally, navigate to [Web Scraping → Resources trackers](https://secutils.dev/ws/web_scraping__resources) and expand the `Demo` tracker's row
16. Click **Update** button to fetch the next revision of the web page resources

:::caution NOTE
Normally, Secutils.dev caches web page resources for **10 minutes**. This means that if you make changes to the web page resources and want to see them reflected in the tracker, you'll need to wait for 10 minutes before re-fetching resources. However, for this guide, I've disabled caching for the tracker so that you can see changes immediately.
:::
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

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_scraping_resources_tracker_diff.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_resources_tracker_diff.mp4" type="video/mp4" />
</video>

## Filter resources with a web page resources tracker

In this guide, you will create a web page resource tracker for the Reddit home page and learn how to track only specific resources:

1. Navigate to [Web Scraping → Resources trackers](https://secutils.dev/ws/web_scraping__resources) and click **Track resources** button
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
<td><b>URL</b></td>
<td>
```
https://github.com/?rev=1
```
</td>
</tr>
</tbody>
</table>

:::tip TIP
Normally, Secutils.dev caches web page resources for **10 minutes**. This means that if you make changes to the web page resource tracker and want to see them take effect, you'll need to wait for 10 minutes before re-fetching resources. However, for this guide, I'm adding an arbitrary `?rev=X` query string parameter to the URL to bypass caching and see the changes immediately. This trick can be quite handy when you are setting up a new tracker and need to fine-tune its configuration. 

Note that every time you change the tracker's URL, all previously fetched resources **will be removed**.
:::

3. Click the **Save** button to save the tracker
4. Once the tracker is set up, it will appear in the trackers grid
5. Expand the tracker's row and click the **Update** button to make the first snapshot of the web page resources
6. Once the tracker has fetched the resources, they will appear in the resources grid. You'll notice that there are nearly 80 resources used for the GitHub home page! In the case of large and complex pages like this one, it's recommended to have multiple separate trackers, e.g. one per logical functionality domain, to avoid overwhelming the developer with too many resources and consequently changes they might need to track. Let's say we're only interested in "vendored" resources.
7. To filter out all resources that are not "vendored", we'll use the `Resource filter/mapper` feature. Click the pencil icon next to the tracker's name to edit the tracker and update the following properties:

<table class="su-table">
<tbody>
<tr>
<td><b>URL</b></td>
<td>
```
https://github.com/?rev=2
```
</td>
</tr>
<tr>
    <td><b>Resource filter/mapper</b></td>
<td>

```javascript
return resource.url?.includes('vendors')
  ? resource
  : null;
```
</td>
</tr>
</tbody>
</table>

8. The **Resource filter/mapper** property accepts a JavaScript function that is executed for each resource detected by the tracker. The function receives a single `resource` argument, which is the resource object. The function must return either the resource object or `null`. If the function returns `null`, the resource will be filtered out and will not be tracked. In our case, we're filtering out all resources that do not contain sso in their URL. You can learn more about resource filter/mapper scripts in the [**Annex: Resource filter/mapper script examples**](#annex-resource-filtermapper-script-examples) section.
9. Now, click the **Save** button to save the tracker.
10. Click the **Update** button to re-fetch web page resources. Once the tracker has re-fetched resources, only about half of the previously extracted resources will appear in the resources grid.

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_scraping_resources_tracker_filter.webm" type="video/webm" />
  <source src="../../video/guides/web_scraping_resources_tracker_filter.mp4" type="video/mp4" />
</video>

## Annex: Resource filter/mapper script examples

In this section, you can find examples of resource filter and mapper scripts that you can use to filter out or map resources based on various criteria. The script essentially defines a function that is executed for each resource detected by the tracker and receives a single `resource` argument, which is the resource object. The function must return either the resource object or `null`. If the function returns `null`, the resource will be filtered out and will not be tracked. 

The `resource` argument has the following interface:

```typescript
interface Resource {
  // Resource full URL. This property is not defined for inline resources. 
  url?: string;
  // Resource content.
  data: string;
  // Resource type.
  type: 'script' | 'stylesheet';
}
```

### Track only external resources

```javascript
return resource.url?.startsWith('http')
  ? resource
  : null;
```

### Track only inline resources

```javascript
return !resource.url
  ? resource
  : null;
```

### Track only JavaScript resources

```javascript
return resource.type === 'script'
  ? resource
  : null;
```

### Track only CSS resources

```javascript
return resource.type === 'stylesheet'
  ? resource
  : null;
```

### Strip query string parameters from resource URLs
Sometimes, resources such as analytics and user tracking scripts load with unique query string parameters, even when the content of the resource remains constant. This can lead to confusion for the tracker and trigger unwanted change notifications. To address this, you can strip query string parameters from the URLs of such resources before calculating the resource fingerprint:

```javascript
const isInlineResource = !resource.url;
if (isInlineResource) {
    return resource
}

const isGoogleAnalyticsResource = resource.url.includes('googletagmanager');
if (!isGoogleAnalyticsResource) {
    return resource
}

// Strip query string parameters from Google Analytics resource URLs.
const [urlWithoutQueryString] = resource.url.split('?');
return { ...resource, url: urlWithoutQueryString };
```

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
