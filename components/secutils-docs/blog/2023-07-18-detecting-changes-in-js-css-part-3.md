---
title: Detecting changes in JavaScript and CSS isn't an easy task, Part 3
description: "Detecting changes in JavaScript and CSS isn't an easy task, Part 3: security and hardening."
slug: detecting-changes-in-js-css-part-3
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-18_web_page_resources_tracker.png
tags: [thoughts, overview, technology]
---
Hello!

This is the third and final part of a series of posts ([**part #1**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-1), [**part #2**](https://secutils.dev/docs/blog/detecting-changes-in-js-css-part-2)) where I explain why comparing JavaScript and CSS files isn't as simple as it may initially seem. Additionally, I'll share how I tackled this problem for the Resources Tracker utility in [**Secutils.dev**](https://secutils.dev/).

In the previous posts, I covered various challenges, including handling both inline and external resources, dealing with dynamically loaded and frequently changing resources, and comparing data and blob URLs. Today, I'd like to discuss the security-related challenges you should be mindful of if you're planning to build a similar tool like the Resources Tracker utility.

<!--truncate-->

## Challenge #6: HTML `onload` and `onerror` attributes

Here comes a tricky thing! To ensure that our JavaScript and CSS resources remain untampered, merely tracking the URL they are loaded from and their content isn't enough. We must also verify that the [**`onload` and `onerror`**](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/load_event) attributes of the `<script>` and `<link[rel=stylesheet]>` elements don't have unexpected changes. These attributes allow inlining any JavaScript code that will be executed when the resource loads or even when it fails to load:

```tsx
<script src="https://some-legit-url" onload="alert('😈')"></script>
```

To address this concern, we need to include the content of these attributes when generating the fingerprint of the resource we wish to track. In Secutils.dev, I simply concatenate the content of these attributes with the actual content of the resource before calculating the [**locality sensitive hash**](https://tlsh.org/). This way, we can be more confident about the integrity of the resources we are tracking.

## Challenge #7: Protected resources

:::caution NOTE
This functionality is not yet available in Secutils.dev. Please refer to [secutils#18](https://github.com/secutils-dev/secutils/issues/18)
:::

Throughout this series of posts, we've assumed that the web pages we need to track resources for are readily accessible to the tracking tool, whether it's a simple HTTP client or a full-fledged browser. However, this isn't always the case. Sometimes, the target page is only accessible to authenticated users, and the scraping tool we use must account for that.

Ideally, the web page would support authentication via the Authorization HTTP header with user credentials, for example, `Authorization: Basic base64(username:password)`. In most cases, though, the scraping tool will need to rely on a long-lived session cookie that is prepared beforehand. In either scenario, this additional authentication information is provided in the HTTP headers as `Authorization` or `Cookie`, respectively. This is one of the reasons why similar tools usually allow users to specify custom HTTP headers, and our tool should too!

In more complex cases, for instance, when the web page doesn't support long-lived sessions or authentication via HTTP headers, the tool might need to simulate the user login flow to create a new session every time it scrapes the resources. Fortunately, tools like Playwright, used in Secutils.dev, make this possible.

## Challenge #8: Malicious users

Another crucial aspect to consider when building a tool for a wide audience is the presence of malicious users. Unfortunately, there will **always** be users who attempt to cause harm, steal data, or abuse our tool. In our case, since users can instruct our tool to run a browser and navigate to any link, it opens up numerous opportunities for malicious intent if we don't take appropriate precautions.

To address this, my advice is to restrict the available functionality and API surface as much as possible and then gradually lift restrictions based on what significantly adds value to the tool.

Firstly, we should decide which URL schemes we want to support. The fewer, the safer. For example, Secutils.dev allows only `http` and `https` schemes, forbidding others like `file://` links that access the local filesystem or `chrome://` links that access browser internals, thus preventing potentially dangerous actions through lesser-known schemes.

Next, it's essential to limit the network resources available for scraping. At a minimum, we should protect our internal infrastructure by allowing access only to globally reachable addresses, excluding local host resources and internal network resources. For example, in the case of IPv4, we can exclude private IP ranges such as `10.0.0.0/8`, `172.16.0.0/12`, and `192.168.0.0/16`. The following is a reduced excerpt from the Secutils.dev IP validation code:
```rust
impl IpAddrExt for IpAddr {
    fn is_global(&self) -> bool {
        if self.is_unspecified() || self.is_loopback() {
            return false;
        }

        match self {
            IpAddr::V4(ip) => {
                 // "This network"
                !(ip.octets()[0] == 0 
                    || ip.is_private()
                    || ip.is_broadcast()
                    || ...
                 )
            }
            IpAddr::V6(ip) => {
                 // IPv4-mapped Address
                !(matches!(ip.segments(), [0, 0, 0, 0, 0, 0xffff, _, _]) 
                    // is_documentation
                    || (ip.segments()[0] == 0x2001) && (ip.segments()[1] == 0xdb8)
                    || ...
                )
            }
        }
    }
}
```

While it's impossible to achieve 100% security, as malicious users can be extremely creative and determined, we can still take additional precautionary measures to make their life harder. For example:

* Avoid exposing raw error messages to users. Instead, log the original message on the server and return something more generic to users. This minimizes the risk of leaking sensitive information in error messages. 
* Implement reasonable default timeouts for requests to extract resources to enhance resilience against Denial-of-Service (DoS) attacks. By setting timeouts, we ensure that requests are canceled and resources are released when necessary. 
* Introduce basic request rate and size limits. Web scraping can be resource-intensive, and web pages and resources can be quite large. By enforcing these limits, we prevent server resources from being easily exhausted.

These are just some of the initial and more apparent measures that come to mind, but there is always more that can be done to enhance security!

## Conclusion

In this series of posts, I have covered various challenges in building a tool to track changes in web page resources. Although this task is certainly doable, it's not without its complexities. If you're up for the challenge, give it a try! Alternatively, you can explore the Resources Tracker utility in [**Secutils.dev**](https://secutils.dev) for an existing SaaS solution.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
