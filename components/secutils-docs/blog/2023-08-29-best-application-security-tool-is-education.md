---
title: "The best application security tool is education"
description: "The best application security tool is education: internal security trainings, hackathons, SAST, DAST, and more."
slug: best-application-security-tool-is-education
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-08-29_best_application_security_tool_is_education.png
tags: [thoughts, application-security]
---
Hello!

:::tip NOTE
Although not directly related to this topic, I encourage you to take a look at the latest [**US national cyber security workforce and education strategy from July 31, 2023**](https://www.whitehouse.gov/wp-content/uploads/2023/07/NCWES-2023.07.31.pdf). The thumbnail picture for this post is taken from there. It's an interesting read!
:::

As you might have guessed, I spend a lot of time thinking about application security - almost every day, in fact. At my day job, I'm constantly pondering how to enhance [**Kibana's**](https://github.com/elastic/kibana) security in a scalable manner without overburdening my already hardworking team. Outside of work, I'm equally dedicated to making [**Secutils.dev**](https://secutils.dev) even more valuable to fellow engineers looking for better security tools.

While I'd love to tell you there's a magic tool or a combination of tools that can make your application completely secure, I don't believe it's quite that simple - at least not yet. If you're working within tight budget constraints, resist the urge to spend it all on solutions like Veracode, Snyk, Secutils.dev, or any other security tool. Also, don't obsess over supply chain security and penetration testing just yet. Instead, focus your initial investment on something absolutely critical - educating your engineers about security. You'll reap the rewards, and so will your team. Only once you have a solid educational program or processes in place should you consider investing in additional security-oriented tools.

<!--truncate-->

I do appreciate various application security tools. Many of them are great and significantly simplify my life as a security engineer. However, none of these tools can prevent developers from inadvertently exposing sensitive information in logs, accidentally sending error stack traces in API responses, caching authorization results in memory, hardcoding S3 bucket names and URLs for third-party services, or neglecting to invalidate access tokens when they are no longer necessary.

This **isn't** a fault of the developers though. They might not even be aware that what they're doing could be exploited by a malicious actor with sufficient motivation. Engineers are a creative group, and they can unintentionally craft a vulnerable piece of code in a way that prevents detection by any code or security scanner. While it's clear to engineers that code should be performant (just think of all these technical interview questions and tasks related to code performance, complexity, and efficiency), it's unfortunately less evident that code should also be as secure, if not more so.

Many of my colleagues and I have experimented with various approaches to enhance the security situation: conducting code security reviews, writing documentation outlining security best practices, advocating for more secure programmatic APIs, and incorporating tools like Snyk and CodeQL. These initiatives certainly help, but they don't always scale efficiently unless engineers begin to consider security with the same seriousness as they do performance and maintainability.

Historically, security was often an afterthought for the majority of software engineers. This was primarily because security breaches and data leaks were not as frequent, dramatic, or devastating as they are today. As more aspects of our lives become digital, and this shift continues to accelerate, along with the slow but [**steady evolution of government regulations**](https://www.whitehouse.gov/wp-content/uploads/2023/07/National-Cybersecurity-Strategy-Implementation-Plan-WH.gov_.pdf), I believe, we should change this perception through education. This education should extend to specific teams, organizations, and entire company, and the investment will undoubtedly pay off over time.

If you really care about security, make software security trainings **mandatory** and **recurring**, just like other compulsory training sessions related to topics like equal treatment and work ethic, which are common today. All these training sessions are important and can literally impact people's lives. Also, actively ask for a feedback and continually update the training content. Admittedly, it's easier said than done, but...

When you onboard a new engineer, consider sending them something more meaningful than just another useless branded mug – perhaps a book on application security or a prepaid voucher for a security training course. This gesture will send a clear message from day one that security is an important concern for your company, not merely a nice-to-have. And I **beg** you, if you offer security training or host internal security-focused hackathons, avoid making them optional or scheduling them after working hours. Otherwise, they won't be taken as seriously – I know I wouldn't.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
