---
title: Technology stack overview
description: "Technology stack overview of the Secutils.dev: Rust, Tantivy, TypeScript, React, SQLite, Docusaurus."
slug: technology-stack-overview
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [overview, technology]
---

Hello!

Today, I'd like to provide an overview of the technology stack powering [**Secutils.dev**](https://secutils.dev). Sharing this information might prove helpful to other individuals working on similar projects. So, without further ado, let's dive into the stack!

<!--truncate-->

<div class="text--center">
  <a href="/docs/blog/beta-release"><strong>🚀 Secutils.dev beta release is now public, click here to read more</strong></a>
</div>

---

**DISCLAIMER:**  I want to acknowledge that some of the choices I've made for the technology stack may seem like overkill to some. However, as a solo engineer/founder, it's crucial for me to maintain internal motivation to push things forward. Personally, I find that learning something new and tackling technical challenges serves as a great source of motivation. So, while it may appear unconventional, these choices align with my personal drive and passion for building Secutils.dev.

---

Currently, Secutils.dev is composed of three distinct components: a [**backend server**](https://github.com/secutils-dev/secutils), a [**Web UI client**](https://github.com/secutils-dev/secutils-webui), and a [**documentation website**](https://github.com/secutils-dev/secutils-docs). I made the decision to separate these components as it aligns with my long-term vision for Secutils.dev. In the future, I plan to transform the backend server, or a portion of it, into a lightweight library that can be distributed independently. This approach will enable tighter integration with third-party solutions at compile-time, providing a more seamless and efficient experience. By structuring Secutils.dev in this manner, I aim to create a flexible and extensible platform that can easily adapt to evolving needs and integrate with various systems.

## Backend

As you may have already deduced, the backbone of Secutils.dev lies within [**its backend server**](https://github.com/secutils-dev/secutils). This server acts as the foundation for exposing the core functionality of Secutils.dev to client applications.

### Programming language

I have extensive expertise in two languages: JavaScript (TypeScript) and Rust. Despite the fact that I could have rapidly developed a functional MVP using TypeScript and Node.js, I intentionally chose Rust as the programming language for the backend.

Now, while the usual benefits of Rust, such as memory safety and fearless concurrency, are certainly noteworthy, my primary motivation stems from a different aspect. In my experience, if a Rust program successfully compiles, it tends to function correctly most of the time. This aspect is paramount when striving for rapid iteration, minimizing the occurrence of trivial bugs, and fostering confidence in deploying changes to production. Bugs can be a major hassle, especially when they affect developer tools like Secutils.dev.

Moreover, Rust excels in cross-platform development. Although I mainly develop on the `x86` machine, I deploy compiled Rust code to much more cost-effective ARM servers (and even my mobile phone!). This process is incredibly smooth, thanks to the excellent tooling provided by Rust's Cargo.

### Web framework

To access the functionality of Secutils.dev, users can utilize either the [**Web UI**](https://github.com/secutils-dev/secutils-webui) or tools like `curl`. These interactions are facilitated through the HTTP APIs exposed by the Secutils.dev server. While there are several exceptional open-source web frameworks available in the Rust ecosystem, I opted for [**Actix Web**](https://github.com/actix/actix-web) for Secutils.dev based on my positive experience while working on [**AZbyte | ETF**](https://azbyte.xyz).

Actix Web stands out for its ease of use, speed, and comprehensive set of middle-wares, including authentication and session management.

### Database

When it comes to the database, Secutils.dev currently has relatively straightforward requirements. It needs to store user registrations, user data, active user sessions, and a few other internal data types. For these purposes, a simple SQLite database is more than sufficient.

To interact with the SQLite database from Rust, I rely on the fantastic [**SQLx**](https://github.com/launchbadge/sqlx) crate. It allows me to verify SQL queries at compile-time without the need for a domain-specific language (DSL). One of the great advantages of SQLx is its database-agnostic nature, which means that migrating to a different database like PostgreSQL in the future is straightforward if the need arises.

Tools like [**Litestream**](https://github.com/benbjohnson/litestream) and [**LiteFS**](https://github.com/superfly/litefs) alleviate my concerns regarding database backups and replication.

### Search engine

Although Secutils.dev currently has basic search capabilities, I believe that search will play a vital role in enhancing its overall usability in the future. Users should not only be able to find the right tool for their specific needs at any given moment but also have the ability to explore the accumulated data, including user notes, content of requests triggering auto-responders, scraped HTML data, and more.

To accomplish this, instead of relying on SQLite's built-in full-text search capabilities, I made the decision to leverage a dedicated full-text search engine written in Rust called [**Tantivy**](https://github.com/quickwit-oss/tantivy). Tantivy is an impressive, lightweight, and incredibly fast search engine that seamlessly integrates with Rust applications.

### Tests

When it comes to testing in Rust, there's not much to say except that it's a breeze! Writing tests in Rust is straightforward, thanks to the built-in testing framework provided by the language. Most of the time, running `cargo test` is all you need to validate your code.

However, I'd like to highlight a fantastic testing library called [**Insta**](https://github.com/mitsuhiko/insta). Insta is a snapshot testing library for Rust that brings the power of snapshot testing, similar to Jest, to the Rust ecosystem. If you're familiar with Jest snapshot testing, you'll appreciate how useful snapshots can be in unit tests.

## Frontend

The frontend or [**Web UI of Secutils.dev**](https://github.com/secutils-dev/secutils-webui) is a relatively straightforward "single-page" React application.

### Programming language

As I mentioned earlier, I have extensive experience in developing applications using JavaScript and TypeScript. Therefore, it was an obvious choice for me to utilize TypeScript for the Secutils.dev Web UI.

Both React and Parcel, a zero-configuration build tool, offer excellent support for TypeScript. This combination allows me to leverage the benefits of static typing and enhanced tooling, resulting in more robust and maintainable code.

### Web UI Framework

With the abundance of Web UI frameworks available today, I wanted to make a practical choice that would allow me to leverage my existing knowledge and meet the specific requirements of Secutils.dev. Rather than investing time in learning a new framework, I decided to work with a framework that I was already familiar with and confident would suit my needs: [**Elastic UI**](https://eui.elastic.co/).

It's worth mentioning that I have a personal connection to Elastic UI as I work for Elastic and know the team behind the framework. This familiarity gives me an added level of confidence in its capabilities and reliability.

However, it's important to note that I don't use React or Elastic UI for the Secutils.dev home page. To ensure the home page remains as lightweight as possible, I employ alternative approaches (static HTML + [**Tailwind CSS**](https://tailwindcss.com)).

## Documentation

For the documentation of Secutils.dev, I utilize the power of [**Docusaurus**](https://github.com/facebook/docusaurus). Docusaurus is a fantastic tool that simplifies the process of creating documentation websites.

One of the main reasons I chose Docusaurus is its support for writing documentation in Markdown format. Additionally, Docusaurus provides customizable styles and layouts, allowing me to maintain a consistent branding across the documentation. Another advantage of using Docusaurus is its built-in support for search engine optimization (SEO), making documentation more discoverable to users seeking information about Secutils.dev.

By leveraging Docusaurus, I can streamline the documentation process and devote more time to creating valuable content.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
