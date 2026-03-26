mod raw_util;

use crate::{
    database::Database,
    users::UserId,
    utils::{HomeSummary, HomeSummaryCounts, HomeSummaryRecentItem, Util},
};
use anyhow::bail;
use raw_util::RawUtil;
use sqlx::query_as;
use std::collections::HashMap;
use time::OffsetDateTime;

/// Extends the primary database with the utility-related methods.
impl Database {
    /// Retrieves all utils from the `Utils` table.
    pub async fn get_utils(&self) -> anyhow::Result<Vec<Util>> {
        let mut root_utils = query_as!(
            RawUtil,
            r#"
SELECT id, handle, name, keywords, parent_id
FROM utils
ORDER BY parent_id NULLS FIRST, id
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        // Utilities are sorted by the parent_id meaning that all root utilities are returned first.
        let child_utils = if let Some(position) = root_utils
            .iter()
            .position(|raw_util| raw_util.parent_id.is_some())
        {
            root_utils.split_off(position)
        } else {
            return root_utils.into_iter().map(Util::try_from).collect();
        };

        let mut parent_children_map = HashMap::<_, Vec<_>>::new();
        for util in child_utils {
            if let Some(parent_id) = util.parent_id {
                parent_children_map.entry(parent_id).or_default().push(util);
            } else {
                bail!("Child utility does not have a parent id.");
            }
        }

        root_utils
            .into_iter()
            .map(|root_util| Self::build_util_tree(root_util, &mut parent_children_map))
            .collect()
    }

    fn build_util_tree(
        raw_util: RawUtil,
        parent_children_map: &mut HashMap<i32, Vec<RawUtil>>,
    ) -> anyhow::Result<Util> {
        let utils = if let Some(mut children) = parent_children_map.remove(&raw_util.id) {
            Some(
                children
                    .drain(..)
                    .map(|util| Self::build_util_tree(util, parent_children_map))
                    .collect::<anyhow::Result<_>>()?,
            )
        } else {
            None
        };

        Util::try_from(raw_util).map(|util| Util { utils, ..util })
    }

    /// Retrieves a summary of all util items for the specified user: per-tool counts and the most
    /// recently updated items across all tools.
    pub async fn get_home_summary(&self, user_id: UserId) -> anyhow::Result<HomeSummary> {
        let (counts, recent_items) = tokio::try_join!(
            async {
                query_as::<_, (i64, i64, i64, i64)>(
                    r#"
SELECT
    (SELECT COUNT(*) FROM user_data_webhooks_responders WHERE user_id = $1),
    (SELECT COUNT(*) FROM user_data_certificates_certificate_templates WHERE user_id = $1)
        + (SELECT COUNT(*) FROM user_data_certificates_private_keys WHERE user_id = $1),
    (SELECT COUNT(*) FROM user_data_web_security_csp WHERE user_id = $1),
    (SELECT COUNT(*) FROM user_data_web_scraping_page_trackers WHERE user_id = $1)
        + (SELECT COUNT(*) FROM user_data_web_scraping_api_trackers WHERE user_id = $1)
                    "#,
                )
                .bind(*user_id)
                .fetch_one(&self.pool)
                .await
                .map(
                    |(webhooks, certificates, csp, web_scraping)| HomeSummaryCounts {
                        webhooks,
                        certificates,
                        csp,
                        web_scraping,
                    },
                )
            },
            async {
                query_as::<_, (String, String, OffsetDateTime)>(
                    r#"
WITH util_handles AS (
    SELECT
        (SELECT handle FROM utils WHERE id = 3) AS webhooks_responders,
        (SELECT handle FROM utils WHERE id = 5) AS certificates_certificate_templates,
        (SELECT handle FROM utils WHERE id = 6) AS certificates_private_keys,
        (SELECT handle FROM utils WHERE id = 9) AS web_security_csp_policies,
        (SELECT handle FROM utils WHERE id = 11) AS web_scraping_page,
        (SELECT handle FROM utils WHERE id = 12) AS web_scraping_api
)
SELECT name, util_handle, updated_at FROM (
    SELECT
        responders.name,
        util_handles.webhooks_responders AS util_handle,
        responders.updated_at
    FROM user_data_webhooks_responders AS responders
    CROSS JOIN util_handles
    WHERE responders.user_id = $1
    UNION ALL
    SELECT
        certificate_templates.name,
        util_handles.certificates_certificate_templates AS util_handle,
        certificate_templates.updated_at
    FROM user_data_certificates_certificate_templates AS certificate_templates
    CROSS JOIN util_handles
    WHERE certificate_templates.user_id = $1
    UNION ALL
    SELECT
        private_keys.name,
        util_handles.certificates_private_keys AS util_handle,
        private_keys.updated_at
    FROM user_data_certificates_private_keys AS private_keys
    CROSS JOIN util_handles
    WHERE private_keys.user_id = $1
    UNION ALL
    SELECT
        csp.name,
        util_handles.web_security_csp_policies AS util_handle,
        csp.updated_at
    FROM user_data_web_security_csp AS csp
    CROSS JOIN util_handles
    WHERE csp.user_id = $1
    UNION ALL
    SELECT
        page_trackers.name,
        util_handles.web_scraping_page AS util_handle,
        page_trackers.updated_at
    FROM user_data_web_scraping_page_trackers AS page_trackers
    CROSS JOIN util_handles
    WHERE page_trackers.user_id = $1
    UNION ALL
    SELECT
        api_trackers.name,
        util_handles.web_scraping_api AS util_handle,
        api_trackers.updated_at
    FROM user_data_web_scraping_api_trackers AS api_trackers
    CROSS JOIN util_handles
    WHERE api_trackers.user_id = $1
) AS items
ORDER BY updated_at DESC
LIMIT 3
                    "#,
                )
                .bind(*user_id)
                .fetch_all(&self.pool)
                .await
                .map(|rows| {
                    rows.into_iter()
                        .map(|(name, util_handle, updated_at)| HomeSummaryRecentItem {
                            name,
                            util_handle,
                            updated_at,
                        })
                        .collect::<Vec<_>>()
                })
            }
        )?;

        Ok(HomeSummary {
            counts,
            recent_items,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        retrack::RetrackTracker,
        tests::{MockCertificateAttributes, MockResponderBuilder, mock_user, mock_user_with_id},
        utils::{
            HomeSummaryCounts,
            certificates::{
                CertificateTemplate, PrivateKey, PrivateKeyAlgorithm, PrivateKeySize,
                SignatureAlgorithm, Version,
            },
            web_scraping::tests::{MockApiTrackerBuilder, MockPageTrackerBuilder},
            web_security::{ContentSecurityPolicy, ContentSecurityPolicyDirective},
        },
    };
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn returns_empty_summary_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let summary = db.get_home_summary(user.id).await?;

        assert_eq!(
            summary.counts,
            HomeSummaryCounts {
                webhooks: 0,
                certificates: 0,
                csp: 0,
                web_scraping: 0,
            }
        );
        assert!(summary.recent_items.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn returns_correct_aggregated_counts(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        // Insert 2 responders.
        for (id, name, path) in [
            (
                uuid!("00000000-0000-0000-0000-000000000001"),
                "resp-1",
                "/a",
            ),
            (
                uuid!("00000000-0000-0000-0000-000000000002"),
                "resp-2",
                "/b",
            ),
        ] {
            db.webhooks()
                .insert_responder(
                    user.id,
                    &MockResponderBuilder::create(id, name, path)?.build(),
                )
                .await?;
        }

        // Insert 1 certificate template + 1 private key → certificates = 2.
        let template = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000010"),
            name: "tpl-1".to_string(),
            attributes: MockCertificateAttributes::new(
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                OffsetDateTime::from_unix_timestamp(946720800)?,
                OffsetDateTime::from_unix_timestamp(1262340000)?,
                Version::Three,
            )
            .build(),
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_certificate_template(user.id, &template)
            .await?;

        let private_key = PrivateKey {
            id: uuid!("00000000-0000-0000-0000-000000000020"),
            name: "pk-1".to_string(),
            alg: PrivateKeyAlgorithm::Rsa {
                key_size: PrivateKeySize::Size2048,
            },
            pkcs8: vec![1, 2, 3],
            encrypted: true,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.certificates()
            .insert_private_key(user.id, &private_key)
            .await?;

        // Insert 1 CSP policy.
        let policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000030"),
            name: "csp-1".to_string(),
            directives: vec![ContentSecurityPolicyDirective::DefaultSrc(
                ["'self'".to_string()].into_iter().collect(),
            )],
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.web_security()
            .insert_content_security_policy(user.id, &policy)
            .await?;

        // Insert 1 page tracker + 2 api trackers → web_scraping = 3.
        let page_tracker = MockPageTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000040"),
            "page-1",
            RetrackTracker::from_reference(uuid!("00000000-0000-0000-0000-000000000041")),
        )?
        .build();
        db.web_scraping(user.id)
            .insert_page_tracker(&page_tracker)
            .await?;

        for (id, name, retrack_id) in [
            (
                uuid!("00000000-0000-0000-0000-000000000050"),
                "api-1",
                uuid!("00000000-0000-0000-0000-000000000051"),
            ),
            (
                uuid!("00000000-0000-0000-0000-000000000060"),
                "api-2",
                uuid!("00000000-0000-0000-0000-000000000061"),
            ),
        ] {
            let tracker = MockApiTrackerBuilder::create(
                id,
                name,
                RetrackTracker::from_reference(retrack_id),
            )?
            .build();
            db.web_scraping(user.id)
                .insert_api_tracker(&tracker)
                .await?;
        }

        let summary = db.get_home_summary(user.id).await?;

        assert_eq!(
            summary.counts,
            HomeSummaryCounts {
                webhooks: 2,
                certificates: 2,
                csp: 1,
                web_scraping: 3,
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn returns_top_3_recent_items_across_tools(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        // Insert items with distinct timestamps across different tools.
        // Responder: updated_at = 1000
        db.webhooks()
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "oldest-responder",
                    "/old",
                )?
                .build(),
            )
            .await?;

        // CSP: updated_at = 946720810 (default from struct creation)
        let policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000002"),
            name: "mid-csp".to_string(),
            directives: vec![],
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(1000000000)?,
        };
        db.web_security()
            .insert_content_security_policy(user.id, &policy)
            .await?;

        // Certificate template: newest
        let template = CertificateTemplate {
            id: uuid!("00000000-0000-0000-0000-000000000003"),
            name: "newest-template".to_string(),
            attributes: MockCertificateAttributes::new(
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                OffsetDateTime::from_unix_timestamp(946720800)?,
                OffsetDateTime::from_unix_timestamp(1262340000)?,
                Version::Three,
            )
            .build(),
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(1100000000)?,
        };
        db.certificates()
            .insert_certificate_template(user.id, &template)
            .await?;

        // Private key: second newest
        let private_key = PrivateKey {
            id: uuid!("00000000-0000-0000-0000-000000000004"),
            name: "second-pk".to_string(),
            alg: PrivateKeyAlgorithm::Ed25519,
            pkcs8: vec![1, 2, 3],
            encrypted: false,
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(1050000000)?,
        };
        db.certificates()
            .insert_private_key(user.id, &private_key)
            .await?;

        let summary = db.get_home_summary(user.id).await?;

        assert_eq!(summary.recent_items.len(), 3);

        assert_eq!(summary.recent_items[0].name, "newest-template");
        assert_eq!(
            summary.recent_items[0].util_handle,
            "certificates__certificate_templates"
        );

        assert_eq!(summary.recent_items[1].name, "second-pk");
        assert_eq!(
            summary.recent_items[1].util_handle,
            "certificates__private_keys"
        );

        assert_eq!(summary.recent_items[2].name, "mid-csp");
        assert_eq!(
            summary.recent_items[2].util_handle,
            "web_security__csp__policies"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn isolates_data_between_users(pool: PgPool) -> anyhow::Result<()> {
        let user_a = mock_user()?;
        let user_b = mock_user_with_id(uuid!("00000000-0000-0000-0000-000000000002"))?;
        let db = Database::create(pool).await?;
        db.insert_user(&user_a).await?;
        db.insert_user(&user_b).await?;

        // Insert a responder for user A.
        db.webhooks()
            .insert_responder(
                user_a.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000010"),
                    "user-a-resp",
                    "/a",
                )?
                .build(),
            )
            .await?;

        // Insert a CSP for user B.
        let policy = ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000020"),
            name: "user-b-csp".to_string(),
            directives: vec![],
            tags: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
        };
        db.web_security()
            .insert_content_security_policy(user_b.id, &policy)
            .await?;

        // User A sees only their responder.
        let summary_a = db.get_home_summary(user_a.id).await?;
        assert_eq!(summary_a.counts.webhooks, 1);
        assert_eq!(summary_a.counts.csp, 0);
        assert_eq!(summary_a.recent_items.len(), 1);
        assert_eq!(summary_a.recent_items[0].name, "user-a-resp");

        // User B sees only their CSP.
        let summary_b = db.get_home_summary(user_b.id).await?;
        assert_eq!(summary_b.counts.webhooks, 0);
        assert_eq!(summary_b.counts.csp, 1);
        assert_eq!(summary_b.recent_items.len(), 1);
        assert_eq!(summary_b.recent_items[0].name, "user-b-csp");

        Ok(())
    }
}
