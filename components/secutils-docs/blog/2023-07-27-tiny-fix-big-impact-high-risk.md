---
title: A tiny fix with big impact and high risk
description: "A tiny fix with big impact and high risk: authentication, SSO, open redirect, preventing unvalidated redirects."
slug: tiny-fix-big-impact-high-risk
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-27_phishing.png
tags: [thoughts, overview, technology]
---
Hello!

In [**my previous post**](https://secutils.dev/docs/blog/alpha2-release), I briefly touched on the latest `1.0.0-alpha.2` release of [**Secutils.dev**](https://secutils.dev). While the [**"Page tracker"**](https://secutils.dev/docs/guides/web_scraping/page) was the central part of this release, I want to highlight one tiny fix that makes a huge difference in user experience but can also be quite risky if not done right: [**“Recover original URL after sign-in”**](https://github.com/secutils-dev/secutils/issues/9).

<!--truncate-->

When you open a link to a web page that requires authentication, you are usually redirected to the login page. The most annoying thing that can happen is that after you enter your credentials and hit “Enter” you’re **NOT** redirected to your original destination! I see this issue more frequently on websites that require complex Single Sign-On (SSO) authentication flows, but it can also occur on sites with simple login flows. While it may be tolerable for one-time use links, it becomes a real pain if you need to use the link frequently or bookmark it.

Such behavior creates quite a bit of friction, distracting users from the action they wanted to perform initially. This drives users (myself included) crazy, and I don’t want to do that to Secutils.dev users.

There are different ways to approach this issue. The most common ones are to “remember” the original URL as a parameter in the login page URL to which the user is redirected, or to avoid redirect altogether by rendering a login modal/popup directly on the destination page. Both approaches have their pros and cons, but considering various corner cases with the login modal/popup approach, I generally prefer the separate login page for its simplicity and universality. As you might have guessed, I use this approach for Secutils.dev.

Now, let’s see how it works: If the user tries to access `https://secutils.dev/ws/web_scraping__resources` but isn’t authenticated, the app will redirect them to `https://secutils.dev/signin?next=/ws/web_scraping__resources`. This way, the login page can extract the original destination page from the `next` query string parameter and redirect the user to it after successful login. Is this that simple? Yes and no!

If we blindly redirect the user to whatever URL is embedded in the `next` query string parameter, we run the risk of becoming an easy phishing target. Imagine someone shares the following link with you: `https://secutils.dev/signin?next=%2F%2Fws-secutils.dev%2Fws%2Fweb_scraping`. The main domain looks legit, and it’s not easy to notice that the `next` parameter includes a URL to a completely different website - `https://ws-secutils.dev/ws/web_scraping`. Malicious actors can exploit various tricks and browser quirks to conceal the real destination in the `next` parameter. For example, here I used a URL-encoded protocol-relative URL (`//` instead of `https://`).

If Secutils.dev doesn’t validate the URL from the `next` parameter properly, after successful login, you'll be redirected to `https://ws-secutils.dev/ws/web_scraping`, and the chances that you'll check the URL bar again are very low. This is known as an [**“open redirect”**](https://cwe.mitre.org/data/definitions/601.html). The `https://ws-secutils.dev/ws/web_scraping` page can look exactly like the login page of the legitimate website and can prompt you to re-enter your Secutils.dev credentials, tricking you into providing your login details on the wrong website. After it steals your credentials, it can finally redirect you to `https://secutils.dev`, and you might not even notice that something was wrong.

:::tip NOTE
Such phishing attacks [**wouldn’t work if you use passkey**](https://support.apple.com/en-us/102195) as your credentials since it’s bound to the legitimate website/origin. If you don’t use passkey to sign in to Secutils.dev yet, I strongly encourage you to consider switching to it. It’s easy to do and convenient to use.
:::

Phishing is much more common than you might think. Take a look at the image with the recent statistics:

![Phishing statistics](https://secutils.dev/docs/img/blog/2023-07-27_phishing.png)

The example above shows that you absolutely have to validate all URLs you redirect users to if there is a chance they can be manipulated by third parties. In the Secutils.dev Web UI, specifically, I rely on the native `URL` class to check [**if the URL has the proper origin**](https://github.com/secutils-dev/secutils-webui/blob/643821fd0c2fc43475994fbdb7194ec4ef558bce/src/tools/url.ts) before redirecting the user. Also, check out [**"Preventing Unvalidated Redirects and Forwards"**](https://cheatsheetseries.owasp.org/cheatsheets/Unvalidated_Redirects_and_Forwards_Cheat_Sheet.html) from OWASP for more tips.

I hope I showed that seemingly unimportant and easy fixes can not only have a significant impact on the user experience but can also put your users at risk if you don’t consider the security implications of such changes. It's crucial to be mindful of potential risks and carefully validate any modifications that involve user redirection to ensure the safety of your users.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
