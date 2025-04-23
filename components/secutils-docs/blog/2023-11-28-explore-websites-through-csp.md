---
title: "Explore web applications through their content security policy (CSP)"
description: "Explore web applications through their content security policy (CSP): import and parse CSP of google.com, bing.com, duckduckgo.com, and ChatGPT. Content Security Policy best practices."
slug: explore-websites-through-csp
authors: azasypkin
image: /img/blog/2023-11-28_import_policy_chatgpt_policy.png
tags: [thoughts, overview, technology]
---
Hello!

I've finally wrapped up the feature development and fixes planned for the [**"Q4 2023 – Oct-Dec" milestone**](https://github.com/orgs/secutils-dev/projects/1/views/1) of [**Secutils.dev**](https://secutils.dev), a month earlier than expected! It feels good to be getting better at estimating my own work 🙂 I still need to update documentation and create a few demo videos for the new functionality, but that should be the easy part. Hopefully, I can release a new version in a week or so.

Like anything we invest our time and energy in, I want to raise awareness about the work I've done, gauge interest, and hopefully receive constructive feedback. I'm not a fan of blunt self-promotion, so I'm going to try something different - demonstrating new features in action. Sometimes I'll show their business value, and other times it'll just be for fun and entertainment. In this post, I'll demonstrate how to use the new [**“Import content security policy”**](https://github.com/secutils-dev/secutils/issues/16) feature to learn a bit more about the websites you use every day. Let's dive in!

<!--truncate-->

Alright, let's explore a few popular websites and uncover any new insights from the CSP they employ. To import the policy, head to [**Web Security → CSP → Policies**](https://secutils.dev/ws/web_security__csp__policies) and click the `Import policy` button. Within the import modal, enter an arbitrary name, the webpage URL, and select the source from which to import the policy: `HTTP header (enforcing)`, `HTTP header (report only)`, or `HTML meta tag`.

![Import CSP dialog](/img/blog/2023-11-28_import_policy_dialog.png)

## Google

By default, [**Secutils.dev**](https://secutils.dev) allows importing CSP from the `Content-Security-Policy` HTTP header, matching the `HTTP header (enforcing)` policy source in the import dialog. However, attempting to import CSP from this source results in an error. Surprisingly, [**google.com**](https://google.com) doesn't transmit this header. Instead, it delivers CSP via the `Content-Security-Policy-Report-Only` HTTP header! It actually makes sense when you think about it: as the most visited website on the planet, they wouldn't want to accidentally disrupt anything with CSP changes, especially considering the need to support all web browsers. Instead, opting for a robust monitoring solution around CSP violation reporting seems more plausible. They might even set up alerts for suspicious activities that could indicate attempts to harm users through their website. This is, of course, speculative, but it seems to align logically.

Now, let's examine what [**google.com**](https://google.com) delivers in the `Content-Security-Policy-Report-Only` HTTP header:

![google.com content security policy](/img/blog/2023-11-28_import_policy_google.png)

The CSP used by Google appears quite reasonable to me, given their stature. Although having two unsafe directives isn't ideal, I trust Google's Security team extensively evaluated potential attack vectors and deemed the risk close to negligible. The inclusion of [**`report-sample`**](https://www.w3.org/TR/CSP/#violation-sample) can be incredibly useful when monitoring CSP violations since having a sample makes it much easier to understand how the policy was violated exactly.

Interestingly, CSP violations are directed to `https://csp.withgoogle.com/csp/gws/other-hp`. I hadn't encountered this website before! I encourage you to visit [**csp.withgoogle.com**](https://csp.withgoogle.com/docs/index.html) to explore this valuable resource dedicated to CSP and its necessity.

## Bing

Alright, let's take a look at Google's competitor - [**bing.com**](https://bing.com). And here's the first surprise - Bing doesn't utilize CSP at all! Who would've thought?

![bing.com content security policy](/img/blog/2023-11-28_import_policy_bing.png)

I can understand why certain websites resort to using unsafe CSP directive values, or why some opt for monitoring rather than strict enforcement. However, I can't seem to find any justification for Bing's complete absence of CSP! If anyone has a hint as to why, I'd love to hear it.

## DuckDuckGo

Okay, let's explore something more exotic - [**duckduckgo.com**](https://duckduckgo.com). And would you look at that!

![duckduckgo.com content security policy](/img/blog/2023-11-28_import_policy_duckduckgo.png)

Firstly, unlike Google, DuckDuckGo enforces CSP via the policy delivered within the `Content-Security-Policy` HTTP header. Secondly, DuckDuckGo's CSP employs a strict `none` default source, which is always a good practice.

Another notable aspect is that DuckDuckGo's CSP includes sources in the `*.onion` top-level domain. This might be because they maintain the same policy regardless of how the website is accessed. That seems logical.

However, it's disappointing that DuckDuckGo doesn't appear interested in analyzing CSP violations as they haven't set up any reporting. It's a letdown, but on the flip side, it would only make sense if they had the desire and resources to actively utilize this data.

## Bonus: ChatGPT

Of course, how can a blog post these days avoid mentioning AI? Let's peek at the CSP employed by ChatGPT. Can we? Apparently not! ChatGPT utilizes Cloudflare application protection, which blocks the [**Secutils.dev**](https://secutils.dev) HTTP client from executing a simple `HEAD` request to retrieve the policy. What a shame!

![ChatGPT Cloudflare WAF](/img/blog/2023-11-28_import_policy_chatgpt.png)

It's something I might address in the future. For now, let me turn this setback into an opportunity to demonstrate another method of importing content security policy to [**Secutils.dev**](https://secutils.dev) - via serialized policy text. This process is a tad more complex as it involves manually capturing the policy using the browser's developer tools and then pasting it into the `Serialized policy` tab within the import dialog. Here's the CSP manually imported from ChatGPT:

![ChatGPT content security policy](/img/blog/2023-11-28_import_policy_chatgpt_policy.png)

Simply by examining a website's CSP, you can learn a lot about its construction and reliance on third-party solutions. It's a complex beast with many moving parts. Nothing particularly alarming here though. It's a pretty standard set for a modern application.

I also noticed that OpenAI collects CSP violation reports using the [**Datadog Content Security Policy integration**](https://docs.datadoghq.com/integrations/content_security_policy_logs/). However, having [**`unsafe-eval` and `unsafe-inline`**](https://www.w3.org/TR/CSP/#directive-script-src) in the scripts directive, especially for an application like ChatGPT, does raise concerns. Hopefully, there's a compelling reason behind this.

![ChatGPT content security policy reporting](/img/blog/2023-11-28_import_policy_chatgpt_reporting.png)

Ultimately, whether you're curious about how an application operates or conducting initial reconnaissance, overlooking CSP would be a mistake. Sometimes, it's like a treasure trove, revealing the inner workings and structure of an application.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
:::
