---
title: Q2 2023 update - Web resources tracker
description: "Q2 2023 update: web resources trackers, track JavaScript and CSS files, protect from supply chain attacks and detect broken deployment early."
slug: q2-2023-update-resources-tracker
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-06-15_resources_trackers.png
tags: [overview, technology, application-security]
---
Hello!

As the end of "Q2 2023 - Apr-Jun" milestone (that's how I structure [my roadmap](https://github.com/orgs/secutils-dev/projects/1/views/1)) is quickly approaching, I wanted to give a quick update on the progress so far. One of the significant deliverables for this milestone is a functional web resources tracker utility. The utility should give developers the ability to track resources of any web page. You may be wondering why they would want to do that and how it relates to security. Let me explain using two personas: the developer and the security researcher.

<!--truncate-->

![Resources trackers](https://secutils.dev/docs/img/blog/2023-06-15_resources_trackers.png)

Imagine you're developing a web page, and after you deploy it, you accidentally notice that it requires some scripts or stylesheets that you didn't expect. Or perhaps it stops requiring something that used to be essential for your application. In most cases, it's probably due to a broken production build or deployment pipeline. However, it could also be an indicator that a malicious actor has successfully compromised your web page and modified it to require resources needed to attack your users. It could be a stored XSS or a supply chain attack, among others.

Unexpected new or missing resources are relatively easy to spot for a non-complex web page, assuming you're constantly monitoring it. But what if malicious content is side-loaded into your existing resources? In this case, the only indicator that something fishy is going on would be a change in size or fingerprint of the particular resource. Now, this would be incredibly hard to spot for a human, but a software tool can do it without any problem.

Whether the underlying reason is a targeted attack or just a broken build, you do want to know about the issue and address it as quickly as possible before real damage occurs.

I hope it's clear now how automated web resource tracking can be useful to developers. But if you're a security researcher, you can benefit from it too. If you focus on discovering and understanding potential security flaws of third-party web applications, you might want to be notified when the application resources change. It could be a sign that the application rolled out an upgrade, and it might be a good time to go and poke holes in it.

**Fun fact:** While testing this functionality on [Secutils.dev](https://secutils.dev/) Web UI, I caught the misspelled name of the Plausible usage analytics script in my development environment 🤦 The functionality isn't yet released, but it's already providing value!

It will take quite a bit of time and iterations to implement all the ideas I have regarding this feature. For the initial release, I'm planning to implement the most basic functionality: the web-scraper component (done, see [secutils-dev/retrack](https://github.com/secutils-dev/retrack)), the UI to register a web page to track resources (in progress), and a way to *manually* trigger the re-fetching of resources (not started yet). In this release, I'm focusing on the resources that usually include a sizeable chunk of application business logic and are therefore the most useful for the target audience: JavaScript and CSS. However, it should eventually support more resources like images, videos, etc.

My plan is to build a web scraper component on top of [Playwright](https://playwright.dev/) since I need to handle both resources that are statically defined in the HTML and those that are loaded dynamically. Leveraging Playwright, backed by a real browser, instead of parsing the static HTML opens up a ton of opportunities to turn a simple web resource scraper into a much more intelligent tool to handle all sorts of use cases: recording and replaying HARs, imitating user activity, and more.

I'll share more updates and insights as I progress. Stay tuned!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
