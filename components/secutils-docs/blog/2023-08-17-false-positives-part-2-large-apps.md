---
title: "The cost of false positives in software security, Part 2: Large applications"
description: "The cost of false positives in software security, Part 2: Large applications: Snyk, Dependabot, Kibana, Node.js, vulnerabilities."
slug: false-positives-part-2-large-apps
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-08-17_cost_of_false_positives_large_apps.png
tags: [thoughts, overview, application-security]
---
Hello!

This is the second part of my reflection sparked by the recent [**“2023 State of Open Source Security”**](https://go.snyk.io/state-of-open-source-security-report-2023-dwn-typ.html) report from Snyk. It got me thinking about the price we pay for false positives in software security. In my previous post, [**“The Cost of False Positives in Software Security, Part 1: Small Applications”**](https://secutils.dev/docs/blog/false-positives-part-1-small-apps), I talked about how true and false positives affect smaller applications like [**Secutils.dev**](https://secutils.dev). Now, I want to take the same idea and apply it to a much larger software that’s a big part of my daily work: [**Kibana**](https://github.com/elastic/kibana).

Saying that Kibana is one of the biggest Node.js apps you can find on GitHub would be no exaggeration. Just a quick glance at its **monthly** GitHub activity tells you all you need to know about its sheer size and scope!

![Kibana Monthly Stat](https://secutils.dev/docs/img/blog/2023-08-17_cost_of_false_positives_large_apps.png)

The code size, complexity, and the multitude of use cases it serves, combined with the numerous teams working on it, make Kibana an ideal case study for this post.

<!--truncate-->

Managing security findings in Kibana's dependencies shares many similarities with the process I described for Secutils.dev in my previous post. However, the size, complexity, and level of responsibility of Kibana bring quite a few significant differences in how we handle these findings.

To begin, Kibana has a dedicated Security team, which I'm currently part of! Unlike small applications, a majority of the code in Kibana isn't under our direct control. This means that determining whether a vulnerability in a dependency is a true positive or a false positive takes more time and resources. Sometimes, we must collaborate with various teams that depend on the same dependency to fully understand the potential impact. Clearly, we can't randomly disrupt other teams without reasonable preliminary confidence in the need to do so. Check out the [**“Industry Report: The True Costs of False Positives in Software Security“**](https://mergebase.com/blog/false-positives-software-security/#false-positives-can-damage-relationships-between-teams) to learn how false positives might even damage the relationships between teams!

Furthermore, Elastic, the company behind Kibana, provides Kibana not only through its managed Elastic Cloud solution but also as standalone artifacts and Docker images for users to deploy on their own premises. This implies that a universal dependency upgrade isn't feasible, and we must account for diverse deployment environments and their potential impacts. What might appear as a false positive for a dependency vulnerability in one environment could very well be a true positive in another.

Moreover, Elastic isn't unique in supporting multiple major versions of its applications concurrently. Given the rapid pace of development shown in the screenshot above, you can imagine the stark differences between Kibana's distinct major versions. One version might have been frozen a year or two ago, accepting only security patches, while another remains under active development. As you might have guessed, a dependency vulnerability's classification as a false positive or a true positive in one major version doesn't necessarily apply to another version. This requires separate assessments based on the context of each major version.

It might not be immediately obvious, but dismissing vulnerabilities in developer-only dependencies as false positives isn't as straightforward for big applications like Kibana. This is primarily due to the non-trivial CI infrastructure, testing procedures, and deployment flows associated with such large-scale application. Transparency isn't always guaranteed, and supply chain attacks, where vulnerable developer dependencies are targeted, are a genuine concern for large applications these days.

Large commercial applications introduce concerns that don't even cross your mind in the context of smaller applications. To illustrate, let's consider Node.js, which is basically the most important dependency for all Node.js applications, including Kibana. Yes, even Node.js can have vulnerabilities, and when it comes to products like Kibana, updating Node.js isn't as straightforward as it might appear. You can get more insights by checking out the [**“Upcoming Kibana releases to run Node.js 18”**](https://www.elastic.co/blog/kibana-releases-nodejs-18) blog post.

I've highlighted just a handful of the most noticeable differences in handling security vulnerabilities between small and large applications that I've observed over the past years. While false positives might incur minimal costs for small applications, they can substantially impact large applications. In certain scenarios, the expense of a false positive might be on par with that of a true positive, which is quite crazy when you think about it.

Unfortunately, we currently lack effective tools capable of accurately identifying vulnerabilities as false positives, a situation that wastes significant time and resources across the industry, possibly amounting to millions or billions of dollars. The field is ripe for innovation, no doubt, and with the emergence of AI-powered developer tools, I'm genuinely optimistic about the future!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
