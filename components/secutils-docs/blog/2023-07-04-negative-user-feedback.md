---
title: First negative user feedback
description: "First negative user feedback: value and conclusions."
slug: negative-user-feedback
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [thoughts]
---
Hello!

Just a brief post today to highlight an important milestone for [**Secutils.dev**](https://secutils.dev/) - I recently received the first negative user feedback! It may not sound like something to celebrate or take pride in, but I view it differently. Over the past month, I've received a bit of unsolicited positive feedback, primarily from fellow builders and indie hackers. Their input has been valuable, and I do appreciate it. However, I have to admit that people like me have a higher tolerance for work-in-progress software, imperfections, and bugs. Moreover, solo-builders tend to be incredibly supportive of one another, much like parents with young children empathizing with other parents in similar situations 🙂

But the negative feedback I received this past weekend came from a "real user," which presents a different perspective altogether.

<!--truncate-->

The feedback primarily focuses on the [**Content-Security-Policy (CSP) utility**](https://secutils.dev/docs/guides/web_security/csp) and how it fails to meet the user's expectations. Here's an excerpt (slightly edited for brevity):

> "... yes, the tool helps me create a policy and provides some guidance, which is nice, but now what? I want to know if the policy is sufficient and secure, and for that, I have to visit two other websites... Instead of the promised all-in-one solution, I'm left with three different tools, each with a different interface..."

This is a very valid point that immediately challenges my core assumptions about the target audience. I developed this utility under the assumption that it would be used as part of security configuration management flows, with users generally having knowledge of what their policy should entail and why. In this context, the ease of policy creation and maintenance, along with the ability to detect issues throughout the policy's lifespan, would be of utmost importance.

However, it appears that there is a group of users looking for a slightly different functionality. They want a tool that not only makes policy creation and maintenance easy but also offers guidance on what to include or not in the policy. I still need to identify the specific "persona" for this user group to understand their motivations. They may be engineers who are new to CSP or information security specialists assessing a product's security posture, without requiring in-depth knowledge of CSP. Understanding the motivation behind the feedback is important as it will determine how I respond to it. This could mean incorporating basic CSP validation features from existing tools, making cross-tool integration seamless or, perhaps, using ChatGPT or a similar AI service to answer questions like "Is my policy secure enough?". I don't know yet and need a better understanding of the user before making a decision.

Another interesting observation from this feedback is the potential demand (although based on just one data point…) for an all-in-one CSP utility that either doesn't currently exist or doesn't meet users' expectations. This could be an opportunity worth exploring further!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
