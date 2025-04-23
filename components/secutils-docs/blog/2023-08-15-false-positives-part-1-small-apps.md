---
title: "The cost of false positives in software security, Part 1: Small applications"
description: "The cost of false positives in software security, Part 1: Small applications: Snyk, Dependabot, vulnerabilities."
slug: false-positives-part-1-small-apps
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-08-15_cost_of_false_positives.png
tags: [thoughts, overview, application-security]
---
Hello!

The other day, I was reading the [**"2023 State of Open Source Security"**](https://snyk.io/reports/open-source-security/) report by Snyk. It’s a nice report to read if you're curious about the state of the modern application security landscape, but here’s the part that particularly resonated with me:

> The constant rising tide of vulnerabilities continues to lead to security backlogs and decisions not to fix vulnerabilities. Part of the challenge here is false positives, which have increased alongside growing security processes and tooling automation. This is clear evidence that, while automation allows for much better coverage and detection, it can introduce data quality issues that are challenging for already stretched security teams to triage and accurately assess. In fact, false positives are reported at such a high volume that it is highly likely security teams are misclassifying some of these warnings. The sheer volume of CVEs that are ignored and left unfixed in applications (either by not applying patches or not versioning software) indicates that organizations are struggling to keep up with the demands of maintaining an airtight supply chain security posture. The widespread introduction of Al and automation injects additional uncertainty, making it harder to stay abreast, let alone get ahead, of supply chain security concerns.

False positives in security are something that really bothers me, as I happen to work on security for both large applications like [**Kibana**](https://github.com/elastic/kibana), with hundreds of contributors, and smaller ones like [**Secutils.dev**](https://secutils.dev), where I'm the sole developer.

<!--truncate-->

The cost of false positives varies significantly between large and small applications. Let's look into this by considering the applications I work on daily as examples. While there are many sources of vulnerabilities and, in turn, false positives, for simplicity's sake, I'll focus solely on vulnerabilities coming from application dependencies.

Now, when a tool like Snyk or Dependabot identifies a vulnerability within a dependency of any component in Secutils.dev, what's my approach? Do I carefully examine the proof-of-concept that could exploit the vulnerability? Do I immediately start assessing its applicability to Secutils.dev? Do I gauge the risk and then decide whether it's warranted to patch or upgrade?

Nope! That would be excessively time and energy-consuming. Usually, I take a more direct route: I swiftly upgrade the vulnerable or potentially exploitable dependency to include the fix. On the same day that the scanning tool detects the vulnerability, I release a new version of Secutils.dev SaaS.

Being the one who develops the application and chooses all the dependencies offers me a fairly accurate sense of **actual** severity. I often disregard the severity stated in the security advisory for the dependency itself. The true severity and risk are really context-dependent, based on how the vulnerable component or dependency is utilized in a specific application.

Naturally, there are exceptions to this approach. For instance, there could be cases where a vulnerability has been disclosed, but no fix is available yet. Alternatively, the vulnerable dependency might be a transitive dependency of another component, making an easy upgrade unfeasible. In these situations, going with an upgrade might not be a straightforward solution. Here, I start answering the following questions: Is the vulnerability indeed exploitable? If yes, how significant is the threat it poses?

In this context, I'm less concerned about issues like DDoS (Distributed Denial of Service) or ReDoS (Regular Expression Denial of Service) and the like. These are relatively manageable if they ever materialize as problems. However, when dealing with a severe vulnerability that could potentially impact Secutils.dev, I change my approach. I might decide to develop a patch, ideally within the Secutils.dev itself, or even in a forked version of a dependency if necessary.

The most important goal is to secure the application as quickly as possible. After that, I can take a closer look at the potential impact.

Irrespective of the approach I choose to address the problem, for severe issues that I can't readily dismiss as false positives and that could have led to data or secrets being compromised, like remote code execution (RCE), there are a few additional key steps I take:

:::caution Suggestion to fellow developers
Never attempt to conceal an exploitable vulnerability in your application if user data may have been compromised. Not only is this illegal in many jurisdictions, but it can also **irreparably** damage your reputation once exposed. Transparency is key, users and customers will value it. Trust me, it's highly likely that someone will eventually uncover it anyway.
:::

1. If there's even a small chance that the vulnerability could have been exploited to gain access to Secutils.dev or internal infrastructure secrets, I promptly rotate any potentially compromised secrets. This is precisely why it's essential to keep track of all your secrets and be prepared to rotate them quickly if necessary.

2. In cases where user data might have been compromised, the investigation becomes considerably more intricate. Even if there's the slightest possibility of compromise, I must inform users, reassure them that all requisite measures are being taken, and provide guidance on any necessary actions on their part. Such conversations are challenging, and it's critical to handle them with care to avoid missteps.

Having said that, I'm also careful not to cause unnecessary concern for my users if the issue turns out to be a false positive. This kind of panic can burden users with unnecessary tasks like password changes, secret rotations, and notifying their own users. Confirming a vulnerability's potential exploitability isn't enough, it's equally important to determine if any actual exploitation occurred. Extensive logging is invaluable here. When done right, logs can reveal past malicious activity. In my post on [**"Privacy-friendly usage analytics and monitoring"**](https://secutils.dev/docs/blog/usage-analytics-and-monitoring), I mentioned that I use the Elastic Stack for monitoring, log collection, and analysis — it's a big help for this task.

:::tip NOTE
Secutils.dev has only a handful of early users, collects minimal personally identifiable information (PII), and the likelihood of active exploitation is low due to limited incentives for experienced researchers or adversaries to invest in it. Given this, and since Secutils.dev is currently in a free beta, I retain logs for just the last 60 days to maintain a reasonable timeframe and avoid costly log storage. Always Be Pragmatic!
:::

Even if I'm entirely certain that user data wasn't compromised, I'll probably still send a "just for your information" post-mortem email to users. These emails aren't obligatory security notifications, so they don't pressure me. They offer a chance to engage with users, educate them, and strengthen mutual trust.

Alright, as you can see, depending on the severity, the true positives can end up being quite costly even for small applications like Secutils.dev. This shouldn't be too surprising since security problems, even in small apps, can result in significant damage. However, the cost of false positives is significantly lower:

* Smaller applications typically have less code and fewer use cases to analyze for confirming false positives. 
* These apps usually follow a single deployment model (like only SaaS), which can make it more practical and cost-effective to address potential vulnerabilities before or even instead confirming whether they are false positives. 
* Smaller applications often involve smaller teams or even individual developers, leading to simpler and more efficient security processes with less communication overhead.

Security issues in larger applications, on the other hand, are a whole different story, and I'll be diving into that in the second part of this post!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
