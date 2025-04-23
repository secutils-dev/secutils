---
title: "Two simple rules for better and more secure code"
description: "Two simple rules for better and more secure code: logs, errors, HTTP, log injections, PII, sensitive information, leaked credentials, hacks."
slug: two-simple-rules-for-secure-code
authors: azasypkin
image: /img/blog/2023-11-07_easy_rules_secure_code.png
tags: [thoughts, overview, technology]
---
Hello!

In one of my previous posts, [**"The best application security tool is education"**](https://secutils.dev/docs/blog/best-application-security-tool-is-education), I discussed why educating yourself or your engineers about security can yield the highest return on investment, especially if you have a limited budget. However, I understand that learning or teaching security is not as straightforward as it sounds. Every organization has its unique characteristics, and every engineer has their own distinct qualities. Moreover, internalizing secure coding practices is a time-consuming process. If you're just starting on this journey, I'm here to share two very simple rules that are easy to remember and have the potential to significantly enhance the security of the code you or your colleagues write. So, let's dive in!

<!--truncate-->

## Don't log what you don't know

If you're working on a non-trivial application, chances are you're logging various pieces of information. Logs are essential for understanding your software and are indispensable during debugging. In fact, the more application and operational information you log, the better prepared you are to debug any issues that your application might encounter.

However, the rule I want you to keep in mind is this: every time you consider logging something, ask yourself these two critical questions:

* **Is the data I'm logging sensitive?** For instance, does it include credentials, request or response headers, or any information that could potentially identify my users, like client IPs or user names? In short, anything that falls under the category of [**Personal Identifiable Information (PII)**](https://www.investopedia.com/terms/p/personally-identifiable-information-pii.asp).
* **If I'm logging a complex data structure, do I fully understand and trust all the fields that will be logged?**

Asking these questions becomes especially crucial when you're dealing with data that originates from users or external services you don't have full control over. This is important because the data you log might eventually end up elsewhere, such as in Elasticsearch, backed up in the cloud, or stored locally for an unknown period. It's not uncommon for these secondary locations to be less secure than the environment where the logs were initially captured. This increases the risk, sometimes significantly, that the logs might be accessed by individuals who weren't meant to see them, even months after the logs were recorded. This is a potential problem, not to mention the possibility of leaks and hacks, which can also occur.

Leaking sensitive information through logs is one problem, but keep in mind that mishandled logs can also be weaponized, as explained in the [**Log injection article from OWASP**](https://owasp.org/www-community/attacks/Log_Injection).

In summary, please avoid blindly logging everything and, instead, select carefully based on what is safe and genuinely necessary.

## Don't expose raw errors

I have to admit that I see it far more frequently than I would like: an engineer carefully crafts a successful response, selects only the necessary data for return, and wisely sanitizes untrusted data. However, at the same time, they completely neglect the errors their code generates and throws.

Errors returned to users or consumers of APIs can sometimes contain as much, if not more, sensitive data compared to a successful response. Stack traces, file system paths, URLs with embedded credentials, internal addresses, and environment variables with secrets are just a few examples of what might be included in these errors, inadvertently leaking from your software.

The rule I want to emphasize is simple: strive to handle errors internally, and log the relevant error details when necessary. But, when it comes to an error that should be visible to your users or API consumers, craft a custom, safe, and actionable error message instead.

:::caution NOTE
The two rules I've discussed may seem simple and reasonable, but you'd be surprised at how often they are neglected, leading to serious security incidents that can cost organizations thousands or even tens of thousands of dollars in remediation and associated expenses. Forewarned is forearmed!
:::

That wraps up today's post, thanks for taking the time to read it! If you found this post helpful or interesting, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
