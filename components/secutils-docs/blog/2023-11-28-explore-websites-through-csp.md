---
title: "Explore web applications through their content security policy (CSP)"
description: "Use Secutils.dev to import and inspect Content Security Policies from real-world apps: google.com, bing.com, duckduckgo.com, and ChatGPT. A practical look at CSP best practices, report-only mode, violation reporting, and the cost of unsafe directives."
slug: explore-websites-through-csp
authors: azasypkin
image: /img/blog/2023-11-28_import_policy_chatgpt_policy.png
tags: [thoughts, overview, technology]
keywords: [content security policy examples, csp google, csp bing, csp duckduckgo, csp chatgpt, csp report-only, csp violation reporting, secutils.dev csp import]
---
Hello!

I've finally wrapped up the feature development and fixes planned for the [**"Q4 2023 - Oct-Dec" milestone**](https://github.com/orgs/secutils-dev/projects/1/views/1) of [**Secutils.dev**](https://secutils.dev), a month earlier than expected. To make the changes more approachable than a wall of release notes, I want to demonstrate one of them in action: the new [**"Import content security policy"**](https://github.com/secutils-dev/secutils/issues/16) feature, by using it to learn a bit about the CSPs of websites you probably already use every day.

<!--truncate-->

:::info UPDATE (May 2026)
A few notes that don't change the substance of the post:

- The CSP screenshots below are **point-in-time snapshots from late 2023** and should be treated as such. Google, Bing, DuckDuckGo, and ChatGPT have all updated their headers since. Run the same import yourself today to see what they're actually serving now.
- The Secutils.dev CSP utility has gained support for **inheriting from an existing policy** (paste a raw policy string or fetch from a URL) and for **publicly sharing** a policy via a unique link, both of which make the workflow below faster.
- The `Cloudflare WAF` issue with ChatGPT in the bonus section is still common when fetching policies for sites behind a WAF, the "Serialized policy" tab is still the workaround.
:::

Alright, let's explore a few popular websites and uncover any new insights from the CSP they employ. To import the policy, head to [**Web Security → CSP**](https://secutils.dev/ws/web_security__csp) and click the `Import policy` button. Within the import modal, enter an arbitrary name, the webpage URL, and select the source from which to import the policy: `HTTP header (enforcing)`, `HTTP header (report only)`, or `HTML meta tag`.

![Import CSP dialog](/img/blog/2023-11-28_import_policy_dialog.png)

## Google

