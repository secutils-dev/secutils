---
title: "Running web scraping service securely"
description: "Running web scraping service securely - Playwright scraper, Node.js scraper, Docker containers, Kubernetes network policies for scraper, seccomp, Chromium sandbox."
slug: running-web-scraping-service-securely
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-09-12_running_web_scraping_service_securely.png
tags: [overview, technology, application-security]
---
Hello!

[**In my previous post**](https://secutils.dev/docs/blog/q3-2023-update-notifications), I shared the update regarding the upcoming "Q3 2023 - Jul-Sep" milestone. While I briefly covered how I implemented the notifications subsystem in [**Secutils.dev**](https://secutils.dev), there are a few other important changes I've been working on for this milestone. One of these changes is related to the fact that I’m preparing to allow Secutils.dev users to inject custom JavaScript scripts into the web pages they track resources for (yay 🎉). As a result, I've spent some time hardening the Web Scraper environment's security and wanted to share what you should keep in mind if you’re building a service that needs to scrape arbitrary web pages.

<!--truncate-->

:::note __UPDATE (Jan 16th, 2024)__
I've published a dedicated [**"How to track anything on the internet or use Playwright for fun and profit"**](./2024-01-16-web-page-content-trackers-and-playwright.md) with a more in-depth look into the scraping process itself. Check it out!
:::

When it comes to web page resource scraping, Secutils.dev relies on a separate component - [**secutils-dev/retrack**](https://github.com/secutils-dev/retrack). I've built it on top of [**Playwright**](https://playwright.dev/) since I need to handle both resources that are statically defined in the HTML and those that are loaded dynamically. Leveraging Playwright, backed by a real browser, instead of parsing the static HTML opens up a ton of opportunities to turn a simple web resource scraper into a much more intelligent tool capable of handling all sorts of use cases: recording and replaying HARs, imitating user activity, and more.

As you might have guessed, running a full-blown browser within your infrastructure that users can point literally anywhere can be quite dangerous if not done right, so security should be a top-of-mind concern here. Let me walk you through the most obvious security concerns one should address before exposing a service like that to the users.

## Input validation

The first line of defense, and the most basic one, is to limit where users can point their browsers through input argument validation. If you know that only certain resources are supposed to be scraped by users, ensure you properly validate the provided URLs and allow only the expected subset. As a bare minimum, you should validate the arguments on the server/API side, but also consider doing it on the client side if possible, as it would significantly improve the user experience of your service and serve your users better. However, **never-ever** rely solely on client-side validation - client-side validation is for your users' convenience and is not a security measure, as it can be easily bypassed by directly accessing your APIs.

Although the resource tracker functionality of Secutils.dev is designed to allow users to scrape virtually any web page on the internet, I still make efforts to validate the provided URL and restrict it as much as possible:
```rust
if tracker.url.scheme() != "http" && tracker.url.scheme() != "https" {
    anyhow::bail!("Tracker URL scheme must be either http or https");
}

// Checks if the specific hostname is a domain and public (not pointing to the local network).
let is_public_host_name = if let Some(domain) = tracker.url.domain() {
    ...
} else {
    false
};

if !is_public_host_name {
    anyhow::bail!("Tracker URL must have a valid public reachable domain name");
}
```

## Resource isolation

Running an entire browser is a resource-intensive operation, even if it’s [**a headless one**](https://en.wikipedia.org/wiki/Headless_browser). It’s likely that the component responsible for running and dealing with the browser isn’t the only part of your service. It's probably not as critical as components dealing with authentication or database access, for example. You certainly don’t want your entire service to go down just because a resource-intensive web page consumed all the available resources on the host.

To address this, consider running the component that spawns the browser within a separate container. This approach not only better protects your business-critical functionality but also allows you to scale up or down your browser-specific service independently.

Additionally, try to explicitly limit the resources available to the container using techniques like [**control groups**](https://en.wikipedia.org/wiki/Cgroups) or similar features that suit your environment. For instance, if you’re running your container in Kubernetes, you can [**limit resources**](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/) available to that container using configurations such as this:
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: web-scraper
spec:
  containers:
  - name: app
    image: node:20-alpine3.18
    resources:
      requests:
        memory: "128Mi"
        cpu: "250m"
      limits:
        memory: "1Gi"
        cpu: "500m"
```

## Privilege management

[**The principle of least privilege**](https://en.wikipedia.org/wiki/Principle_of_least_privilege) is particularly crucial when dealing with complex software like a web browser. Running a browser as the root user is inviting trouble, and it's something you should avoid. For instance, if you're using Node.js to automate a headless browser with tools like Puppeteer or Playwright, make sure to run it as a [**non-root user**](https://github.com/nodejs/docker-node/blob/main/docs/BestPractices.md#non-root-user):
```bash
FROM node:20-alpine3.18
...
USER node
CMD [ "node", "src/index.js" ]
```

If you're running your container in Kubernetes and relying on a non-root user, you can safely [**drop all capabilities**](https://kubernetes.io/docs/tasks/configure-pod-container/security-context/#set-capabilities-for-a-container) for that container:
```yaml
securityContext:
  capabilities:
    drop: [ ALL ]
```

You can take additional steps by setting the [**appropriate seccomp profile**](https://kubernetes.io/docs/tasks/configure-pod-container/security-context/#set-the-seccomp-profile-for-a-container) for the Node.js container:
```yaml
securityContext:
  seccompProfile:
    type: Localhost
    ## Taken from https://github.com/microsoft/playwright/tree/main/utils/docker
    localhostProfile: secutils-web-scraper-seccomp-profile.json
```

These measures ensure that your browser runs with the least privileges necessary, reducing potential security risks.

## Browser sandbox

If you've followed the recommendation from the previous section and are running your browser process as a non-root user, there's no reason not to enable a [**sandbox for your browser**](https://chromium.googlesource.com/chromium/src/+/lkgr/docs/linux/sandboxing.md#linux-sandboxing). For example, if you're using Playwright with Chromium, you can enable the sandbox like this:

```javascript
import { chromium } from 'playwright';

const browserToRun = await chromium.launch({
  chromiumSandbox: true,
});
```

Enabling the sandbox adds an extra layer of security to your browser operations, visit **[no-sandbox.io](https://no-sandbox.io/)** to learn about the potential risks of disabling the Chromium/Chrome sandbox.

## Network policies

Even if your input validation code appears reliable today and you have a solid test coverage, bugs can occur at any time. If you have the opportunity to implement multiple layers of defense, make use of as many layers as your financial and resource constraints allow. Implementing proper network policies for the container running the browser based on user-provided URLs is one of such layers. At the very least, you should safeguard your internal infrastructure by allowing access only to globally reachable addresses while excluding local host resources and internal network resources. For example, in the case of IPv4, you can exclude private IP ranges like `10.0.0.0/8`, `172.16.0.0/12`, and `192.168.0.0/16`.

In Kubernetes, you can achieve this using [**`NetworkPolicy`**](https://kubernetes.io/docs/concepts/services-networking/network-policies/). Here's an example of how to set up a simple policy to forbid access to non-global IP addresses:

```yaml
kind: NetworkPolicy
apiVersion: networking.k8s.io/v1
metadata:
  name: secutils-web-scraper-network-policy
  namespace: secutils
spec:
  policyTypes: [ Egress ]
  podSelector:
    matchLabels:
      app: secutils-web-scraper
  egress:
  - to:
    - ipBlock:
        # Allow all IPs.
        cidr: 0.0.0.0/0
        except:
          # Except for the private IP ranges.
          - 10.0.0.0/8
          - 172.16.0.0/20
          - 192.168.0.0/16
```

## Monitoring

So, you've implemented robust input validation, ensured container security, isolated resource-heavy workloads, and restricted network access with network policies. Is that enough for peace of mind? Well, it might be, but then again, it might not. Threat actors and their tactics evolve daily, and what appears secure today might not be tomorrow. In our imperfect world, bugs, misconfigurations, and other errors happen regularly. Stay vigilant, keep an eye out, and maintain constant monitoring of your deployments.

Fortunately, there are [**numerous tools**](https://secutils.dev/docs/blog/usage-analytics-and-monitoring#monitoring) available for monitoring and alerting, ranging from free to paid, simple to sophisticated, self-hosted to fully managed. There's no excuse not to utilize them. Monitor resource usage and set alerts for unexpected spikes, watch for brute-force and DDoS attempts, and pay attention to unexpected errors and service crashes. If your service or product is publicly accessible, I guarantee, monitoring data will reveal a lot of unexpected stuff about what's happening while you're catching some sleep 🙂

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
