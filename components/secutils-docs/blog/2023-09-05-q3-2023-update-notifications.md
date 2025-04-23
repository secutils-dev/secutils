---
title: "Q3 2023 update - Notifications"
description: "Q3 2023 update - Notifications: scheduler, email batching, Loops."
slug: q3-2023-update-notifications
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-09-05_q3_2023_update_notifications.png
tags: [overview, technology, application-security]
---
Hello!

With just one month remaining in the "Q3 2023 - Jul-Sep" milestone (this is how I structure [**my roadmap**](https://github.com/orgs/secutils-dev/projects/1/views/1)), I wanted to provide a quick progress update. A significant deliverable for this milestone includes adding support for email notifications and other transactional emails.

Notifications, in general, and email notifications, specifically, are integral to any product that involves any monitoring or tracking activities. [**Secutils.dev**](https://secutils.dev) already includes, and will continue to expand, features that require the ability to send notifications. Two notable examples include sending notifications for changes detected by the web page resources trackers and changes detected in the tracked content security policies (CSP).

<!--truncate-->

Of course, I don't have infinite resources or time to architect different notification solutions for various use cases. Therefore, the notifications subsystem must be flexible and robust enough to cover all of today's needs and those that might arise in the near future. This includes seemingly unrelated use cases, such as new account activation emails, password reset emails, and even contact form messages. If you take a moment to think about it, you'll realize that these are essentially just different types of notifications.

For now, a fairly simple [**Notification definition**](https://github.com/secutils-dev/secutils/blob/2f6c10bc5c47e0ef217fbd7874dd41ceda41ba8e/src/notifications/notification.rs) (shown below) covers all my immediate needs:
```rust
/// Defines a notification.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Notification {
    /// Unique id of the notification.
    pub id: NotificationId,
    /// The destination of the notification (e.g. registered user, email or server log).
    pub destination: NotificationDestination,
    /// The content of the notification (e.g. simple text or HTML email).
    pub content: NotificationContent,
    /// The time at which the notification is scheduled to be sent, in UTC.
    pub scheduled_at: OffsetDateTime,
}
---
// Example #1: Resources tracker detected changes in the web page reso
Notification::new(
    NotificationDestination::User(12345),
    NotificationContent::Text(format!(
        "Web page resources tracker {} ({}) detected changes in resources.",
        tracker.name, tracker.url
    )),
    OffsetDateTime::now_utc()
)
---
// Example #2: New account activation email
Notification::new(
    NotificationDestination::Email("12@34.56".to_string()),
    NotificationContent::Email(NotificationEmailContent::html(
        "Activate you Secutils.dev account",
        // Plain text fallback for simple email clients.    
        format!("To activate your Secutils.dev account…"),
        // HTML email for advanced email clients.
        format!(r#"
<!DOCTYPE html>
<html>
  <head><title>Activate your Secutils.dev account</title>…</head>
  <body><h1>Activate your Secutils.dev account</h1>…</body>
</html>"#)
    )),
    OffsetDateTime::now_utc()
)
```

To compose and send emails, I rely on an incredible open-source Rust library called [**Lettre**](https://github.com/lettre/lettre).

Sending notifications over the network can be quite resource-intensive, and I don't want to let it affect the user experience or block the primary functionality of Secutils.dev. To deal with this, I've implemented a system where notifications are sent in a separate thread. Besides, Secutils.dev doesn't send notifications immediately. Instead, it [**schedules them for batch delivery**](https://github.com/secutils-dev/secutils/blob/2f6c10bc5c47e0ef217fbd7874dd41ceda41ba8e/src/notifications/api_ext.rs#L36-L46) at regular intervals. Scheduling notifications is very lightweight, essentially as cheap as inserting a single row into a SQLite database.

As I previously covered in my [**“Building a scheduler for a Rust application”**](https://secutils.dev/docs/blog/scheduler-component) post, I rely on [**Tokio Cron Scheduler**](https://github.com/mvniekerk/tokio-cron-scheduler) for various routine background tasks, and [**one such task**](https://github.com/secutils-dev/secutils/blob/2f6c10bc5c47e0ef217fbd7874dd41ceda41ba8e/src/scheduler/scheduler_jobs/notifications_send_job.rs) runs every 30 seconds (although the default value, it's configurable). This task checks if there are any pending notifications ready for dispatch. Fortunately, the extra 30-second delay doesn't significantly impact my use cases, but it allows me to manage the load more effectively. This approach works equally well for both near-real-time notifications and those scheduled for future delivery.

The notification sending job also tracks the time taken to send each batch of notifications, recording this data in the server log. This information is then captured by [**my monitoring setup**](https://secutils.dev/docs/blog/usage-analytics-and-monitoring#monitoring). This monitoring approach helps me determine when it's necessary to fine-tune or scale up my notifications setup.

Speaking of scalability, the separation between the code responsible for preparing notifications and the code handling their actual transmission gives me the flexibility to scale my notification system easily. I can achieve this by deploying one or more dedicated Secutils.dev server instances solely tasked with serving pending notifications. These dedicated instances may have entirely different hardware profiles optimized just for this purpose.

Today, Secutils.dev exclusively supports two types of notification destinations: email and the server log. The rationale for requiring email notifications is self-evident. However, it's worth shedding light on the server log as a notification destination. Utilizing the server log as a notification destination is not only very handy for debugging purposes but also serves as an integration channel between Secutils.dev and the Elastic Stack monitoring system. All server logs are ingested into the Elasticsearch instance I've deployed for monitoring. I've crafted a dedicated dashboard and several visualizations within Kibana to specifically monitor server log notifications, leveraging the `[Notification]` log record prefix. This allows me to track and visualize any imaginable custom application metric.

As it's set up right now, the notifications system can be expanded to include more destinations like Slack, Telegram, browser push notifications, or any other third-party webhook with minimal tweaks. And speaking of the future, once Secutils.dev makes it out of beta, I'll be handling more sophisticated transactional or marketing emails, and planning to add [**Loops**](https://loops.so) as the notification destination.  Loops is a nice service positioned as "Email for modern SaaS". They also offer a pretty generous free plan, check it out!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
