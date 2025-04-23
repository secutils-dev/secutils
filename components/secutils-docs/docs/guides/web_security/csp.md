---
sidebar_position: 1
sidebar_label: Content Security Policies
title: Web Security ➔ Content security policies (CSP)
description: Learn how to create and use content security policies (CSP) in Secutils.dev.
---

# What is a Content Security Policy?

Content Security Policy (CSP) is an added layer of security that helps to detect and mitigate certain types of attacks, including Cross-Site Scripting (XSS) and data injection attacks. These attacks are used for everything from data theft, to site defacement, to malware distribution.

Generally, to enable CSP, you need to configure your web server to return the **Content-Security-Policy** HTTP header or HTML meta tag. For more details, refer to [MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP) and [OWASP](https://owasp.org/www-community/controls/Content_Security_Policy).

On this page, you can find guides on creating Content Security Policies that match your specific needs.

## Create a Content Security Policy

In this guide you'll create a simple Content Security Policy template that allows you to generate policies that are ready to be applied to any web application:

1. Navigate to [Web Security → CSP → Policies](https://secutils.dev/ws/web_security__csp__policies) and click **Create policy** button
2. Configure a new policy with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
secutils.dev
```
</td>
</tr>
<tr>
<td><b>Default source (default-src)</b></td>
<td>
```
'self', api.secutils.dev
```
</td>
</tr>
<tr>
<td><b>Style source (style-src)</b></td>
<td>
```
'self', fonts.googleapis.com
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the policy
4. Once the policy is set up, it will appear in the policies grid
5. Click the policy's **Copy policy** button and use **Policy source** dropdown to switch between different policy representations:

<table class="su-table">
<tbody>
<tr>
<td><b>HTTP header (enforcing)</b></td>
<td>
```
Content-Security-Policy: default-src 'self' api.secutils.dev; style-src 'self' fonts.googleapis.com
```
</td>
</tr>
<tr>
    <td><b>HTML meta tag</b></td>
<td>

```html
<meta http-equiv="Content-Security-Policy" content="default-src 'self' api.secutils.dev; style-src 'self' fonts.googleapis.com">
```
</td>
</tr>
</tbody>
</table>

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_security_csp_new_policy.webm" type="video/webm" />
  <source src="../../video/guides/web_security_csp_new_policy.mp4" type="video/mp4" />
</video>

## Import a Content Security Policy from URL

In this guide you'll import a Content Security Policy from an external URL:

1. Navigate to [Web Security → CSP → Policies](https://secutils.dev/ws/web_security__csp__policies) and click **Import policy** button
2. Pick **URL** tab and use the following values for import:

<table class="su-table">
<tbody>
<tr>
<td><b>Policy name</b></td>
<td>
```
Google CSP
```
</td>
</tr>
<tr>
<td><b>URL</b></td>
<td>
```
https://google.com
```
</td>
</tr>
<tr>
<td><b>Policy source</b></td>
<td>
```
HTTP header (report only)
```
</td>
</tr>
</tbody>
</table>

3. Click the **Import** button to import the policy
4. Once the policy is imported, it will appear in the policies grid
5. Refer to the **Policy** grid column or policy edit flyout to view the content of the imported policy

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_security_csp_import_policy_url.webm" type="video/webm" />
  <source src="../../video/guides/web_security_csp_import_policy_url.mp4" type="video/mp4" />
</video>

## Import a Content Security Policy from a string

In this guide you'll import a Content Security Policy from a string (serialized policy text):

1. Navigate to [Web Security → CSP → Policies](https://secutils.dev/ws/web_security__csp__policies) and click **Import policy** button
2. Pick **Serialized policy** tab and use the following values for import:

<table class="su-table">
<tbody>
<tr>
<td><b>Policy name</b></td>
<td>
```
Custom CSP
```
</td>
</tr>
<tr>
<td><b>Serialized policy</b></td>
<td>
```
default-src 'self' api.secutils.dev; style-src 'self' fonts.googleapis.com
```
</td>
</tr>
</tbody>
</table>

3. Click the **Import** button to import the policy
4. Once the policy is imported, it will appear in the policies grid
5. Refer to the **Policy** grid column or policy edit flyout to view the content of the imported policy

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_security_csp_import_policy_string.webm" type="video/webm" />
  <source src="../../video/guides/web_security_csp_import_policy_string.mp4" type="video/mp4" />
</video>

## Test a Content Security Policy

In this guide, you will create a Content Security Policy and test it using a custom HTML responder:

1. First, navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
2. Configure a new responder with the following values to respond with a simple HTML page that uses **eval()** function to evaluate JavaScript code represented as a string:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
CSP Test
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/csp-test
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
    <td><b>Body</b></td>
<td>
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <title>Evaluate CSP</title>
</head>
<body>
<label for="eval-input">Expression to evaluate:</label>
<input id="eval-input" type="text" value="alert('xss')"/>
<button id="eval-test">Eval</button>
<script type="text/javascript" defer>
  (async function main() {
    const evalTestBtn = document.getElementById('eval-test');
    evalTestBtn.addEventListener('click', () => {
      const evalExpression = document.getElementById('eval-input');
      window.eval(evalExpression.value);
    });
  })();
</script>
</body>
</html>
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the responder
4. Once the responder is set up, it will appear in the responders grid along with its unique URL
5. Click the responder's URL and use **Eval** button on the rendered page to see that nothing prevents you from using **eval()** function
6. Now, navigate to [Web Security → CSP → Policies](https://secutils.dev/ws/web_security__csp__policies) and click **Create policy** button to create a Content Security Policy to forbid **eval()**
7. Configure a new policy with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
CSP Test
```
</td>
</tr>
<tr>
<td><b>Script source (script-src)</b></td>
<td>
```
'self', 'unsafe-inline'
```
</td>
</tr>
</tbody>
</table>

8. Click the **Save** button to save the policy
9. Once the policy is set up, it will appear in the policies grid
10. Click the policy's **Copy policy** button and use **Policy source** dropdown to switch to **HTML meta tag** policy representation
11. Copy `<meta>` HTML tag with the policy and navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) again
12. Edit **Body** property of the previously created **CSP Test** responder to include `<meta>` HTML tag with the policy inside `<head>` HTML tag
13. Click the **Save** button and navigate to the responder's URL again
14. This time, when you click the **Eval** button, nothing happens and an error message is logged in the browser console meaning that you have successfully forbidden **eval()** with the Content Security Policy

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_security_csp_test_policy.webm" type="video/webm" />
  <source src="../../video/guides/web_security_csp_test_policy.mp4" type="video/mp4" />
</video>

## Report Content Security Policy violations

In this guide, you will create a Content Security Policy and collect its violation reports using a custom tracking responder:

1. Navigate to [Web Security → CSP → Policies](https://secutils.dev/ws/web_security__csp__policies) and click **Create policy** button
2. Configure a new policy with the following values:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
CSP Reporting
```
</td>
</tr>
<tr>
<td><b>Script source (script-src)</b></td>
<td>
```
'self', 'unsafe-inline', 'report-sample'
```
</td>
</tr>
<tr>
<td><b>Report to (report-to)</b></td>
<td>
```
default
```
</td>
</tr>
</tbody>
</table>

3. Click the **Save** button to save the policy
4. Once the policy is set up, it will appear in the policies grid
5. Click the policy's **Copy policy** button, switch **Policy source** to **HTTP header (enforcing)** and copy generated policy header
6. Now, navigate to [Webhooks → Responders](https://secutils.dev/ws/webhooks__responders) and click **Create responder** button
7. Configure a new responder with the following values to collect CSP violation reports:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
CSP Reporting
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/csp-reporting
```
</td>
</tr>
<tr>
<td><b>Method</b></td>
<td>
```
POST
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
</tbody>
</table>

8. Click the **Save** button and copy responder's URL
9. Click **Create responder** button once again
10. Configure another responder with the following values to respond with a simple HTML page that will try to use **eval()** function to evaluate JavaScript code represented as a string:

<table class="su-table">
<tbody>
<tr>
<td><b>Name</b></td>
<td>
```
CSP Eval Test
```
</td>
</tr>
<tr>
<td><b>Path</b></td>
<td>
```
/csp-eval-test
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
Reporting-Endpoints: default="[**REPLACE WITH `CSP Reporting` responder URL**]",
Content-Security-Policy: [*REPLACE WITH `CSP Reporting` policy content**],
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
  <title>Evaluate CSP</title>
</head>
<body>
<label for="eval-input">Expression to evaluate:</label>
<input id="eval-input" type="text" value="alert('xss')"/>
<button id="eval-test">Eval</button>
<script type="text/javascript" defer>
  (async function main() {
    const evalTestBtn = document.getElementById('eval-test');
    evalTestBtn.addEventListener('click', () => {
      const evalExpression = document.getElementById('eval-input');
      window.eval(evalExpression.value);
    });
  })();
</script>
</body>
</html>
```
</td>
</tr>
</tbody>
</table>

11. Click the **Save** button to save the responder
12. Once the responder is set up, it will appear in the responders grid along with its unique URL
13. Click the responder's URL to navigate to the test page
14. On the test page, click the **Eval** button and notice that nothing happens except that browser logs an error message in its console meaning that you have successfully forbidden **eval()** with the Content Security Policy
15. Go back to the responder's grid and expand the **CSP Reporting** responder to view the CSP violation report that browser has sent when you tried to use **eval()**

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_security_csp_report_policy_violations.webm" type="video/webm" />
  <source src="../../video/guides/web_security_csp_report_policy_violations.mp4" type="video/mp4" />
</video>

## Share a Content Security Policy

This guide will walk you through sharing a Content Security Policy template publicly, allowing anyone on the internet to view it:

1. Navigate to [Web Security → CSP → Policies](https://secutils.dev/ws/web_security__csp__policies) and pick the policy you'd like to share
2. Click the policy's **Share policy** button and toggle **Share policy** switch to **on** position
3. Once the policy is shared, the dialog will show a **Copy link** button
4. Click the **Copy link** button to copy a unique shared policy link to your clipboard
5. To stop sharing the policy, click the **Share policy** button again, and switch the **Share policy** toggle to the **off** position.

Watch the video demo below to see all the steps mentioned earlier in action:

<video controls preload="metadata" width="100%">
  <source src="../../video/guides/web_security_csp_policy_share.webm" type="video/webm" />
  <source src="../../video/guides/web_security_csp_policy_share.mp4" type="video/mp4" />
</video>
