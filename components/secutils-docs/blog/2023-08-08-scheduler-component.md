---
title: Building a scheduler for a Rust application
description: "Building a scheduler for a Rust application: tokio, cron jobs, scheduler tests, job retries, persistent storage, SQLite."
slug: scheduler-component
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-08-08_scheduler_component_job_create.png
tags: [thoughts, overview, technology]
---
Hello!

As you might have learned from the [**"A Plan for the Q3 2023 Iteration"**](https://secutils.dev/docs/blog/q3-2023-iteration) post, my focus for this iteration is on adding support for automatic scheduled resource checks for the [**"Web Scraping → Page trackers"**](https://secutils.dev/docs/guides/web_scraping/page) utility in [**Secutils.dev**](https://secutils.dev). This work is already in progress, and in this post, I'd like to share more details about how I'm designing the scheduler for Secutils.dev. If you're building a scheduler for your application, hopefully, you can learn a useful thing or two.

<!--truncate-->

The scheduler is going to be one of the most important components of Secutils.dev. It will handle regular resource checks, monitor HTTP response headers and content security policy (CSP), manage user notifications, and much more. Therefore, it needs to be performant, reliable, and flexible.

:::note __UPDATE (Jan 10th, 2024)__

Since the initial version of this post, I've successfully integrated the scheduler into [**Secutils.dev**](https://secutils.dev) components:

- [**Web Scraping → Page trackers**](/docs/guides/web_scraping/page) utility that allows developers to detect and monitor the content or resources of **any** web page
- **Platform → Notifications** component that handles all notifications sent by Secutils.dev

Everything seems to be functioning smoothly!

Recently, I've added support for scheduler job retries (constant, exponential, and linear backoff strategies). For further details and the UI built for this functionality, check out the [**v1.0.0-alpha.4 release**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.4) in the release notes.

If you're looking for inspiration to build your own scheduler-like component in Rust or want to learn how to write unit tests for it, feel free to explore the source code at [**#secutils-dev/secutils/scheduler**](https://github.com/secutils-dev/secutils/tree/main/src/scheduler).
:::

## Performance

To achieve the best performance with minimal overhead, the scheduler should be tightly integrated with the main Secutils.dev server. This way, it can be reused for any functionality that requires repetitive or scheduled asynchronous work. After considering different options, I figured that implementing the scheduler as a part of the Secutils.dev server itself in Rust would provide the lowest overhead.

:::tip NOTE
Another important reason for integrating the scheduler into the Secutils.dev server instead of, for example, using [**Kubernetes cron jobs**](https://kubernetes.io/docs/concepts/workloads/controllers/cron-jobs/) is to simplify deployment for those who want to run Secutils.dev on their premises rather than relying on my fully-managed solution.
:::

In the Rust ecosystem, the go-to tool for writing something like this is [**Tokio**](https://github.com/tokio-rs/tokio) - an event-driven, non-blocking I/O platform for writing asynchronous applications with zero-cost abstractions, promising bare-metal performance.

## Flexibility

In addition to performance, the scheduler should also offer flexibility in job scheduling. It needs to support both one-time jobs and repetitive jobs with custom schedules. While many jobs will be scheduled and managed internally, I also want to provide users with the ability to configure schedules for some jobs with minimal effort.

For example, users should be able to manually set the schedule to periodically fetch resources from the web pages they are tracking. Initially, Secutils.dev might only provide simple options like scheduling jobs once a day or once a week. However, I want to give users more flexibility over time, allowing them to schedule resource checks for specific times, such as "midnight every Saturday" or other custom intervals.

As you might have guessed, I plan to have support for a [**crontab-like syntax**](https://en.wikipedia.org/wiki/Cron#Overview) in the scheduler. This syntax is powerful and familiar to many users, making it a great choice for providing advanced scheduling options.

## Reliability

Making sure the scheduler is reliable is super important for Secutils.dev because I roll out new versions all the time, and inevitably certain server pods or nodes will go offline. The scheduler must not be disrupted by these events and should continue running jobs at their expected schedules with minimal deviation as soon as the Secutils.dev server is up and running again.

To achieve this, the scheduler should rely on persistent storage for its jobs and state management. [**In the case of Secutils.dev**](https://secutils.dev/docs/blog/technology-stack-overview#database), the persistent storage mechanism is an SQLite database.

---

Now, for the last and not least requirement, the scheduler should be built without taking ages 😅 After a few hours of research, I stumbled upon a promising candidate for the job (pun intended): the [**Tokio cron scheduler**](https://github.com/mvniekerk/tokio-cron-scheduler)! It's a Rust crate that allows you to schedule tasks on Tokio using a cron-like annotation.

* ✅ Performance - It leverages Tokio under the hood.
* ✅ Flexibility - It supports both one-time and repetitive jobs with `crontab` syntax for the schedule.
* ✅ Reliability - It can optionally store jobs and state in PostgreSQL or NATS. Although it doesn't support SQLite out of the box, it allows consumers to implement their own storage, which I will do.
* ✅ Maintainability - Being open-source and having a permissive license, I can easily extend it if needed.

Here are a few diagrams from the project repository that explain how it works:

**Job creation**
![Job creation](https://secutils.dev/docs/img/blog/2023-08-08_scheduler_component_job_create.png)

**Job activity**
![Job activity](https://secutils.dev/docs/img/blog/2023-08-08_scheduler_component_job_activity.png)

Tokio cron scheduler ticked all the boxes for me! Its architecture is simple, and if I need to tweak it, I can do it without much trouble. Right now, I've already added an SQLite storage provider and started hooking up the scheduler with the [**"Web Scraping → Page trackers"**](https://secutils.dev/docs/guides/web_scraping/page) utility. Everything is going smoothly, and I hope to finish up the scheduled resources checks functionality in the next few weeks.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
