---
title: "Interview with SafetyDetectives about the origin, future, and vision for Secutils.dev"
description: "Interview with SafetyDetectives about the origin, future, and vision for Secutils.dev"
slug: interview-safety-detectives
authors: azasypkin
image: /img/blog/2025-06-16_interview_safety_detectives.png
tags: [interview, thoughts, overview, technology]
---

<head>
    <link rel="canonical" href="https://www.safetydetectives.com/blog/aleh-zasypkin-secutils-dev/" />
</head>

:::info NOTE
This interview was initially published on the <a href="https://www.safetydetectives.com/blog/aleh-zasypkin-secutils-dev/" target="_blank" rel="nofollow noopener noreferrer">SafetyDetectives blog</a>. 
:::

Heya!

Recently I had a chance to talk to folks from <a href="https://www.safetydetectives.com" target="_blank" rel="nofollow noopener noreferrer">SafetyDetectives</a> about the origin, future, and vision for Secutils.dev. I'm also publishing a copy of the interview here for better reach. Read on!

<!--truncate-->

## Can you tell us about your background and what led you to create Secutils.dev?

I've been in Software Engineering for almost two decades now, working on everything from proprietary commercial software to free and open-source projects. For the last third of that time, I've been focused on application security - from directly designing and building security features (authentication, authorization, etc.) to promoting secure coding practices and helping maintain a strong security posture in a large engineering organization (200+ engineers).

Over the years, while dealing with security-related things both at work and outside of it, I accumulated a lot of code, tools, how-to articles, links to one-off online utilities, and shell snippets. Eventually, this grew into an unmanageable mess that made it hard to find and reuse things efficiently. That's how Secutils.dev started - as a way to organize all that into a unified, user-friendly interface with a curated set of the most useful utilities. It was originally meant for personal use, but it turned out to be helpful for other software engineers and security researchers dealing with similar day-to-day problems.

## What is Secutils.dev, and how is it different from other security toolkits or SaaS platforms?

In simple terms, [Secutils.dev](https://secutils.dev) aims to be a go-to place for engineers and security researchers to solve common ad-hoc security tasks. Need a disposable webhook to test an automation flow or set up a honeypot? Use the webhook utility. Need a self-signed certificate with edge-case parameters? There's a utility for that. Want to be notified when security-relevant parts of an open-source project change? Use the content tracking utility.

Think of Secutils.dev as a Notion for the needs of a security-minded professional. It's still early days, but that's the direction.

To use a kitchen analogy: if any given security toolkit is a chef's knife and B2B SaaS platforms are specialized kitchen appliances, then Secutils.dev is a Swiss-army knife. It's a personal tool you can carry from project to project, company to company - minimal hassle, no lock-in. Unlike deep domain-specific tools or enterprise-focused platforms, Secutils.dev is a jack-of-all-trades. It doesn't necessarily go deep into any one area, but helps unblock whatever you're working on right now.

It isn't better or worse than existing tools - it's different. It's focused on the individual professional's journey, not on an enterprise or specific domain.

## Many of Secutils.dev's features emphasize transparency and control. Why was a self-hostable model so important to you, and what challenges did it present?

From my experience, earning the trust of security-minded professionals is the hardest part. If you're a well-known brand, people might trust even a closed-source, fully managed, locked-in product. But if you’re not there yet, you need to earn that trust – and the way to do that is by being as transparent as possible, making the product and its roadmap open-source, and giving users control and choice via a self-hosted option.

Having access to source code that can be hosted independently and at no extra cost should send a clear signal: there's no intent to lock anyone in. Whatever users invest in Secutils.dev - time, experience, code - won't just vanish due to reasons outside of their control.

Of course, offering a self-hosted option (code, Docker images, docs, etc.) isn't the same as fully supporting it long-term. Many don't have the resources or incentive to maintain that. I do have an incentive - to build trust, so I self-host the managed version of Secutils.dev and use it myself, so I know it actually works.

Self-hosting isn't free - it costs time, expertise, and focus. However, I automate as much as possible (scaling, monitoring, upgrades) to make self-hosting less painful.

## Certificate monitoring and CSP validation can be difficult for smaller teams to implement correctly. How does Secutils.dev make these workflows easier or safer?

:::info NOTE
Certificate monitoring is planned and can be done with workarounds, but not natively today. I'll focus on CSP validation.
:::

Creating a reasonably secure Content Security Policy (CSP) is straightforward. The hard part is ensuring that CSP stays effective as the product evolves – not accidentally weakened by engineers or tooling, and still responding to modern threats.

Secutils.dev helps by parsing the raw CSP and presenting it in a user-friendly UI, organizing directives into categories and flagging potentially risky ones with links to explanations. When new directives are introduced, they'll be reflected in the tool. You can also store CSP versions for reference and use the webhook utility to collect CSP violation reports - without maintaining your own endpoint or server for that.

Based on recent feedback, CSP utility will soon gain auto-tracking - notifying users of any unexpected policy changes before they impact real users. That should make it even more useful.

## With the increasing complexity of modern web security, how do you decide which tools to include in the Secutils.dev suite - and how do you ensure they stay up to date?

These days, user feedback is the main driver. If I hear similar requests from different users, that's a strong sign of demand. When I say "users", I mean engineers and security researchers - people solving real problems in the field.

As I mentioned earlier, the initial utilities were built for my own use, and I still encounter new problems that feed into the roadmap. I'm also watching the industry closely - there's still a lot of stuff without user-friendly solutions, and that's a big source of ideas.

## What's next for Secutils.dev? Are there any upcoming features, community initiatives, or integrations you're particularly excited about?

Right now, Secutils.dev is getting a major upgrade in its tracking and notification capabilities. Its change tracking subsystem has evolved into a standalone open-source project - [retrack.dev](https://retrack.dev) - that now powers multiple projects, including Secutils.dev.

This opens up new possibilities for tracking not just web content, but also HTTP headers, cross-origin policies, permissions, CSP, file content, TLS certificates, and more. The goal is simple: anything security-related you could track manually, you should be able to delegate to Secutils.dev.

And yes, I can't avoid mentioning AI, right? Another big focus is integrating AI into new and existing utilities, using both hosted and offline/local LLMs. Many hosted LLMs aren't ideal for tricky security work due to their training safety boundaries, so giving users a choice to use custom, less constrained, local LLMs is important.

Imagine:
 - Instead of writing a script, you give a prompt like `Track the latest Node.js vulnerabilities at <insert web site address> and notify me weekly if there's a high-severity one`. 
 - Or `Monitor any changes to security-related headers and alert me on anything suspicious`. 
 - Or a honeypot that responds intelligently to incoming requests based on what it sees.

I see AI as a huge enabler for better user experience, and I plan to fully leverage that in Secutils.dev. If AI adoption continues, and I strongly believe it will, security-focused toolkits and SaaS platforms may eventually expose their own MCP servers that Secutils.dev can interact with - and vice versa, providing integration points essentially for free.

That's the kind of future I'm building toward.

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
:::
