use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        EmailNotificationAttachment, EmailNotificationContent,
        notification_content_template::SECUTILS_LOGO_BYTES,
    },
};
use serde_json::json;
use uuid::Uuid;

/// Content shorter than this threshold is shown in full; longer content uses the diff view.
const DIFF_CONTENT_LENGTH_THRESHOLD: usize = 200;

/// Compiles page tracker changes template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    tracker_id: Uuid,
    tracker_name: &str,
    content: &Result<String, String>,
    diff: Option<&str>,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!(
        "{}ws/web_scraping__page?q={}",
        api.config.public_url, tracker_id
    );
    let (subject, text, html) = match content {
        Ok(content) => {
            let diff_html = diff
                .filter(|_| content.len() > DIFF_CONTENT_LENGTH_THRESHOLD)
                .map(diff_to_html);
            (
                format!("[Secutils.dev] Change detected: \"{tracker_name}\""),
                format!(
                    "\"{tracker_name}\" tracker detected changes. Visit {back_link} to learn more.",
                ),
                api.templates.render(
                    "page_tracker_changes_email",
                    &json!({
                        "tracker_name": tracker_name,
                        "content": content,
                        "diff_html": diff_html,
                        "back_link": back_link,
                        "home_link": api.config.public_url.as_str(),
                    }),
                )?,
            )
        }
        Err(error_message) => (
            format!("[Secutils.dev] Check failed: \"{tracker_name}\""),
            format!(
                "\"{tracker_name}\" tracker failed to check for changes due to the following error: {error_message}. Visit {back_link} to learn more."
            ),
            api.templates.render(
                "page_tracker_changes_error_email",
                &json!({
                    "tracker_name": tracker_name,
                    "error_message": error_message,
                    "back_link": back_link,
                    "home_link": api.config.public_url.as_str(),
                }),
            )?,
        ),
    };

    Ok(EmailNotificationContent::html_with_attachments(
        subject,
        text,
        html,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            SECUTILS_LOGO_BYTES.to_vec(),
        )],
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn diff_to_html(diff: &str) -> String {
    let mut out = String::from("<div class=\"diff-block\">");
    for line in diff.lines() {
        let class = if line.starts_with('+') {
            "diff-add"
        } else if line.starts_with('-') {
            "diff-del"
        } else if line.starts_with("@@") {
            "diff-hunk"
        } else {
            "diff-ctx"
        };
        out.push_str(&format!(
            "<div class=\"{class}\">{}</div>",
            html_escape(line)
        ));
    }
    out.push_str("</div>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_to_html_renders_lines_with_correct_classes() {
        let diff = "@@ -1,3 +1,3 @@\n context\n-removed\n+added\n";
        let html = diff_to_html(diff);
        assert!(html.starts_with("<div class=\"diff-block\">"));
        assert!(html.ends_with("</div>"));
        assert!(html.contains("<div class=\"diff-hunk\">@@ -1,3 +1,3 @@</div>"));
        assert!(html.contains("<div class=\"diff-ctx\"> context</div>"));
        assert!(html.contains("<div class=\"diff-del\">-removed</div>"));
        assert!(html.contains("<div class=\"diff-add\">+added</div>"));
    }

    #[test]
    fn diff_to_html_escapes_html_entities() {
        let diff = "+<script>alert(1)</script>\n";
        let html = diff_to_html(diff);
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn html_escape_handles_all_entities() {
        assert_eq!(
            html_escape("<b>&\"x\"</b>"),
            "&lt;b&gt;&amp;&quot;x&quot;&lt;/b&gt;"
        );
    }
}
