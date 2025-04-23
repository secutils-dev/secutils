---
title: Does a one-man project need a formal project management process?
description: "Does a one-man project like Secutils.dev need a formal project management process? Project management of the open-source project in GitHub and Notion."
slug: project-management
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-06-06_roadmap.png
tags: [overview, project-management]
---
Hello!

Today, I'd like to share my perspective on formal project management for small to medium-sized projects, using [**Secutils.dev**](https://secutils.dev) as an example. When starting a new project, it's often driven by a spark of inspiration or a strong desire to solve a specific issue for yourself or a group of people. At this early stage, formality can be a distraction and drain motivation quickly. You have a clear vision of what needs to be done, and adding unnecessary formalities can hinder progress.

Initially, things may go smoothly without a formal project management process. You create functional prototypes, launch an MVP with a catchy domain name, and receive positive feedback from early users. However, over time, the excitement from these achievements can diminish, and internal motivation alone may not be enough to drive the project forward. This is a natural human tendency, and it's important to recognize it. If you're satisfied with your project in its current state, or if it was originally intended as a short-term fun project and you're ready to move on to something new, that's perfectly fine. You should absolutely embrace the joy of building and exploring new ideas.

However, if you want to advance a more complex project and still maintain sufficient motivation, I believe it's essential to adopt a different strategy. The strategy I'm going to discuss next involves incorporating a bit of formal project management to keep yourself on track, sustain progress, and avoid the disappointment of yet another unfinished project.

<!--truncate-->

---

Here are the previous posts in the "Building Secutils.dev" series:

- [**Technology stack overview**](/blog/2023-05-25-technology-stack-overview.md)
- [**Deployment overview of micro-cluster for micro-SaaS**](/blog/2023-05-28-deployment-overview.md)
- [**Privacy-friendly usage analytics and monitoring**](/blog/2023-05-30-usage-analytics-and-monitoring.md)
- [**Running micro-SaaS for less than 1€ a month**](/blog/2023-06-01-running-micro-saas-for-less-than-one-euro-a-month.md)

---

## Aspirational goal

When tackling a more complex project, adding a bit of formality and order can be helpful. The first step is to set an aspirational goal for the project. This goal should be ambitious enough to drive your motivation, yet still realistic and approachable. Write it down and, if possible, make it public. Whether it's on the project's GitHub page, the project website, or even in a social media post or tweet, publicly stating your goal adds an extra sense of accountability.

For example, the ultimate goal for Secutils.dev is to become the go-to place for engineers needing tools to build and test secure applications. In the modern digital world, security is not a luxury but a necessity. While this goal is ambitious, it is achievable in theory, considering the success of dominant software tools in other areas. By stating this goal on the GitHub repository and project website, it keeps me motivated and accountable, even though the odds may not be in my favor.

![Secutils.dev promo page](https://secutils.dev/docs/img/blog/goal.png)

## Public roadmap

Having a clearly formulated goal is one part of the puzzle, but the other, and more challenging, part is creating an actual roadmap to achieve that goal. Your roadmap should provide a high-level overview of where your project is heading and when you expect to reach certain milestones. By structuring your ideas and attaching time frames to them, you can measure your progress and adjust your plans if necessary, freeing up your mind for more creative and rewarding work.

In the case of Secutils.dev, since the source code is hosted on GitHub, I utilize [**GitHub Projects**](https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/about-projects) for my roadmap. This allows me to outline high-level intermediate goals and approximate time frames for achieving them. I have made my roadmap public [**here**](https://github.com/orgs/secutils-dev/projects/1/views/1), which adds an extra layer of accountability. While the roadmap is subject to change as ideas evolve, it helps me stay focused and avoid getting distracted by constant influx of new ideas.

Unsurprisingly, GitHub Projects provides seamless integration with other GitHub features, such as issues, wikis, and pull requests, making it incredibly useful for project management.

![Secutils.dev public roadmap](https://secutils.dev/docs/img/blog/2023-06-06_roadmap.png)

## Work breakdown

The roadmap serves as a guide for the project, but to effectively achieve your goals, it's crucial to break down the work into smaller, actionable tasks. These tasks should be manageable within a reasonable time frame, providing a sense of accomplishment as you complete them. This not only boosts morale but also maintains your motivation to continue working on the project. Such tasks are primarily for you, allowing you to stay focused and know exactly what needs to be done at any given moment. The broader public and users typically don't need to see this level of detail, as they are more interested in the overall progress and outcomes.

In the case of Secutils.dev, I utilize GitHub issues to break down the work and have created a dedicated GitHub project for managing these issues. The project is private, ensuring that it's solely for my reference and not intended for public consumption. Its structure allows me to prioritize each issue or task. I use both public issues and private ad-hoc "drafts" within this project. Public issues are visible to collaborators and the community, while private drafts are for internal notes and early-stage ideas that may not be relevant to the wider audience.

![Secutils.dev private work breakdown](https://secutils.dev/docs/img/blog/2023-06-06_breakdown.png)

In addition to GitHub, I also utilize a dedicated Secutils.dev “space” in Notion for more sensitive non-public information. Notion serves as a tool for various purposes: drafting blog posts and maintaining a knowledge database for ideas, legal inquiries, competitive analysis, and more. It’s an awesome tool that I highly recommend.

![Secutils.dev Notion “space”](https://secutils.dev/docs/img/blog/2023-06-06_notion.png)

## Conclusion

In conclusion, whether a one-man project needs formal project management depends on various factors. For short-lived or hobby projects, as well as early prototypes, formal project management may not be helpful and could even be harmful. However, for more complex projects like Secutils.dev that span months or longer, adopting a formal process can be beneficial. Fortunately, there are many free tools available today, such as GitHub Projects and Notion, that can assist in project management.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
