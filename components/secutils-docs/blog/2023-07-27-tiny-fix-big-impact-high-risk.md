---
title: A tiny fix with big impact and high risk
description: "How a small 'recover original URL after sign-in' fix dramatically improves UX and how the same fix can become a phishing vector if you don't validate the redirect target. Open redirect (CWE-601), passkeys, and the OWASP cheat sheet for unvalidated redirects."
slug: tiny-fix-big-impact-high-risk
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-27_phishing.png
tags: [thoughts, overview, technology]
keywords: [open redirect, cwe-601, unvalidated redirects, post-login redirect, sso, passkey, phishing prevention, ory kratos return_to, secutils.dev, owasp cheat sheet]
---

Hello!

In [**my previous post**](/blog/alpha2-release) I covered the `1.0.0-alpha.2` release of [**Secutils.dev**](https://secutils.dev). The headline feature was the [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page), but I want to highlight a much smaller change from the same release: ["recover the original URL after sign-in"](https://github.com/secutils-dev/secutils/issues/9). It is a tiny fix with a huge UX payoff, and it's also a nice case study in how easy it is to introduce a subtle security bug while shipping it.

<!--truncate-->

:::info UPDATE (May 2026)
Since this post, identity in Secutils.dev has been delegated to [**Ory Kratos**](https://github.com/ory/kratos). The post-login redirect mechanism is now driven by Kratos's standard [**`return_to`**](https://www.ory.sh/docs/kratos/concepts/browser-redirects-flow-completion) parameter (Kratos enforces a configurable allow-list of return URLs) rather than the in-house `next` parameter shown below. The threat model and the validation principles are unchanged, the example URLs and Web UI source path (`secutils-dev/secutils-webui/.../url.ts`) are now in the [mono-repo](https://github.com/secutils-dev/secutils/tree/main/components/secutils-webui).
:::

## The friction this fixes

When you open a link to a page that requires authentication, you usually get sent to a login screen. The most annoying thing that can happen next is being landed on the app's home page after sign-in instead of where you actually wanted to go. It's especially common on apps with elaborate SSO flows, but plenty of "simple" auth setups have the same bug. For one-off links it's tolerable; for links you bookmark or use daily, it's painful.

There are two well-known fixes:

1. **Inline login** (modal or popover on the destination page).
2. **Pass the target URL through the login flow** (e.g. `?next=...` or `?return_to=...`), then redirect after success.

I prefer approach 2 for its simplicity and because it works the same regardless of where the user was redirected from. So Secutils.dev does this:

- Unauthenticated request to `https://secutils.dev/ws/web_scraping__resources`
- Redirect to `https://secutils.dev/signin?next=/ws/web_scraping__resources` (or the equivalent Kratos `return_to` URL today).
- After successful sign-in, redirect to the original URL.

So far, so straightforward. Where does it go wrong?

## The trap: open redirects

If the app redirects to whatever URL is in `next` without validating it, it becomes an open redirect, classic [**CWE-601**](https://cwe.mitre.org/data/definitions/601.html), and a great phishing primer.

Imagine a malicious user shares this link with you:

```
https://secutils.dev/signin?next=%2F%2Fws-secutils.dev%2Fws%2Fweb_scraping
```

The hostname looks legitimate. URL-decoded, the `next` parameter is `//ws-secutils.dev/ws/web_scraping`, a **protocol-relative** URL pointing at a **different** domain (`ws-secutils.dev`). After you sign in, the browser sails over to the lookalike site, which renders a perfect copy of the Secutils.dev sign-in page and asks you to "re-authenticate". You enter your real credentials, the lookalike sends them off, then redirects you to the real Secutils.dev. Most users won't notice anything was off.

Phishing is far more common than people realise. Recent statistics:

![Phishing campaign volume statistics over time](https://secutils.dev/docs/img/blog/2023-07-27_phishing.png)

:::tip NOTE
Phishing attacks like the one above [**don't work if you use a passkey**](https://support.apple.com/en-us/102195), because passkeys are bound to the legitimate origin and refuse to authenticate against a lookalike. Secutils.dev supports passkeys via Kratos. If you haven't switched yet, do.
:::

## How to do this safely

There is no clever trick: **always validate the redirect target before following it**, against an explicit allow-list. The simplest robust check uses the platform's URL parser to compare origins:

```ts
// Simplified validation logic
function safeNextUrl(next: string | null): string {
  if (!next) return '/';

  // Parse against the current origin so relative URLs resolve sensibly,
  // and protocol-relative `//evil.com/...` URLs reveal their true origin.
  const parsed = new URL(next, window.location.origin);

  // Reject anything that doesn't end up on our own origin.
  if (parsed.origin !== window.location.origin) return '/';

  return parsed.pathname + parsed.search + parsed.hash;
}
```

The Secutils.dev Web UI has done some variant of this from day one (see the [`url.ts` helper in the mono-repo](https://github.com/secutils-dev/secutils/tree/main/components/secutils-webui)). With Kratos in the mix today, the same check happens server-side: Kratos's `return_to` allow-list is configured to permit only Secutils.dev origins. **Defence in depth**: the client-side check protects against the most common bugs, but the server-side allow-list is what makes the system actually safe.

The [**OWASP "Unvalidated Redirects and Forwards" cheat sheet**](https://cheatsheetseries.owasp.org/cheatsheets/Unvalidated_Redirects_and_Forwards_Cheat_Sheet.html) covers more edge cases (HTTP-level redirects, server-side forwards, `<meta http-equiv="refresh">`, etc.); it's worth a few minutes if you're shipping anything with redirect parameters.

## Lesson

A "trivial" UX improvement (preserve the destination URL across login) is also a "trivial" path to a phishing vulnerability. Anything where the application redirects a user based on parameters under attacker control is in scope for open-redirect review. It's an easy security audit to add to your code-review checklist.

## Frequently asked questions

### Why pass the target through the URL instead of cookies?

Cookies break in a few common cases (cross-origin SSO, third-party cookie blocking, multiple browser tabs). The URL parameter is explicit, easy to inspect, and works in every flow, **provided you validate it**.

### Does Kratos validate `return_to` automatically?

Yes. Kratos enforces a configured allow-list of return URLs for browser flows; anything outside the allow-list is rejected. See [**Kratos browser redirects**](https://www.ory.sh/docs/kratos/concepts/browser-redirects-flow-completion).

### What about `<meta http-equiv="refresh">` redirects?

Same problem class. If the URL in the `content` attribute comes from user-controlled input, it must be validated. The OWASP cheat sheet covers this case explicitly.

### Are passkeys really safer than passwords against phishing?

Yes. Passkeys are scoped to an origin (the WebAuthn spec calls this the "Relying Party ID"), so a passkey for `secutils.dev` simply will not authenticate against `ws-secutils.dev`. The browser refuses to even offer the credential.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
