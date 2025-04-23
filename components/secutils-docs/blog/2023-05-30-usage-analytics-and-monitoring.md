---
title: Privacy-friendly usage analytics and monitoring
description: "Privacy-friendly usage analytics and monitoring for the Secutils.dev: Elasticsearch, Kibana, Beats, Plausible."
slug: usage-analytics-and-monitoring
authors: azasypkin
image: https://secutils.dev/docs/img/blog/elastic.png
tags: [overview, technology]
---

Hello!

In my previous posts, I covered the [**technological stack**](/blog/2023-05-25-technology-stack-overview.md) behind [**Secutils.dev**](https://secutils.dev) and how I [**deploy its components**](/blog/2023-05-28-deployment-overview.md) to the Kubernetes cluster.


Today, I want to showcase the tools I use to monitor my Secutils.dev deployment and collect usage analytics.

<!--truncate-->

## Usage analytics

Knowing your users, where they come from, and how they use your product is not just important but absolutely critical, especially in the early days.

When it comes to web analytics, Google Analytics is the obvious choice, but let's face it, it has a bad reputation for user privacy. Building and maintaining user trust is a top priority for me, so after digging around a bit, I stumbled upon this cool tool called [**Plausible Analytics**](https://github.com/plausible/analytics). It's simple, lightweight (less than 1 KB), open-source, and privacy-friendly. Plus, it claims to be fully compliant with GDPR, CCPA, and PECR. How awesome is that?

So, for monitoring and analyzing Secutils.dev, I decided to give Plausible Analytics a shot. It's all about striking the right balance between understanding user behavior and respecting their privacy.

Normally, I'd lean towards opting for their paid fully-managed offering to lighten the maintenance burden and support the Plausible Team. However, since Secutils.dev is still in the pre-revenue stage, every euro counts. Fortunately, the Plausible Team has us covered with a [**free self-hosting option**](https://plausible.io/docs/self-hosting). They even provide Kubernetes configuration files to make the deployment process a breeze. That's exactly what I've been searching for!

The Plausible Analytics app comprises three key components: a main PostgreSQL database, a ClickHouse database, and the Plausible web server itself. While I won't delve into the details of deploying these components to a Kubernetes cluster in this post (it's a bit beyond the scope), rest assured that it's incredibly straightforward with the Kubernetes configuration files they've shared.

One additional trick worth mentioning is renaming the Plausible script to prevent it from being inadvertently [**blocked by ad-blockers**](https://plausible.io/docs/proxy/introduction). I understand that it's a subject of debate, but I firmly believe that privacy-friendly analytics tools like Plausible pose no harm to users. In fact, they can significantly contribute to enhancing the services users receive. To handle ad-blockers, I've set up a dedicated Traefik Ingress rule specifically for Plausible:

```yaml
apiVersion: traefik.containo.us/v1alpha1
kind: IngressRoute
...
spec:
  ...
  routes:
    - kind: Rule
      match: Host(`secutils.dev`) && (Path(`/js/script.js`)
      services:
        - kind: Service
          name: plausible
          port: 8000
...
```

Here's what the Plausible dashboard for Secutils.dev looks like:

![Plausible Dashboard](https://secutils.dev/docs/img/blog/plausible.png)

## Monitoring

Picking the monitoring tool was a no-brainer for me since I currently work for Elastic and have extensive experience using both [**Elasticsearch and Kibana**](https://www.elastic.co). It's like having insider knowledge!

Similar to Plausible, Elastic also offers Kubernetes configuration for all the required components through [**Elastic Cloud on Kubernetes (ECK)**](https://www.elastic.co/guide/en/cloud-on-k8s/current/k8s-quickstart.html). This means I can self-host the entire monitoring infrastructure for free, which is great!

For my monitoring setup, I deploy the following Elastic Stack components:

- [**Elasticsearch**](https://www.elastic.co/guide/en/cloud-on-k8s/current/k8s-elasticsearch-specification.html) - it serves as the core search engine and database for storing the monitoring data.
- [**Filebeat**](https://www.elastic.co/beats/filebeat) - this component collects logs from various Kubernetes pods and ingests them into Elasticsearch, allowing for centralized log analysis and searching.
- [**Metricbeat**](https://www.elastic.co/beats/metricbeat) - this component is responsible for collecting host metrics, such as CPU usage, memory usage, and network statistics. It then sends these metrics to Elasticsearch, enabling monitoring and analysis of system-level performance.
- [**Kibana**](https://www.elastic.co/guide/en/cloud-on-k8s/current/k8s-kibana.html) - it is the visualization and exploration interface provided by Elastic. It allows me to interact with the logs and metrics stored in Elasticsearch, providing the ability to drill down into specific details.

![Kibana Dashboard](https://secutils.dev/docs/img/blog/elastic.png)

This monitoring setup provides me with the ability to not only identify when my servers are experiencing issues but also catch errors in production that might have been missed during testing. By leveraging the monitoring capabilities, I can proactively address these issues before users report them.

Although I'm currently utilizing only the basic functionalities of the Elastic Stack, I have plans to expand its usage over time. Specifically, I intend to employ Elastic APM (Application Performance Monitoring) to monitor the performance of Secutils.dev components. Additionally, I'm interested in utilizing Elastic Machine Learning to detect anomalies in user behavior and identify unusual patterns or suspicious activities, enhancing the security of Secutils.dev.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
