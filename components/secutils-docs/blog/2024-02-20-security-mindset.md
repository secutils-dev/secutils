---
title: "Cybersecurity basics: security mindset"
description: "Cybersecurity basics: security mindset: unhappy path, security through obscurity, assume compromise, pragmatic security."
slug: security-mindset
authors: azasypkin
image: /img/blog/2024-02-20_security_mindset.png
tags: [thoughts, overview, technology]
---
Hello!

Recently, I was invited to give a presentation on cybersecurity to a group of young developers at [**Onja**](https://onja.org/), a social enterprise in Madagascar. Since they are at the beginning of their cybersecurity journey, I didn't want to bore them with the hackneyed OWASP Top 10 or overwhelm them with the plethora of security tools developers have to rely on these days to keep software safe and secure. Instead, I wanted to discuss something basic yet foundational for anyone dealing with cybersecurity - the security mindset.

In my experience, when it comes to security, the right mindset is what transforms an average engineer into a good one. It's not something you can buy or acquire quickly, but it's something everyone can learn over time and benefit from throughout their career. The earlier you realize this, the better. Similar to building personal wealth, the earlier you start learning and investing, the better your life will be.

Generally speaking, if you're dealing with anything related to security, the right mindset gets you roughly 80% of the job done; the remaining 20% comes from proper tooling, a good team, and other factors. I like to say that developing a security mindset is simple, but not easy.

This blog post is the presentation turned into a blog post. Read on!

<!--truncate-->

## Explore unhappy path

![Explore unhappy path](/img/blog/2024-02-20_security_mindset_explore_unhappy_path.png)

As software engineers, you've likely heard about the term "happy path" - the ideal scenario in the application flow. If the user follows the path we expect, and everything else in the application works as planned, everyone is happy. Hence, the term happy path. For a software engineer, it's entirely reasonable to think about the happy path first: it helps in understanding what needs to be built. However, when your goal is to assess whether the application is secure enough and understand how malicious actors or hackers could potentially break it, you should take a much closer look at how the application behaves when things go wrong – the so-called unhappy path.

This approach is somewhat similar to what QA engineers do, but as a software engineer, you go much deeper. Start by examining what happens when a legitimate user does something unexpected, like pressing buttons in the wrong order, repeatedly and quickly refreshing the page, bombarding your server with requests, or uploading a file of an unexpected type or much larger size than expected. Move on to more advanced scenarios, such as what if the external service or database your application communicates with is unavailable or hacked, sending malicious data? Or if the file you're reading from disk is corrupted, or the user content you render in the application contains malicious code?

Building a habit of thinking about the unhappy path first might not be easy, but ignoring these scenarios often leads to weaknesses in your application that others could exploit to harm your users and your reputation.

## Assume compromise

![Assume compromise](/img/blog/2024-02-20_security_mindset_assume_compromise.png)

More frequently than not, you hear things like:

- "Oh, our network is behind a firewall and fully protected within a corporate network, we don’t need to worry about establishing trust between internal services and applications". But then, a testing application is deployed and exposed to the internet by mistake, becoming an open door for intruders to easily access everything within a corporate perimeter.
- Or, "we use Okta to manage our users and their permissions, it takes care of everything, we don’t need to monitor for suspicious activity of employees' accounts." Then suddenly, Okta's customer support system is hacked, and malicious actors can now act on behalf of your own employees.
- Or, "we don’t need to waste time on automated credentials revocation when an employee leaves the company; the admin will do it manually later this week". But then, suddenly, an angry fired employee wipes out your customer data backups and wreaks havoc in the internal infrastructure.
- Or even, "let’s introduce this new file upload feature without any additional configuration switch that could allow us to easily reduce file size limit or disable the feature completely without rebuilding and re-deploying the application entirely". And then, the application is being DDoS’ed with gigantic files, putting your entire infrastructure down or skyrocketing your Cloud storage bills.

If something bad can happen in theory, someday it will happen in practice, and you'd better be prepared for that right from the start. Assuming that all your security measures are compromised will help you build much more resilient and future-proof applications that will save your time, money, and reputation.

## Avoid insecurity through obscurity

![Avoid insecurity through obscurity](/img/blog/2024-02-20_security_mindset_avoid_insecurity_through_obscurity.png)

You might have heard the term "Security through obscurity" - in simple terms, it's the reliance on secrecy as the primary method of providing security to a system or component. There's even an illusion that closed-source software is more secure than open-source because attackers cannot understand the application logic and spot flaws in the code. Some even claim that obfuscating code is a strong security measure to protect intellectual property.

Statements like that aren’t just untrue, but actually very dangerous - if a company believes that secrecy is a sufficient security measure, they won't invest time and money in educating their employees about security and won’t bother thinking about mitigation and response tactics if they come under attack. Lying to yourself is the worst type of lie.

Attackers are a smart and creative bunch of people. If they have enough motivation, they will figure out everything about your application and infrastructure they need to conduct an attack. There are thousands of tools easily accessible to them that can analyze every bit and piece of your application. Not to mention the emergence of AI that can sometimes collect the puzzle in minutes.

So, if you're serious about security, it's better to assume that your secrets aren't secrets to a sufficiently motivated party. It’s important to keep your secrets safe, though it shouldn’t be the only or certainly not the main security measure.

## Be pragmatic about security

![Be pragmatic about security](/img/blog/2024-02-20_security_mindset_be_pragmatic_about_security.png)

There is no doubt that keeping an application secure is important for both developers and users. However, keep in mind that the main goal of the software is to deliver value to the user. Nobody would use or pay for an application just because it's super secure. Look at security as something fundamental, always going without saying, like the absence of bugs, reasonable performance, and usability. There are practical limits to how secure an application can be while still delivering value to the user. When working with other engineers and helping them make their software secure, be collaborative and pragmatic. You have a common goal: to deliver top-notch software to your users that is both secure, useful, and user-friendly.

If you have to make trade-offs in security to improve usability or deliver more value, talk through the risks with the team, document everything, prepare a plan in case anything goes wrong, and move on.

## Always be learning

![Always be learning](/img/blog/2024-02-20_security_mindset_always_be_learning.png)

The security landscape is constantly changing, in fact, it's one of the most dynamic areas of information technology today. Almost every day, we hear about new vulnerabilities, novel approaches to break applications, or trick users. I won't stop repeating that your knowledge is your most important tool, but it becomes dull very quickly if you don't sharpen it regularly enough. Make learning a part of your routine, read books, follow security researchers on their social networks - they do like to share their knowledge, dive deep into security disclosures and statements, grind through exploit proof-of-concepts until you fully understand how it works.

Always be curious, always be learning, it will pay off. We are all different, find the way to learn that is suitable for you, something that brings you joy and is fun to do, then it will be much easier.

## Example: logs and exceptions

![Example: logs and exceptions](/img/blog/2024-02-20_security_mindset_logs_and_exceptions.png)

During their lifetime logs can flow through different environments with different level of security and access, and it’s hard to be 100% sure where your logs will be at any point in the future. It’s not uncommon to ingest your logs into different external cloud-based solutions (e.g. Elasticsearch or Datadog) for monitoring and further analysis. Older logs might be stored for a long term in a complete (e.g. S3).

When you’re dealing with logs, these are the important questions you should ask yourself:

- Can logs contain secrets or customer information?
    - Sensitive information isn’t only passwords or secret tokens, but also so-called PII (Personal Identifiable Information) - anything that can be used to identify a specific person - e.g., full name, username, address, phone number, and any other official document number. Even if you rotate leaked passwords in your application, they might still be valid in other places. Unfortunately, people like to reuse passwords.
- If I log exceptions, do I fully trust all the fields that will be logged?
    - Usually, developers pay attention to the data and responses they send to the user, but throwing exceptions is another important program flow that leads to returning some data to the user that might be sensitive too, but it’s often ignored. Exceptions can include file paths, environment variables, request and response headers that might include credentials and cookies.
- Who can access logs during their entire lifetime?
    - You should think about where your logs go after they are logged, how they are stored, and who might potentially have access to them. The safest assumption is that logs might eventually be exposed to the public in one way or another, so it’s better to be careful with logging anything that can be potentially sensitive.
- Can logs contain user content that can be weaponized?
    - There is an entire class of vulnerabilities that can be exploited through logs, so-called log injections. It can be anything, from making your terminal unusable to remote code execution where the attacker can execute binaries in your environment. It can be quite scary. I'd encourage you to read more about [**Log4Shell**](https://en.wikipedia.org/wiki/Log4Shell) and [**log injections**](https://owasp.org/www-community/attacks/Log_Injection).

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
:::
