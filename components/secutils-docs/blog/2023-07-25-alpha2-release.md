---
title: Announcing 1.0.0-alpha.2 release
description: "Announcing 1.0.0-alpha.2 release: web page resources tracker, bug fixes, enhancements and more."
slug: alpha2-release
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [announcement, release, thoughts]
---

Hello!

This weekend, I finally wrapped up the "Q2 2023 – Apr-Jun" iteration and cut a new 1.0.0-alpha.2 release of [**Secutils.dev**](https://secutils.dev). Admittedly, this release was delayed "a bit" (well, almost 3 weeks delay, that happens) since I needed slightly more time to prepare the [**"Page tracker"**](https://secutils.dev/docs/guides/web_scraping/page) functionality for the general public. I tried to explain why it wasn't a trivial task in the "Detecting changes in JavaScript and CSS isn't an easy task" series of posts ([**part 1**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-1), [**part 2**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-2), [**part 3**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-3)). Check them out!

If you want to learn more about the "Page tracker" functionality, I encourage you to start from [**this guide**](https://secutils.dev/docs/guides/web_scraping/page). For your convenience, I'm also attaching a short video clip here demonstrating how it works using a "fake" HTML page backed by the [**"Responders" feature**](https://secutils.dev/docs/guides/webhooks). For the rest of the changes included in this release, please refer to the full changelog at [**secutils@v1.0.0-alpha.2**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.2).

<!--truncate-->

<video controls preload="metadata" width="100%">
  <source src="../video/guides/web_scraping_page_resources_tracker.webm" type="video/webm" />
  <source src="../video/guides/web_scraping_page_resources_tracker.mp4" type="video/mp4" />
</video>

Okay, I hope you'll find "Resources tracker" useful. Next, I'm going to share my thoughts about Secutils.dev releases and the type of work I'm currently prioritizing.

In general, I plan to release at least once every 3 months to include changes from every development iteration ("Q1 2023 – Jan-Mar", "Q2 2023 – Apr-Jun", etc.). It's a reasonable amount of time to deliver enough value in each release and, at the same time, not be overwhelmed with too frequent updates — I'm the only one working on Secutils.dev, after all.

I have two types of iterations: stabilization and new-feature work. The "Q2 2023 – Apr-Jun" was meant to be the first stabilization iteration after the [**initial release**](/docs/blog/beta-release) that happened at the end of May, and I was mostly focused on polishing UI, fixing bugs, tuning the database schema, and increasing the level of automation in my deployment workflow.

There's no need to explain why improving UI/UX and fixing bugs is important — nobody likes buggy and clumsy software. However, the significance of getting your database schema right and preparing a data migration strategy early might not be immediately obvious. The thing is, as soon as you have any real users who store any data in your database, you become quite limited in how far you can go in refactoring your data schema since you cannot afford to lose their data during an upgrade — there's no excuse at all for that. If you're in the early days of your software project like me, it's impossible to come up with the ideal data schema that you'll never change. So, by all means, make sure you have a plan for backing up and migrating user data during upgrades as soon as possible.

I explicitly mentioned that I've spent quite some time improving the automation of my development and deployment workflows. If you've ever bootstrapped any medium-to-large product on your own, you'll understand why: you have a very limited amount of time that you have to divide wisely between development, operations, support, talking to your users, and promoting your product. **Automating everything that can be automated** becomes an existential matter, especially if you also have other responsibilities in life, such as a family, a full-time job, or other commitments.

The ~~next~~ current [**"Q3 2023 – July-Sep"**](https://github.com/orgs/secutils-dev/projects/1/views/1) iteration will be mostly dedicated to new feature work. I'm really excited about that!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
