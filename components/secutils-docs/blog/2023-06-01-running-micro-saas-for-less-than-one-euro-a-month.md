---
title: Running micro-SaaS for less than 1€ a month
description: "Running micro-SaaS almost for free: GitHub, OCI, Elastic, Plausible, Let's Encrypt, Zoho."
slug: running-micro-saas-for-less-than-one-euro-a-month
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [overview, technology, economics]
---
Hello!

In my previous posts, I mostly focused on the technical aspects related to [**Secutils.dev**](https://secutils.dev), such as the [**technological stack**](/blog/2023-05-25-technology-stack-overview.md), [**deployment process**](/blog/2023-05-28-deployment-overview.md), and [**tools for monitoring and usage analytics**](/blog/2023-05-30-usage-analytics-and-monitoring.md).

Today, I'd like to discuss the costs associated with running [**Secutils.dev**](https://secutils.dev) in production. As developers, we understand the importance of being resourceful, frugal, and efficient when building and maintaining products. Therefore, minimizing costs is a crucial and ongoing topic. Let's dive into how I optimize costs for Secutils.dev.

<!--truncate-->

---

**DISCLAIMER:** I want to acknowledge that while the strategies I'm about to share work well in the early stages of a product and micro-SaaS, they may not be as effective as the product grows. However, at this point, we can safely set aside concerns about scaling and growth issues since many companies and products fail long before reaching that stage.

---

## Source code management

**Cost:** 0€ / month

**Vendor:** [**GitHub**](https://github.com/pricing)

The source code for [**Secutils.dev**](https://secutils.dev) is publicly available and is stored in several separate repositories hosted on GitHub, which offers free hosting. Additionally, GitHub provides unlimited free private repositories, which is ideal for managing content that is not relevant to the broader community. For instance, I use a private repository to host the source code for the Secutils.dev promotional website, as well as the terms and privacy policy.

GitHub's free plan also includes features like secret scanning and dependabot, that I'm pleased to make use of.

## Continuous integration

**Cost:** 0€ / month

**Vendor:** [**GitHub**](https://github.com/pricing)

For my current CI needs, I require basic checks such as ensuring that the backend server, Web UI client, and documentation can be successfully built after every push to the upstream branch. I also run tests and perform a few essential linting checks.

GitHub's free plan offers a generous allocation of 2000 minutes per month for [**GitHub Actions**](https://github.com/features/actions), which is more than sufficient for a small project like mine. Since I don't push every commit to GitHub, the CI process is not triggered very frequently. Typically, I push a batch of commits once or twice a day. As of now, the average CI run time for the Web UI client and documentation website is around 3 minutes each. The backend server's full build time varies between 3 to 18 minutes, depending on the changes made. However, I already have plans to optimize and reduce this build time by half. Since I don't make changes across all three repositories every day, a maximum estimate of 25 minutes of CI time per day seems reasonable. At this rate, I would exceed the 2000 minutes monthly budget only after 80 days, rather than the standard 30 days.

To minimize build times, I make extensive use of Cargo and `npm` caches between CI runs. You can find the GitHub Actions configurations for the [**backend server**](https://github.com/secutils-dev/secutils/blob/main/.github/workflows/ci.yml), [**Web UI client**](https://github.com/secutils-dev/secutils-webui/blob/main/.github/workflows/ci.yml), and [**documentation**](https://github.com/secutils-dev/secutils-docs/blob/main/.github/workflows/ci.yml) in the corresponding repositories.

## Hosting

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/)

As I discussed in my previous post, [**Technology stack overview**](/blog/2023-05-25-technology-stack-overview.md), Secutils.dev comprises various components built on different technologies, each with its own resource requirements. When it came to choosing a hosting solution, I found Kubernetes to be the most suitable tool for the job.

However, finding a cloud provider that offers ready-to-use Kubernetes infrastructure in a free tier proved challenging. Therefore, I prepared myself to set up my own Kubernetes cluster from scratch. During my research, I came across the [**Oracle Cloud Free Tier**](https://www.oracle.com/cloud/free/#always-free). While their free tier does not include Container Engine for Kubernetes (OKE), it still offers an impressive range of features. Here's the part of their offer that is particularly relevant to me:

> **Arm-based Ampere A1 cores and 24 GB of memory usable as 1 VM or up to 4 VMs with 3000 OCPU hours and 18000 GB hours per month.**
>

With 3000 OCPU hours per month, which is equivalent to 4 OCPUs, I have the flexibility to manually create a small cluster with up to 4 nodes. At this stage, it is acceptable to allocate one of the worker nodes for the control plane. For example, I can assign 2 OCPUs and 12 GB RAM to the `secutils-prod` node, 1 OCPU and 8 GB RAM to the `secutils-dev` node, and 1 OCPU and 4 GB RAM to the `secutils-qa` node.

The free tier also offers additional benefits, such as unlimited inbound data transfer and a monthly limit of 10 TB for outbound traffic, free of charge. I encourage you to explore the details if you're interested.

Please note that while the Oracle Cloud Free Tier provides generous resources, it is important to monitor your usage to ensure you stay within the free tier limits. Don’t forget to set up budget alerts as a precautionary measure.

## Monitoring

**Cost:** 0€ / month

**Vendor:** [**Elastic (self-hosted)**](https://www.elastic.co)

As discussed in my previous post, [**Privacy-friendly usage analytics and monitoring**](/blog/2023-05-30-usage-analytics-and-monitoring.md), I utilize Elasticsearch, Kibana, and Beats for monitoring purposes. Since I have my own Kubernetes cluster, I can self-host and use these tools for free, within the limits of the [**Elastic Basic license**](https://www.elastic.co/subscriptions).

To ensure that logs and metrics data do not accumulate indefinitely and consume all available space in the free tier, I have set up an [**index lifecycle policy**](https://www.elastic.co/guide/en/elasticsearch/reference/master/getting-started-index-lifecycle-management.html). This policy allows me to automatically delete old data when the index size exceeds a predefined threshold.

## Analytics

**Cost:** 0€ / month

**Vendor:** [**Plausible (self-hosted)**](https://plausible.io)

As mentioned in [**Privacy-friendly usage analytics and monitoring**](/blog/2023-05-30-usage-analytics-and-monitoring.md), I utilize Plausible Analytics for gathering usage analytics. Similar to my monitoring setup, I self-host Plausible Analytics within my Kubernetes cluster, resulting in no additional cost associated with it.

Plausible Analytics stores usage data in a ClickHouse database, which is highly efficient in compressing and storing large datasets. Therefore, storage capacity is unlikely to become an issue for Secutils.dev's usage analytics in the foreseeable future.

I want to highlight that once Secutils.dev becomes profitable, supporting the small team behind Plausible Analytics is important to me. Therefore, it is highly likely that Plausible Analytics will be the first product for which I switch to a paid subscription.

## Secret management

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/)

No matter what type of product you're building, you will likely need to handle sensitive information and secrets such as master keys, API keys for third-party integrations, and passwords. Storing these secrets in private Git repositories is not recommended, and it's generally preferred to use a secure vault solution.

While you have the option to use the self-hosted Vault from HashiCorp, I take advantage of the fact that Oracle Cloud Free Tier [**includes a vault**](https://docs.oracle.com/en-us/iaas/Content/KeyManagement/Concepts/keyoverview.htm). Since it's already available to me, it makes sense to utilize it for my secret management needs.

## TLS certificates

**Cost:** 0€ / month

**Vendor:** [**Internet Security Research Group (Let's Encrypt)**](https://letsencrypt.org)

Gone are the days when you were required to pay for TLS certificates for your website. Thanks to the Internet Security Research Group, obtaining TLS certificates has become free and accessible. Since I have a Kubernetes cluster, I utilize [**Traefik**](https://doc.traefik.io/traefik/https/acme/) to automatically issue and renew TLS certificates for the `secutils.dev` domain and its subdomains. This service is provided by Let's Encrypt, and it comes at no cost to me.

## Storage

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/)

At the current stage of Secutils.dev, the storage requirements are minimal, and the 200 GB of block volume storage included in the [**Oracle Cloud Free Tier**](https://www.oracle.com/cloud/free/#always-free) is more than sufficient.

Additionally, the free tier includes 20 GB of Object Storage, which I utilize for backups of the main SQLite database through the Amazon S3 Compatibility API, using [**Litestream**](https://litestream.io). This allows me to securely store backups while staying within the free tier limits.

## Email hosting

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/) and [**Zoho**](https://www.zoho.com/mail/zohomail-pricing.html)

Within the [**Oracle Cloud Free Tier**](https://www.oracle.com/cloud/free/#always-free), an Email Delivery Service is available, allowing me to send up to 3000 emails per day for free. While I don't currently send a significant number of transactional emails to Secutils.dev users, it's great to have such a tool at my disposal.

For personalized, manually-crafted emails that I send from `*@secutils.dev` addresses, I utilize the [**Forever Free Plan**](https://www.zoho.com/mail/zohomail-pricing.html) provided by Zoho. They offer an excellent service, and if I ever reach the limits of the free plan, I am open to upgrading to a paid plan.

## Marketing

**Cost:** 0€ / month

**Vendor:** Word of mouth

Marketing for Secutils.dev is based solely on content marketing. I publish posts that I think people may find interesting and useful, and share them on my social media channels and in different niche communities. I do not engage in any paid advertising or marketing campaigns on social media or search engines, and I do not pay anyone to recommend Secutils.dev to their audience. Instead, I rely on the support of the Secutils.dev community to spread the word about the tool.

## Conclusion

In summary, the cost of running [**Secutils.dev**](https://secutils.dev) in production is nearly zero, excluding the investment of my time and energy. So why did I choose the title "Running micro-SaaS for less than 1€ a month" instead of "Running micro-SaaS for free"? The reason is that there are still expenses to consider. In my case, it's the cost of the `secutils.dev` domain name, which amounts to 11.3€ per year or 0.94€ per month!

While there are startup programs available that offer many of the required tools and resources for free, they often come with certain criteria that need to be fulfilled, and they are temporary in nature. During the early stages of a product, it can be more beneficial to focus on building the product and serving users rather than fulfilling the requirements of these programs.

Overall, the combination of free services and the minimal cost of the domain name makes it possible to bootstrap indie projects like Secutils.dev with a very low budget, as long as you're comfortable with taking ownership of all the technical and operational challenges involved.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