By default, [**Secutils.dev**](https://secutils.dev) allows importing CSP from the `Content-Security-Policy` HTTP header, matching the `HTTP header (enforcing)` policy source in the import dialog. However, attempting to import CSP from this source results in an error. Surprisingly, [**google.com**](https://google.com) doesn't transmit this header. Instead, it delivers CSP via the `Content-Security-Policy-Report-Only` HTTP header! It actually makes sense when you think about it: as the most visited website on the planet, they wouldn't want to accidentally disrupt anything with CSP changes, especially considering the need to support all web browsers. Instead, opting for a robust monitoring solution around CSP violation reporting seems more plausible. They might even set up alerts for suspicious activities that could indicate attempts to harm users through their website. This is, of course, speculative, but it seems to align logically.

Now, let's examine what [**google.com**](https://google.com) delivers in the `Content-Security-Policy-Report-Only` HTTP header:

![google.com content security policy](/img/blog/2023-11-28_import_policy_google.png)

The CSP used by Google appears quite reasonable to me, given their stature. Although having two unsafe directives isn't ideal, I trust Google's Security team extensively evaluated potential attack vectors and deemed the risk close to negligible. The inclusion of [**`report-sample`**](https://www.w3.org/TR/CSP/#violation-sample) can be incredibly useful when monitoring CSP violations since having a sample makes it much easier to understand how the policy was violated exactly.

Interestingly, CSP violations are directed to `https://csp.withgoogle.com/csp/gws/other-hp`. I hadn't encountered this website before! I encourage you to visit [**csp.withgoogle.com**](https://csp.withgoogle.com/docs/index.html) to explore this valuable resource dedicated to CSP and its necessity.

## Bing

Alright, let's take a look at Google's competitor, [**bing.com**](https://bing.com). And here's the first surprise - Bing doesn't utilize CSP at all! Who would've thought?

![bing.com content security policy](/img/blog/2023-11-28_import_policy_bing.png)

I can understand why certain websites resort to using unsafe CSP directive values, or why some opt for monitoring rather than strict enforcement. However, I can't seem to find any justification for Bing's complete absence of CSP! If anyone has a hint as to why, I'd love to hear it.

## DuckDuckGo

Okay, let's explore something more exotic, [**duckduckgo.com**](https://duckduckgo.com). And would you look at that!

![duckduckgo.com content security policy](/img/blog/2023-11-28_import_policy_duckduckgo.png)

Firstly, unlike Google, DuckDuckGo enforces CSP via the policy delivered within the `Content-Security-Policy` HTTP header. Secondly, DuckDuckGo's CSP employs a strict `none` default source, which is always a good practice.

Another notable aspect is that DuckDuckGo's CSP includes sources in the `*.onion` top-level domain. This might be because they maintain the same policy regardless of how the website is accessed. That seems logical.

However, it's disappointing that DuckDuckGo doesn't appear interested in analyzing CSP violations as they haven't set up any reporting. It's a letdown, but on the flip side, it would only make sense if they had the desire and resources to actively utilize this data.

## Bonus: ChatGPT

Of course, how can a blog post these days avoid mentioning AI? Let's peek at the CSP employed by ChatGPT. Can we? Apparently not! ChatGPT utilizes Cloudflare application protection, which blocks the [**Secutils.dev**](https://secutils.dev) HTTP client from executing a simple `HEAD` request to retrieve the policy. What a shame!

![ChatGPT Cloudflare WAF](/img/blog/2023-11-28_import_policy_chatgpt.png)

It's something I might address in the future. For now, let me turn this setback into an opportunity to demonstrate another method of importing content security policy to [**Secutils.dev**](https://secutils.dev), via serialized policy text. This process is a tad more complex as it involves manually capturing the policy using the browser's developer tools and then pasting it into the `Serialized policy` tab within the import dialog. Here's the CSP manually imported from ChatGPT:

![ChatGPT content security policy](/img/blog/2023-11-28_import_policy_chatgpt_policy.png)

Simply by examining a website's CSP, you can learn a lot about its construction and reliance on third-party solutions. It's a complex beast with many moving parts. Nothing particularly alarming here though. It's a pretty standard set for a modern application.

I also noticed that OpenAI collects CSP violation reports using the [**Datadog Content Security Policy integration**](https://docs.datadoghq.com/integrations/content_security_policy_logs/). However, having [**`unsafe-eval` and `unsafe-inline`**](https://www.w3.org/TR/CSP/#directive-script-src) in the scripts directive, especially for an application like ChatGPT, does raise concerns. Hopefully, there's a compelling reason behind this.

![ChatGPT content security policy reporting](/img/blog/2023-11-28_import_policy_chatgpt_reporting.png)

Ultimately, whether you're curious about how an application operates or conducting initial reconnaissance, overlooking CSP would be a mistake. Sometimes, it's like a treasure trove, revealing the inner workings and structure of an application.

## Frequently asked questions

### How do I import a CSP from a URL in Secutils.dev?

Go to [**Web Security → CSP**](https://secutils.dev/ws/web_security__csp), click **Import policy**, pick a name and the URL, and choose the source: `HTTP header (enforcing)`, `HTTP header (report only)`, or `HTML meta tag`. The full guide is [**here**](https://secutils.dev/docs/guides/web_security/csp).

### What if the target site is behind a WAF (e.g. Cloudflare)?

Use the **Serialized policy** tab in the import dialog. Capture the `Content-Security-Policy` (or `Content-Security-Policy-Report-Only`) header value yourself with browser dev tools and paste it in.

### Why do some sites use `Content-Security-Policy-Report-Only`?

Report-only mode reports violations without blocking the resource. It's a common choice for very high-traffic sites where the cost of accidentally breaking a feature outweighs the marginal protection of strict enforcement, as long as someone is actually triaging the violation reports.

### How can I monitor a deployed CSP for unexpected changes?

Today: a [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page) or an [**API tracker**](https://secutils.dev/docs/guides/web_scraping/api) pointed at the URL serving the header, with email notifications enabled. Native CSP-aware monitoring is on the roadmap (see [**"Security configuration management for software engineers"**](/blog/security-configuration-management)).

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
:::
