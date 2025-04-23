---
title: Deployment overview of micro-cluster for micro-SaaS
description: "Deployment overview of the micro Kubernetes cluster for the Secutils.dev micro-SaaS."
slug: deployment-overview
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [overview, technology]
---

Hello!

In [**my previous post**](/blog/2023-05-25-technology-stack-overview.md), I discussed the technological stack behind Secutils.dev and introduced its four separate components: a backend server written in Rust, a React-based Web UI client, a documentation website powered by Docusaurus, and a lightweight static promotional home page.

Today, I'll provide a brief overview of how and where I deploy these components to ensure the complete functionality of Secutils.dev.

<!--truncate-->

<div class="text--center">
  <a href="/docs/blog/beta-release"><strong>🚀 Secutils.dev beta release is now public, click here to read more</strong></a>
</div>

---

**DISCLAIMER:**  I want to acknowledge that some of the choices I've made for the technology stack may seem like overkill to some. However, as a solo engineer/founder, it's crucial for me to maintain internal motivation and drive to push forward. Personally, I find that learning something new and tackling technical challenges serves as a great source of motivation. So, while it may appear unconventional, these choices align with my personal drive and passion for building Secutils.dev.

---

The simplified structure of the Secutils.dev deployment looks like this:

`secutils.dev/*` ➡ [public] promotional home page / a few static HTMLs
`secutils.dev/docs/*` ➡ [public] documentation website / Docusaurus
`secutils.dev/ws/*` ➡ [private] user workspace / Web UI client
`secutils.dev/api/*` ➡ [private] API endpoints / backend server

All components of Secutils.dev have their own distinct scope and evolve independently. They are brought together during the deployment process. While it would have been possible to consolidate everything into a single repository and deploy as a single application, I have chosen a different approach. I prefer deploying the main components separately to allow for fine-tuning of allocated resources and better cost control.

Most of the components, except for the API server, primarily serve static resources and do not require significant computing power. These components can be effectively delegated to content delivery networks (CDNs) to enhance performance and scalability. On the other hand, the API server may need more flexible scaling mechanisms to handle potential increases in demand (🤞).

Given that I was already managing a self-hosted Kubernetes cluster in Oracle Cloud (which I will discuss in more detail in one of my next posts), I decided to deploy the Secutils.dev components as [**separate Kubernetes pods**](https://kubernetes.io/docs/concepts/workloads/pods/). This allows for efficient traffic routing using Traefik Ingress rules, ensuring that requests are directed to the appropriate pods based on the URL:

```yaml
apiVersion: traefik.containo.us/v1alpha1
kind: IngressRoute
...
spec:
  routes:
    - kind: Rule
      match: Host(`secutils.dev`) && PathPrefix(`/api`)
      services:
        - kind: Service
          name: secutils-api-svc // backend server pod
          port: 7070
    - kind: Rule
      match: Host(`secutils.dev`) && PathPrefix(`/docs`)
      services:
        - kind: Service
          name: secutils-docs-svc // documentation pod
          port: 7373
```

For more details on Traefik Ingress rules, you can refer to the [**official documentation**](https://doc.traefik.io/traefik/providers/kubernetes-ingress/).

To automate the issuance and renewal of TLS certificates for the `secutils.dev` domain name, I utilize [**Traefik along with Let's Encrypt**](https://doc.traefik.io/traefik/https/acme/). The use of TLS certificates is essential, especially for the `.dev` top-level domain, which is included on the [**HSTS preload list**](https://get.dev). This list mandates that all connections to `.dev` websites be made over HTTPS. By leveraging Traefik, I can ensure that the TLS certificates are automatically managed and renewed, eliminating the risk of overlooking the certificate renewal.

Each component of Secutils.dev has its own Git repository, and within each repository, there is a `Dockerfile` provided. These files are used to build Docker images that are subsequently deployed to the Kubernetes cluster. To optimize the size and efficiency of the Docker images, I employ [**multi-stage builds**](https://docs.docker.com/build/building/multi-stage/). This approach allows me to include only the necessary dependencies and artifacts in the final image, resulting in a lightweight and efficient container. You can find an example of this approach in the [**Web UI `Dockerfile`**](https://github.com/secutils-dev/secutils-webui/blob/main/Dockerfile) of the Secutils.dev project:

```bash
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM node:20-alpine3.17 as UI_BUILDER
...
RUN set -x && npm run build

FROM nginx:stable-alpine
COPY --from=UI_BUILDER ["/app/dist/", "/usr/share/nginx/html/"]
COPY ["./config/nginx.conf", "/etc/nginx/conf.d/default.conf"]
```

For components that serve pre-built static assets, I opt for the NGINX Alpine Linux image as the base image. NGINX is well-known for its speed and configurability, and its Alpine Docker image is lightweight. In each component's repository, you can find the NGINX configuration file ([**example here**](https://github.com/secutils-dev/secutils-webui/blob/main/config/nginx.conf)) that includes settings for Content Security Policy (CSP), compression, and additional routing configurations.

When preparing to deploy a new version to the production environment, I follow a specific process. Initially, I push the changes to a dedicated "dev" environment to perform a quick smoke test. While Kubernetes simplifies managing multiple environments, I acknowledge that the manual deployment process can be somewhat inefficient. To address this, I am currently exploring the use of [**Argo CD**](https://argo-cd.readthedocs.io/en/stable/) to automate the continuous deployment process for the dev environment.

Although deploying to Kubernetes may seem complex initially, it offers significant advantages in terms of deployment control, orchestration, and scalability. In this post, I had to omit some of the finer details to maintain readability. However, if you have any specific questions about the deployment of Secutils.dev, please feel free to leave a comment, and I'll be more than happy to provide detailed answers and insights!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
