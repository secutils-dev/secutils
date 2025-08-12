---
title: "Announcing 1.0.0-alpha.3 release: more powerful resource tracking, notifications and content sharing"
description: "Announcing 1.0.0-alpha.3 release: more powerful resource tracking, notifications and content sharing."
slug: alpha3-release
authors: azasypkin
image: /img/blog/2023-10-04_resources_trackers_enhancements.png
tags: [announcement, release, overview]
---
Hello!

Earlier this week, I wrapped up the [**"Q3 2023 – Jul-Sep"**](https://github.com/orgs/secutils-dev/projects/1/views/1) iteration and cut a new [**1.0.0-alpha.3 release**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.3) of [**Secutils.dev**](https://secutils.dev). In this post, I would like to quickly walk you through the major changes since [**1.0.0-alpha.2**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.2): notifications, more powerful web page resource tracker, sharing capabilities and more. Let’s dive in!

<!--truncate-->

## Scheduled resources checks

If you’ve read my previous posts or tried Secutils.dev [**web page resources tracker**](https://secutils.dev/docs/guides/web_scraping/page) functionality, you might recall that users were required to manually trigger resource checks. With this release, you have an option to schedule automatic resources checks to be performed hourly, daily, weekly, or monthly! When you configure the web page resource tracker, you define how many resource revisions Secutils.dev should store so that you can view the diff between two consecutive revisions. Once the limit is reached, the next revision will displace the oldest one.

![Scheduled resources checks](/img/blog/2023-10-04_scheduled_resource_checks.png)

## Email notifications for changed resources

Since previously you were supposed to manually trigger immediate resource checks, it wouldn't make much sense to send you any additional notifications about detected changes. You'll be presented with the check result in the UI as soon as the check is complete without losing context. However, the automatic scheduled resource checks change the control flow, where Secutils.dev should perform the check regularly and notify you if it detects any changes. In the latest release, you can opt in to email notifications, and Secutils.dev will email you if it detects any changes in resources.

![Email notifications for changed resources](/img/blog/2023-10-04_email_notifications.png)

## Custom resources filtering and mapping

Modern web pages can contain numerous resources, and tracking changes for all of them may not be always necessary. Additionally, certain resources, like those injected by web page analytics solutions, can change with every page load, potentially leading to excessive notifications. In such cases, you'll likely want to filter out irrelevant resources or focus on specific ones.

In more advanced scenarios, you might be interested in only a portion of a web page resource. For instance, there could be scripts bundling multiple third-party libraries, with changes in some libraries being more important than others. It would be convenient to have the ability to trim or "map" these resources into more meaningful resources.

I explored various approaches to address these use cases in the simplest way possible, but there were always complex edge cases that required a change in direction. However, considering that the primary audience for Secutils.dev is software developers, I decided that introducing some complexity could offer much greater flexibility.

As mentioned in [**this post**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-1#challenge-2-dynamically-loaded-resources), I use Playwright (with Chromium) to extract web page resources. While this choice adds complexity to implementation, security, and deployment, it grants quite a bit of flexibility. With Playwright, I can access, intercept, or modify virtually everything on the tracked web page. Notably, Playwright allows me to inject custom JavaScript scripts into a web page. Rather than inventing my own syntax/parser for custom user resource filters and mapping rules, I can provide users with the full power of _modern_ JavaScript executed within the latest available browser. The only constraint is that users must adhere to the input and output interfaces expected by Secutils.dev.

![Custom resources filtering and mapping](/img/blog/2023-10-04_custom_resources_filtering.png)

The potential applications of this approach are vast. I'm already planning to extend it to cover more utilities and use cases, such as tracking changes in page content, not just resources. Imagine a change tracker for virtually anything on the web!

## Sharing & collaboration

In today's world, it's challenging to envision a software engineer or security researcher working entirely in isolation. As software systems grow in size and complexity, collaboration becomes essential. That's one of the reasons why collaboration software is on the rise, with built-in collaboration features becoming increasingly common.

While it may be too early to implement full-fledged two-way collaboration functionality in Secutils.dev, I recognize that the absence of such features could limit the tool's adoption. Therefore, I'm planning to gradually introduce collaboration-related features in each iteration, starting with the "one-way" sharing functionality released in [**1.0.0-alpha.3 release**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.3). With this release, you can share created content security policies with anyone on the internet, even if they don't have a Secutils.dev account.

In the future, I intend to expand this sharing functionality to include digital certificate templates and tracked web page resources.

![Sharing & collaboration](/img/blog/2023-10-04_sharing.png)

## Other enhancements and bug fixes

In addition to the major features mentioned above, this release also includes several smaller enhancements. These include extending the digital certificate editor to allow users to configure private key size (for RSA and DSA) and elliptic curve name (for ECDSA).

As previously mentioned, while the resource tracker functionality has become more powerful, it also comes with increased security risks. Therefore, I've made security enhancements for Docker images for all Secutils.dev components, and the Web Scraper component itself. I've covered this in more detail in my [**Running web scraping service securely**](https://secutils.dev/docs/blog/running-web-scraping-service-securely) post.

You can find the full change log here: [**changelog#1.0.0-alpha.3**](https://secutils.dev/docs/project/changelog/#100-alpha3)

In the next few days, I'll be prioritizing work for the upcoming "Q4 2023 – Oct-Dec" iteration. In my next post, I'll provide more details on what I'll be focusing on during this period.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
