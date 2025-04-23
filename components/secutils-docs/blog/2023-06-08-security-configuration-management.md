---
title: Security configuration management for software engineers
description: "Security configuration management for software engineers: content security policy, infosec, red team, vulnerability scans, OWASP, security misconfiguration."
slug: security-configuration-management
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-06-08_csp_create.png
tags: [thoughts, guides, application-security]
---
In my previous posts, I have consistently emphasized the importance of improving efficiency and reducing waste, whether it's time, money, or energy. This concept has become a central theme in several projects I am currently involved in, including Secutils.dev. Today, I want to share my thoughts on security configuration management and how Secutils.dev and similar tools can enhance efficiency in this area by empowering software engineers, who are responsible for designing and implementing security configurations.

<!--truncate-->

Currently, security configuration management primarily falls under the domain of security experts and dedicated InfoSec teams. But is this approach truly efficient? I believe not, and I think it's an area where our industry can and must do better. While security teams focus on their specialized roles, software engineers actively engage in building, testing, and using the product on a daily basis. They possess the necessary context, technical expertise, and product domain knowledge required for effective security configuration management. By limiting their involvement, the feedback cycle for detecting and addressing security configuration issues becomes significantly delayed. This delay can result in increased costs for organizations and potential harm to their brand and customer trust.

It's no surprise that OWASP recently elevated [**"Security Misconfiguration"**](https://owasp.org/Top10/A05_2021-Security_Misconfiguration/) from the 6th to the 5th position in the [**OWASP Top Ten Web Application Security Risks**](https://owasp.org/www-project-top-ten/). The reason is simple: dealing with security configuration for any non-trivial product is very challenging.

To illustrate this point further, let's consider a specific example. Suppose you're working on a web application that accepts user input, and you want to minimize the risk of data injection attacks by implementing a restrictive [**content security policy (CSP)**](https://secutils.dev/docs/guides/web_security/csp). You create a policy that you believe is sufficiently restrictive, test it, and eventually deploy it. Maybe you verify everything is in order once more when it's in production, and then you continue with your engineering tasks.

Over time, browsers introduce new APIs while deprecating others. Advancing attack techniques constantly push web standards to evolve. Just take a look at the compatibility table for content-security-policy [on MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP#browser_compatibility) alone. Even security experts may struggle to keep up with the latest developments in this area:

![MDN CSP compatibility matrix](https://secutils.dev/docs/img/blog/2023-06-08_mdn_csp.png)

The policy you created a year ago may no longer be as restrictive as you intended it to be. There might be new and more appropriate directives available, or certain directives may have been deprecated. So, how do you determine when it's time to update your policy? The answer can vary.

In large organizations with substantial security budgets and mature security policies, there may be dedicated Red Teams constantly assessing the security posture of web applications against the latest standards and best practices. Alternatively, there may be InfoSec teams equipped with expensive enterprise-grade security configuration management software, running periodic security scans to identify any issues with the configuration. Even if these teams and tools are up-to-date and capable of detecting configuration issues, the process of triaging, assessing impact, and following up with the engineering teams responsible for the configuration can be time-consuming. It can work, but it's often an expensive, lengthy, and somewhat painful process.

Smaller organizations that cannot afford dedicated security teams may rely on contracting external vendors to conduct security scans once or twice a year. The results of these scans often yield extensive reports, consisting of tens or hundreds of pages of findings, many of which turn out to be false positives. These reports are then handed off to already overloaded developers, who must perform the initial triage, plan, and implement the necessary changes.

For the smallest organizations, startups, and indie projects, even this inefficient process is unaffordable, leaving their customers and users at the greatest risk.

The time that passes between the occurrence of a security configuration issue and its detection, triage, and remediation is often significant. The process involves multiple teams that pass the issue along until it finally reaches the engineering team, which can be resource-intensive. This notable inefficiency in security configuration management is something I have observed within the industry.

Wouldn't it be more efficient to involve engineering teams, who already possess the required context and knowledge, in security configuration management and monitoring from the very beginning? In other areas such as application performance, we already do this, and it has proven to work quite well. To achieve this, we need tools and processes that are simple, accessible, and approachable for any engineering team. Organizations can still benefit greatly from the existing security processes described earlier, but eliminating unnecessary "middle-men" in certain areas can overall strengthen the organization's security posture.

This is where the security configuration management capabilities of Secutils.dev can be valuable, through three essential steps: create, deploy, and monitor. Let's take a look at how the management of a [**content security policy (CSP) configuration**](https://secutils.dev/docs/guides/web_security/csp) can be handled in Secutils.dev.

## Create configuration

First and foremost, a robust security configuration management system should guide engineers through the process of creating the required configuration, ensuring they have all the necessary information to make informed choices.

For instance, when a user creates a content security policy configuration, the user interface might provide an explanation of each CSP directive and include links to the latest specification for those who want to delve deeper.

![Secutils.dev CSP editor](https://secutils.dev/docs/img/blog/2023-06-08_csp_create.png)

## Deploy configuration

Once the security configuration is created, the system should assist engineers in correctly deploying the configuration to the appropriate locations.

In the case of content security policies, the system can automatically serialize the created policy into a format that is compatible with either the `Content-Security-Policy` HTTP header or the HTML `meta` tag, and explain how to report policy violations.

![Secutils.dev CSP serialization](https://secutils.dev/docs/img/blog/2023-06-08_csp_deploy.png)

## Monitor configuration

Last but certainly not least, a security configuration management system should assist engineers in ensuring that the configuration *continues* to function as intended throughout its entire lifespan.

When it comes to the content security policy configuration, the system can notify engineers about common issues such as misconfigured CSP nonces, upcoming deprecations of directives, or unexpected policy changes that may occur during deployment.

:::caution NOTE

This functionality is not yet available in Secutils.dev. Please refer to [**#secutils/15**](https://github.com/secutils-dev/secutils/issues/15) for more information.

:::

<div class="text--center">
    <img src="https://secutils.dev/docs/img/blog/2023-06-08_csp_monitor.png" alt="Secutils.dev CSP monitoring" />
</div>

## Conclusion

The content security policy configuration used as an example here is just one of many security configurations that modern products need to consider. Alongside content security policies, there are [**same-origin policies**](https://developer.mozilla.org/en-US/docs/Web/Security/Same-origin_policy), [**subresource integrity**](https://developer.mozilla.org/en-US/docs/Web/Security/Subresource_Integrity) rules, [**web permissions policies**](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Permissions-Policy), and more. These sophisticated configurations and policies exist for a reason: the cybersecurity landscape is constantly evolving. Threat actors target organizations of all sizes, and their strategies and tools adapt and evolve every single day.

That's why I think it's super important to democratize security configuration management even more and give engineers accessible tools to stay ahead in this ever-changing environment.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
