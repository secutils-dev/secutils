pub mod api_trackers;
pub mod certificate_templates;
pub mod content_security_policies;
mod home_summary_get;
pub mod notifications_unsubscribe;
pub mod page_trackers;
pub mod private_keys;
pub mod responders;
pub mod scheduler_parse_schedule;
pub mod search;
pub mod security_subscription_update;
pub mod security_users_clone;
pub mod security_users_email;
pub mod security_users_get;
pub mod security_users_get_by_email;
pub mod security_users_get_self;
pub mod security_users_remove;
pub mod security_users_signup;
pub mod send_message;
pub mod status_get;
pub mod status_set;
mod ui_state_get;
pub mod user_api_keys;
pub mod user_data_export;
pub mod user_data_import;
pub mod user_notification_email;
pub mod user_scripts;
pub mod user_secrets;
pub mod user_settings_get;
pub mod user_settings_set;
pub mod user_tags;
mod webhooks_responders;
mod webhooks_retrack;

pub use self::{
    home_summary_get::home_summary_get, ui_state_get::ui_state_get,
    webhooks_responders::webhooks_responders, webhooks_retrack::webhooks_retrack,
};

use crate::{
    error::Error,
    server::app_state::AppState,
    users::{SharedResource, User, UserShare},
};
use actix_web::web;
use utoipa::OpenApi;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearerAuth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "JWT token obtained from the authentication service. \
                             Pass as `Authorization: Bearer <token>`.",
                        ))
                        .build(),
                ),
            );
        }
    }
}

/// Resolves the effective user for a shared-resource-aware handler.
///
/// If a `UserShare` is present and its resource matches the expected shared resource, the share
/// owner is resolved from the database. If only a directly authenticated `User` is present, that
/// user is returned. Returns `Error::access_forbidden()` when neither is available.
pub(crate) async fn resolve_shared_user(
    state: &web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
    expected_resource: &SharedResource,
) -> Result<User, Error> {
    match (user, user_share) {
        // Authenticated user without a share header - use directly.
        (Some(user), None) => Ok(user),
        // Authenticated user whose share belongs to themselves - use directly.
        (Some(user), Some(ref share)) if user.id == share.user_id => Ok(user),
        // Share present (anonymous or different user) - verify resource and resolve owner.
        (_, Some(share)) if &share.resource == expected_resource => state
            .api
            .users()
            .get(share.user_id)
            .await?
            .ok_or_else(Error::access_forbidden),
        // No valid credentials.
        _ => Err(Error::access_forbidden()),
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Secutils",
        description = "An open-source, versatile, yet simple security toolbox for engineers and researchers.",
        contact(name = "Aleh Zasypkin", email = "dev@secutils.dev"),
        license(
            name = "AGPL-3.0",
            url = "https://github.com/secutils-dev/secutils/blob/main/LICENSE"
        )
    ),
    external_docs(
        url = "https://secutils.dev/docs",
        description = "Secutils.dev documentation"
    ),
    tags(
        (name = "webhooks", description = "Create HTTP responders that capture and replay incoming requests."),
        (name = "certificates", description = "Generate X.509 certificate templates and manage private keys."),
        (name = "web_security", description = "Build, parse, and serialize Content Security Policy headers."),
        (name = "tags", description = "Organize resources with colored tags."),
        (name = "secrets", description = "Store encrypted secrets for use in responder and tracker scripts."),
        (name = "scripts", description = "Manage reusable JavaScript scripts for responders and trackers."),
        (name = "settings", description = "Read and update user preferences."),
        (name = "data", description = "Export and import user data."),
        (name = "status", description = "Application status and health."),
        (name = "users", description = "User registration, lookup, and subscription management."),
        (name = "scheduler", description = "Schedule parsing utilities."),
        (name = "search", description = "Full-text search across user resources."),
        (name = "messages", description = "Send messages and notifications."),
        (name = "web_scraping", description = "Track changes to web pages and API endpoints."),
        (name = "api_keys", description = "Create and manage API keys for programmatic access."),
        (name = "notifications", description = "Public endpoints for managing notification delivery.")
    ),
    paths(
        // API keys
        user_api_keys::user_api_keys_list,
        user_api_keys::user_api_keys_create,
        user_api_keys::user_api_keys_update,
        user_api_keys::user_api_keys_delete,
        user_api_keys::user_api_keys_regenerate,
        user_api_keys::user_api_keys_create_for_user,
        // Tags
        user_tags::user_tags_list,
        user_tags::user_tags_create,
        user_tags::user_tags_update,
        user_tags::user_tags_delete,
        // Secrets
        user_secrets::user_secrets_list,
        user_secrets::user_secrets_create,
        user_secrets::user_secrets_update,
        user_secrets::user_secrets_delete,
        // Scripts
        user_scripts::user_scripts_list,
        user_scripts::user_scripts_get,
        user_scripts::user_scripts_create,
        user_scripts::user_scripts_update,
        user_scripts::user_scripts_delete,
        // Settings
        user_settings_get::user_settings_get,
        user_settings_set::user_settings_set,
        user_notification_email::user_notification_email_get,
        user_notification_email::user_notification_email_set,
        user_notification_email::user_notification_email_verify,
        user_notification_email::user_notification_email_resend,
        user_notification_email::user_notification_email_delete,
        // Notifications (public)
        notifications_unsubscribe::notifications_unsubscribe,
        notifications_unsubscribe::notifications_unsubscribe_get,
        // User data
        user_data_export::user_data_export,
        user_data_import::user_data_import_preview,
        user_data_import::user_data_import,
        // Status
        status_get::status_get,
        status_set::status_set,
        // Search
        search::search,
        // Users
        security_users_get_self::security_users_get_self,
        security_users_get::security_users_get,
        security_users_get_by_email::security_users_get_by_email,
        security_users_signup::security_users_signup,
        security_users_email::security_users_email,
        security_users_remove::security_users_remove,
        security_users_remove::security_users_remove_by_id,
        security_users_clone::security_users_clone,
        security_subscription_update::security_subscription_update,
        // Scheduler
        scheduler_parse_schedule::scheduler_parse_schedule,
        // Messages
        send_message::send_message,
        // Certificate templates
        certificate_templates::certificate_templates_list,
        certificate_templates::certificate_templates_get,
        certificate_templates::certificate_templates_create,
        certificate_templates::certificate_templates_update,
        certificate_templates::certificate_templates_delete,
        certificate_templates::certificate_templates_generate,
        certificate_templates::certificate_templates_share,
        certificate_templates::certificate_templates_unshare,
        certificate_templates::certificates_fetch,
        // Private keys
        private_keys::private_keys_list,
        private_keys::private_keys_get,
        private_keys::private_keys_create,
        private_keys::private_keys_update,
        private_keys::private_keys_delete,
        private_keys::private_keys_export,
        // Content security policies
        content_security_policies::csp_list,
        content_security_policies::csp_get,
        content_security_policies::csp_create,
        content_security_policies::csp_update,
        content_security_policies::csp_delete,
        content_security_policies::csp_serialize,
        content_security_policies::csp_share,
        content_security_policies::csp_unshare,
        // Webhooks responders
        responders::responders_list,
        responders::responders_create,
        responders::responders_update,
        responders::responders_delete,
        responders::responders_get_history,
        responders::responders_clear_history,
        responders::responders_get_stats,
        // Page trackers
        page_trackers::page_trackers_list,
        page_trackers::page_trackers_create,
        page_trackers::page_trackers_update,
        page_trackers::page_trackers_delete,
        page_trackers::page_trackers_get_history,
        page_trackers::page_trackers_clear_history,
        page_trackers::page_trackers_get_logs,
        page_trackers::page_trackers_clear_logs,
        page_trackers::page_trackers_get_logs_summary,
        page_trackers::page_trackers_debug,
        // API trackers
        api_trackers::api_trackers_list,
        api_trackers::api_trackers_create,
        api_trackers::api_trackers_update,
        api_trackers::api_trackers_delete,
        api_trackers::api_trackers_get_history,
        api_trackers::api_trackers_clear_history,
        api_trackers::api_trackers_get_logs,
        api_trackers::api_trackers_clear_logs,
        api_trackers::api_trackers_get_logs_summary,
        api_trackers::api_trackers_test,
        api_trackers::api_trackers_debug,
    ),
    modifiers(&SecurityAddon),
    security(
        ("bearerAuth" = [])
    ),
    components(schemas(
        // API keys
        crate::users::UserApiKey,
        crate::users::ApiKeyCreateResponse,
        crate::users::ApiKeyCreateParams,
        crate::users::ApiKeyUpdateParams,
        crate::users::ApiKeyRegenerateParams,
        // Tags
        crate::users::UserTag,
        crate::users::EntityTag,
        crate::users::TagCreateParams,
        crate::users::TagUpdateParams,
        // Secrets
        crate::users::UserSecret,
        crate::users::SecretCreateParams,
        crate::users::SecretUpdateParams,
        // Scripts
        crate::users::UserScript,
        crate::users::UserScriptType,
        crate::users::ScriptContext,
        crate::users::ScriptCreateParams,
        crate::users::ScriptUpdateParams,
        // Settings
        crate::users::UserSettings,
        crate::users::UserSettingsSetter,
        crate::users::NotificationChannelKind,
        crate::users::UserNotificationDestination,
        crate::users::NotificationEmailSetParams,
        crate::users::NotificationEmailVerifyParams,
        notifications_unsubscribe::NotificationsUnsubscribeParams,
        // Status
        crate::server::DatabaseStatus,
        crate::server::Status,
        crate::server::StatusLevel,
        // Subscription
        crate::users::UserSubscription,
        crate::users::SubscriptionTier,
        // Identity
        crate::security::kratos::Identity,
        crate::security::kratos::IdentityTraits,
        crate::security::kratos::IdentityVerifiableAddress,
        crate::security::kratos::RecoveryLink,
        // Clone
        security_users_clone::CloneParams,
        security_users_clone::CloneSource,
        security_users_clone::CloneDestination,
        security_users_clone::CloneResponse,
        // Certificate templates
        crate::utils::certificates::CertificateTemplate,
        crate::utils::certificates::CertificateAttributes,
        crate::utils::certificates::TemplatesCreateParams,
        crate::utils::certificates::TemplatesUpdateParams,
        crate::utils::certificates::TemplatesGenerateParams,
        crate::utils::certificates::TemplatesFetchCertificatesParams,
        crate::utils::certificates::ExportFormat,
        crate::utils::certificates::PrivateKeyAlgorithm,
        crate::utils::certificates::PrivateKeySize,
        crate::utils::certificates::PrivateKeyEllipticCurve,
        crate::utils::certificates::SignatureAlgorithm,
        crate::utils::certificates::KeyUsage,
        crate::utils::certificates::ExtendedKeyUsage,
        crate::utils::certificates::Version,
        certificate_templates::CertificateTemplateGetResponse,
        // Private keys
        crate::utils::certificates::PrivateKey,
        crate::utils::certificates::PrivateKeysCreateParams,
        crate::utils::certificates::PrivateKeysUpdateParams,
        crate::utils::certificates::PrivateKeysExportParams,
        // Content security policies
        crate::utils::web_security::ContentSecurityPolicy,
        crate::utils::web_security::ContentSecurityPolicyDirective,
        crate::utils::web_security::ContentSecurityPolicySource,
        crate::utils::web_security::ContentSecurityPolicySandboxDirectiveValue,
        crate::utils::web_security::ContentSecurityPolicyWebrtcDirectiveValue,
        crate::utils::web_security::ContentSecurityPolicyTrustedTypesDirectiveValue,
        crate::utils::web_security::ContentSecurityPolicyRequireTrustedTypesForDirectiveValue,
        crate::utils::web_security::ContentSecurityPolicyContent,
        crate::utils::web_security::ContentSecurityPoliciesCreateParams,
        crate::utils::web_security::ContentSecurityPoliciesUpdateParams,
        crate::utils::web_security::ContentSecurityPoliciesSerializeParams,
        content_security_policies::ContentSecurityPolicyGetResponse,
        // Webhooks responders
        crate::utils::webhooks::Responder,
        crate::utils::webhooks::ResponderLocation,
        crate::utils::webhooks::ResponderMethod,
        crate::utils::webhooks::ResponderNotificationSettings,
        crate::utils::webhooks::ResponderPathType,
        crate::utils::webhooks::ResponderSettings,
        crate::utils::webhooks::ResponderStats,
        crate::utils::webhooks::RespondersCreateParams,
        crate::utils::webhooks::RespondersUpdateParams,
        crate::users::SecretsAccess,
        // Page trackers
        crate::utils::web_scraping::PageTracker,
        crate::utils::web_scraping::PageTrackerConfig,
        crate::utils::web_scraping::PageTrackerTarget,
        crate::utils::web_scraping::PageTrackerCreateParams,
        crate::utils::web_scraping::PageTrackerUpdateParams,
        crate::utils::web_scraping::PageTrackerDebugParams,
        crate::utils::web_scraping::PageTrackerGetHistoryParams,
        // API trackers
        crate::utils::web_scraping::ApiTracker,
        crate::utils::web_scraping::ApiTrackerConfig,
        crate::utils::web_scraping::ApiTrackerTarget,
        crate::utils::web_scraping::ApiTrackerCreateParams,
        crate::utils::web_scraping::ApiTrackerUpdateParams,
        crate::utils::web_scraping::ApiTrackerDebugParams,
        crate::utils::web_scraping::ApiTrackerGetHistoryParams,
        crate::utils::web_scraping::ApiTrackerTestParams,
        crate::utils::web_scraping::ApiTrackerTestResult,
        // Shared resources
        crate::users::ClientUserShare,
        crate::users::ClientSharedResource,
        // Handler-local types
        status_set::SetStatusAPIParams,
        search::SearchParams,
        scheduler_parse_schedule::SchedulerParseScheduleParams,
        scheduler_parse_schedule::SchedulerParseScheduleResult,
        send_message::SendMessageParams,
    ))
)]
pub(super) struct SecutilsOpenApi;

#[cfg(test)]
mod tests {
    use super::SecutilsOpenApi;
    use insta::{assert_json_snapshot, assert_snapshot};
    use utoipa::OpenApi;

    fn spec() -> serde_json::Value {
        serde_json::from_str(&SecutilsOpenApi::openapi().to_json().unwrap()).unwrap()
    }

    /// Renders a spec fragment as pretty JSON via `serde_json`. The workspace enables
    /// `serde_json/arbitrary_precision` (for precise round-tripping of user-supplied JSON
    /// numbers), under which `serde_json::Value`'s numbers carry an internal sentinel. Generic
    /// serializers (like insta's `assert_json_snapshot!`) leak it as
    /// `{"$serde_json::private::Number": "0"}`, whereas `serde_json`'s own serializer collapses it
    /// back to a bare number — exactly as the server serves the spec. Snapshot fragments that
    /// contain numbers (e.g. paginated paths with a `minimum: 0` constraint) via this helper so the
    /// snapshot matches the served bytes.
    fn spec_json(value: &serde_json::Value) -> String {
        serde_json::to_string_pretty(value).unwrap()
    }

    #[test]
    fn openapi_spec_has_correct_info() {
        let spec = spec();
        assert_json_snapshot!(spec["info"], {".version" => "[version]"}, @r###"
        {
          "title": "Secutils",
          "description": "An open-source, versatile, yet simple security toolbox for engineers and researchers.",
          "contact": {
            "name": "Aleh Zasypkin",
            "email": "dev@secutils.dev"
          },
          "license": {
            "name": "AGPL-3.0",
            "url": "https://github.com/secutils-dev/secutils/blob/main/LICENSE"
          },
          "version": "[version]"
        }
        "###);
    }

    #[test]
    fn openapi_spec_has_all_paths() {
        let spec = spec();
        let paths = spec["paths"].as_object().unwrap();
        let mut path_keys: Vec<&str> = paths.keys().map(|k| k.as_str()).collect();
        path_keys.sort();
        assert_json_snapshot!(path_keys, @r###"
        [
          "/api/certificates/_fetch",
          "/api/certificates/private_keys",
          "/api/certificates/private_keys/{key_id}",
          "/api/certificates/private_keys/{key_id}/_export",
          "/api/certificates/templates",
          "/api/certificates/templates/{template_id}",
          "/api/certificates/templates/{template_id}/_generate",
          "/api/certificates/templates/{template_id}/_share",
          "/api/certificates/templates/{template_id}/_unshare",
          "/api/notifications/unsubscribe",
          "/api/scheduler/parse_schedule",
          "/api/search",
          "/api/send_message",
          "/api/status",
          "/api/user/api_keys",
          "/api/user/api_keys/{api_key_id}",
          "/api/user/api_keys/{api_key_id}/_regenerate",
          "/api/user/data/_export",
          "/api/user/data/_import",
          "/api/user/data/_import_preview",
          "/api/user/notification_email",
          "/api/user/notification_email/_resend",
          "/api/user/notification_email/_verify",
          "/api/user/scripts",
          "/api/user/scripts/{script_id}",
          "/api/user/secrets",
          "/api/user/secrets/{secret_id}",
          "/api/user/settings",
          "/api/user/subscription",
          "/api/user/tags",
          "/api/user/tags/{tag_id}",
          "/api/users",
          "/api/users/_clone",
          "/api/users/email",
          "/api/users/remove",
          "/api/users/self",
          "/api/users/signup",
          "/api/users/{user_id}",
          "/api/users/{user_id}/api_keys",
          "/api/web_scraping/api_trackers",
          "/api/web_scraping/api_trackers/_debug",
          "/api/web_scraping/api_trackers/_logs_summary",
          "/api/web_scraping/api_trackers/_test",
          "/api/web_scraping/api_trackers/{tracker_id}",
          "/api/web_scraping/api_trackers/{tracker_id}/_clear",
          "/api/web_scraping/api_trackers/{tracker_id}/_clear_logs",
          "/api/web_scraping/api_trackers/{tracker_id}/_history",
          "/api/web_scraping/api_trackers/{tracker_id}/_logs",
          "/api/web_scraping/page_trackers",
          "/api/web_scraping/page_trackers/_debug",
          "/api/web_scraping/page_trackers/_logs_summary",
          "/api/web_scraping/page_trackers/{tracker_id}",
          "/api/web_scraping/page_trackers/{tracker_id}/_clear",
          "/api/web_scraping/page_trackers/{tracker_id}/_clear_logs",
          "/api/web_scraping/page_trackers/{tracker_id}/_history",
          "/api/web_scraping/page_trackers/{tracker_id}/_logs",
          "/api/web_security/csp",
          "/api/web_security/csp/{policy_id}",
          "/api/web_security/csp/{policy_id}/_serialize",
          "/api/web_security/csp/{policy_id}/_share",
          "/api/web_security/csp/{policy_id}/_unshare",
          "/api/webhooks/responders",
          "/api/webhooks/responders/_stats",
          "/api/webhooks/responders/{responder_id}",
          "/api/webhooks/responders/{responder_id}/_clear",
          "/api/webhooks/responders/{responder_id}/_history"
        ]
        "###);
    }

    #[test]
    fn openapi_spec_has_all_schemas() {
        let spec = spec();
        let schemas = spec["components"]["schemas"].as_object().unwrap();
        let mut schema_keys: Vec<&str> = schemas.keys().map(|k| k.as_str()).collect();
        schema_keys.sort();
        assert_json_snapshot!(schema_keys, @r###"
        [
          "ApiKeyCreateParams",
          "ApiKeyCreateResponse",
          "ApiKeyRegenerateParams",
          "ApiKeyUpdateParams",
          "ApiTarget",
          "ApiTracker",
          "ApiTrackerConfig",
          "ApiTrackerCreateParams",
          "ApiTrackerDebugParams",
          "ApiTrackerGetHistoryParams",
          "ApiTrackerTarget",
          "ApiTrackerTestParams",
          "ApiTrackerTestResult",
          "ApiTrackerUpdateParams",
          "ApplyDeletionSelections",
          "CertificateAttributes",
          "CertificateTemplate",
          "CertificateTemplateGetResponse",
          "ClientSharedResource",
          "ClientUserShare",
          "CloneDestination",
          "CloneParams",
          "CloneResponse",
          "CloneSource",
          "ConflictResolution",
          "ContentSecurityPoliciesCreateParams",
          "ContentSecurityPoliciesSerializeParams",
          "ContentSecurityPoliciesUpdateParams",
          "ContentSecurityPolicy",
          "ContentSecurityPolicyContent",
          "ContentSecurityPolicyDirective",
          "ContentSecurityPolicyGetResponse",
          "ContentSecurityPolicyRequireTrustedTypesForDirectiveValue",
          "ContentSecurityPolicySandboxDirectiveValue",
          "ContentSecurityPolicySource",
          "ContentSecurityPolicyTrustedTypesDirectiveValue",
          "ContentSecurityPolicyWebrtcDirectiveValue",
          "DataFileSecret",
          "DatabaseStatus",
          "EmailParams",
          "EntityTag",
          "ExportFormat",
          "ExportSelection",
          "ExportTrackableSelection",
          "ExportedPrivateKey",
          "ExportedResponder",
          "ExportedResponderRequest",
          "ExportedRetrackData",
          "ExportedTracker",
          "ExtendedKeyUsage",
          "ExtractorEngine",
          "Identity",
          "IdentityTraits",
          "IdentityVerifiableAddress",
          "ImportAction",
          "ImportEntitySelection",
          "ImportMode",
          "ImportSelections",
          "ImportedScript",
          "KdfParams",
          "KeyUsage",
          "NotificationChannelKind",
          "NotificationEmailSetParams",
          "NotificationEmailVerifyParams",
          "NotificationsUnsubscribeParams",
          "PageTarget",
          "PageTracker",
          "PageTrackerConfig",
          "PageTrackerCreateParams",
          "PageTrackerDebugParams",
          "PageTrackerGetHistoryParams",
          "PageTrackerTarget",
          "PageTrackerUpdateParams",
          "Page_ApiTracker",
          "Page_CertificateTemplate",
          "Page_ContentSecurityPolicy",
          "Page_PageTracker",
          "Page_PrivateKey",
          "Page_Responder",
          "Page_UserScript",
          "Page_UserSecret",
          "Page_UserTag",
          "PrivateKey",
          "PrivateKeyAlgorithm",
          "PrivateKeyEllipticCurve",
          "PrivateKeySize",
          "PrivateKeysCreateParams",
          "PrivateKeysExportParams",
          "PrivateKeysUpdateParams",
          "RecoveryLink",
          "RemoveParams",
          "Responder",
          "ResponderLocation",
          "ResponderMethod",
          "ResponderNotificationSettings",
          "ResponderPathType",
          "ResponderSettings",
          "ResponderStats",
          "RespondersCreateParams",
          "RespondersUpdateParams",
          "RetrackTracker",
          "RetrackTrackerValue",
          "SchedulerJobConfig",
          "SchedulerJobRetryStrategy",
          "SchedulerParseScheduleParams",
          "SchedulerParseScheduleResult",
          "ScriptContext",
          "ScriptCreateParams",
          "ScriptUpdateParams",
          "SearchParams",
          "SecretCreateParams",
          "SecretUpdateParams",
          "SecretsAccess",
          "SecretsEncryptionMeta",
          "SendMessageParams",
          "SetStatusAPIParams",
          "SignatureAlgorithm",
          "SignupParams",
          "Status",
          "StatusLevel",
          "SubscriptionTier",
          "TagCreateParams",
          "TagUpdateParams",
          "TargetRequest",
          "TemplatesCreateParams",
          "TemplatesFetchCertificatesParams",
          "TemplatesGenerateParams",
          "TemplatesUpdateParams",
          "TrackerConfig",
          "TrackerDataRevision",
          "TrackerDataValue_Value",
          "TrackerTarget",
          "UpdateSubscriptionParams",
          "UserApiKey",
          "UserDataExportInclude",
          "UserDataExportParams",
          "UserDataImportFile",
          "UserDataImportFileData",
          "UserDataImportParams",
          "UserDataImportPreviewParams",
          "UserNotificationDestination",
          "UserScript",
          "UserScriptType",
          "UserSecret",
          "UserSettings",
          "UserSettingsSetter",
          "UserSubscription",
          "UserTag",
          "Version"
        ]
        "###);
    }

    #[test]
    fn openapi_spec_tags_operations() {
        let spec = spec();
        let tags_path = &spec["paths"]["/api/user/tags"];
        assert_snapshot!(spec_json(tags_path), @r###"
        {
          "get": {
            "tags": [
              "tags"
            ],
            "summary": "Lists tags for the authenticated user (paginated).",
            "operationId": "user_tags_list",
            "parameters": [
              {
                "name": "page",
                "in": "query",
                "description": "Zero-based page index. Defaults to `0`.",
                "required": false,
                "schema": {
                  "type": "integer",
                  "format": "int32",
                  "minimum": 0
                }
              },
              {
                "name": "pageSize",
                "in": "query",
                "description": "Number of items per page. Defaults to 15, clamped to a maximum of 100.",
                "required": false,
                "schema": {
                  "type": "integer",
                  "format": "int32",
                  "minimum": 0
                }
              },
              {
                "name": "sort",
                "in": "query",
                "description": "Field to sort by. Entity-specific; falls back to the entity default when not in the\nallowlist.",
                "required": false,
                "schema": {
                  "type": "string"
                }
              },
              {
                "name": "order",
                "in": "query",
                "description": "Sort direction (`asc` or `desc`).",
                "required": false,
                "schema": {
                  "$ref": "#/components/schemas/SortOrder"
                }
              },
              {
                "name": "q",
                "in": "query",
                "description": "Free-text query matched (case-insensitively) against the entity name, or matched verbatim\nagainst the entity id (used by \"filter to a single entity\" workspace links that navigate to\n`?q=<entity-id>`).",
                "required": false,
                "schema": {
                  "type": "string"
                }
              },
              {
                "name": "tags",
                "in": "query",
                "description": "Page-level tag filter (OR): a comma-separated list of tag IDs, items having ANY of these\ntags are returned.",
                "required": false,
                "schema": {
                  "type": "string"
                }
              },
              {
                "name": "globalTags",
                "in": "query",
                "description": "Global-scope tag filter (AND): a comma-separated list of tag IDs, only items having ALL of\nthese tags are returned.",
                "required": false,
                "schema": {
                  "type": "string"
                }
              }
            ],
            "responses": {
              "200": {
                "description": "Paginated list of user tags.",
                "content": {
                  "application/json": {
                    "schema": {
                      "$ref": "#/components/schemas/Page_UserTag"
                    }
                  }
                }
              },
              "401": {
                "description": "Missing or invalid authentication credentials."
              }
            }
          },
          "post": {
            "tags": [
              "tags"
            ],
            "summary": "Creates a new tag.",
            "operationId": "user_tags_create",
            "requestBody": {
              "content": {
                "application/json": {
                  "schema": {
                    "$ref": "#/components/schemas/TagCreateParams"
                  }
                }
              },
              "required": true
            },
            "responses": {
              "201": {
                "description": "Tag was successfully created.",
                "content": {
                  "application/json": {
                    "schema": {
                      "$ref": "#/components/schemas/UserTag"
                    }
                  }
                }
              },
              "400": {
                "description": "Invalid tag parameters."
              },
              "401": {
                "description": "Missing or invalid authentication credentials."
              }
            }
          }
        }
        "###);
    }

    #[test]
    fn openapi_spec_user_tag_schema() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["UserTag"], @r###"
        {
          "type": "object",
          "description": "A user-managed tag with a name and display color.",
          "required": [
            "id",
            "name",
            "color",
            "createdAt",
            "updatedAt"
          ],
          "properties": {
            "color": {
              "type": "string"
            },
            "createdAt": {
              "type": "integer",
              "format": "int64"
            },
            "id": {
              "type": "string",
              "format": "uuid"
            },
            "name": {
              "type": "string"
            },
            "updatedAt": {
              "type": "integer",
              "format": "int64"
            }
          }
        }
        "###);
    }

    #[test]
    fn openapi_spec_user_secret_schema_excludes_internal_fields() {
        let spec = spec();
        let schema = &spec["components"]["schemas"]["UserSecret"];
        // user_id and encrypted_value must not appear (they have #[serde(skip)]).
        let props = schema["properties"].as_object().unwrap();
        assert!(!props.contains_key("userId"), "userId should be excluded");
        assert!(
            !props.contains_key("encryptedValue"),
            "encryptedValue should be excluded"
        );
        assert!(props.contains_key("id"));
        assert!(props.contains_key("name"));
        assert!(props.contains_key("createdAt"));
        assert!(props.contains_key("updatedAt"));
    }

    #[test]
    fn openapi_spec_tag_create_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["TagCreateParams"]["example"], @r###"
        {
          "name": "production",
          "color": "primary"
        }
        "###);
    }

    #[test]
    fn openapi_spec_secret_create_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["SecretCreateParams"]["example"], @r###"
        {
          "name": "GITHUB_TOKEN",
          "value": "ghp_xxxxxxxxxxxx",
          "tagIds": []
        }
        "###);
    }

    #[test]
    fn openapi_spec_script_create_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["ScriptCreateParams"]["example"], @r###"
        {
          "name": "my-extractor",
          "scriptType": "api_extractor",
          "content": "export default async function() { return document.title; }",
          "tagIds": []
        }
        "###);
    }

    #[test]
    fn openapi_spec_status_path() {
        let spec = spec();
        assert_json_snapshot!(spec["paths"]["/api/status"], @r###"
        {
          "get": {
            "tags": [
              "status"
            ],
            "summary": "Returns the current server status.",
            "operationId": "status_get",
            "responses": {
              "200": {
                "description": "Current server status.",
                "content": {
                  "application/json": {
                    "schema": {
                      "$ref": "#/components/schemas/Status"
                    }
                  }
                }
              }
            },
            "security": [
              {}
            ]
          },
          "post": {
            "tags": [
              "status"
            ],
            "summary": "Sets the server status level (operator-only).",
            "operationId": "status_set",
            "requestBody": {
              "content": {
                "application/json": {
                  "schema": {
                    "$ref": "#/components/schemas/SetStatusAPIParams"
                  }
                }
              },
              "required": true
            },
            "responses": {
              "204": {
                "description": "Status was successfully updated."
              },
              "401": {
                "description": "Missing or invalid authentication credentials."
              },
              "403": {
                "description": "Caller is not an operator."
              },
              "500": {
                "description": "Failed to update status."
              }
            }
          }
        }
        "###);
    }

    #[test]
    fn openapi_spec_certificate_templates_crud_operations() {
        let spec = spec();
        let path = &spec["paths"]["/api/certificates/templates"];
        assert_snapshot!(spec_json(path), @r###"
        {
          "get": {
            "tags": [
              "certificates"
            ],
            "summary": "Lists certificate templates for the authenticated user (paginated).",
            "operationId": "certificate_templates_list",
            "parameters": [
              {
                "name": "page",
                "in": "query",
                "description": "Zero-based page index. Defaults to `0`.",
                "required": false,
                "schema": {
                  "type": "integer",
                  "format": "int32",
                  "minimum": 0
                }
              },
              {
                "name": "pageSize",
                "in": "query",
                "description": "Number of items per page. Defaults to 15, clamped to a maximum of 100.",
                "required": false,
                "schema": {
                  "type": "integer",
                  "format": "int32",
                  "minimum": 0
                }
              },
              {
                "name": "sort",
                "in": "query",
                "description": "Field to sort by. Entity-specific; falls back to the entity default when not in the\nallowlist.",
                "required": false,
                "schema": {
                  "type": "string"
                }
              },
              {
                "name": "order",
                "in": "query",
                "description": "Sort direction (`asc` or `desc`).",
                "required": false,
                "schema": {
                  "$ref": "#/components/schemas/SortOrder"
                }
              },
              {
                "name": "q",
                "in": "query",
                "description": "Free-text query matched (case-insensitively) against the entity name, or matched verbatim\nagainst the entity id (used by \"filter to a single entity\" workspace links that navigate to\n`?q=<entity-id>`).",
                "required": false,
                "schema": {
                  "type": "string"
                }
              },
              {
                "name": "tags",
                "in": "query",
                "description": "Page-level tag filter (OR): a comma-separated list of tag IDs, items having ANY of these\ntags are returned.",
                "required": false,
                "schema": {
                  "type": "string"
                }
              },
              {
                "name": "globalTags",
                "in": "query",
                "description": "Global-scope tag filter (AND): a comma-separated list of tag IDs, only items having ALL of\nthese tags are returned.",
                "required": false,
                "schema": {
                  "type": "string"
                }
              }
            ],
            "responses": {
              "200": {
                "description": "Paginated list of certificate templates.",
                "content": {
                  "application/json": {
                    "schema": {
                      "$ref": "#/components/schemas/Page_CertificateTemplate"
                    }
                  }
                }
              },
              "401": {
                "description": "Missing or invalid authentication credentials."
              }
            }
          },
          "post": {
            "tags": [
              "certificates"
            ],
            "summary": "Creates a new certificate template.",
            "operationId": "certificate_templates_create",
            "requestBody": {
              "content": {
                "application/json": {
                  "schema": {
                    "$ref": "#/components/schemas/TemplatesCreateParams"
                  }
                }
              },
              "required": true
            },
            "responses": {
              "201": {
                "description": "Template was successfully created.",
                "content": {
                  "application/json": {
                    "schema": {
                      "$ref": "#/components/schemas/CertificateTemplate"
                    }
                  }
                }
              },
              "400": {
                "description": "Invalid template parameters."
              },
              "401": {
                "description": "Missing or invalid authentication credentials."
              }
            }
          }
        }
        "###);
    }

    #[test]
    fn openapi_spec_certificate_templates_action_operations() {
        let spec = spec();

        // _generate
        let generate =
            &spec["paths"]["/api/certificates/templates/{template_id}/_generate"]["post"];
        assert_eq!(generate["operationId"], "certificate_templates_generate");
        assert_eq!(
            generate["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/TemplatesGenerateParams"
        );

        // _share
        let share = &spec["paths"]["/api/certificates/templates/{template_id}/_share"]["post"];
        assert_eq!(share["operationId"], "certificate_templates_share");

        // _unshare
        let unshare = &spec["paths"]["/api/certificates/templates/{template_id}/_unshare"]["post"];
        assert_eq!(unshare["operationId"], "certificate_templates_unshare");

        // _fetch
        let fetch = &spec["paths"]["/api/certificates/_fetch"]["post"];
        assert_eq!(fetch["operationId"], "certificates_fetch");
        assert_eq!(
            fetch["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/TemplatesFetchCertificatesParams"
        );
    }

    #[test]
    fn openapi_spec_certificate_template_schema() {
        let spec = spec();
        let schema = &spec["components"]["schemas"]["CertificateTemplate"];
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("id"));
        assert!(props.contains_key("name"));
        assert!(props.contains_key("attributes"));
        assert!(props.contains_key("createdAt"));
        assert!(props.contains_key("updatedAt"));
        // Timestamps should be integers
        assert_eq!(props["createdAt"]["type"], "integer");
        assert_eq!(props["updatedAt"]["type"], "integer");
    }

    #[test]
    fn openapi_spec_certificate_create_params_has_example() {
        let spec = spec();
        let example = &spec["components"]["schemas"]["TemplatesCreateParams"]["example"];
        assert!(example["templateName"].is_string());
        assert!(example["attributes"]["keyAlgorithm"].is_object());
        assert!(example["attributes"]["isCa"].is_boolean());
    }

    #[test]
    fn openapi_spec_fetch_certificates_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["TemplatesFetchCertificatesParams"]["example"], @r###"
        {
          "url": "https://example.com"
        }
        "###);
    }

    #[test]
    fn openapi_spec_private_keys_crud_operations() {
        let spec = spec();
        let path = &spec["paths"]["/api/certificates/private_keys"];

        // GET (list)
        assert_eq!(path["get"]["operationId"], "private_keys_list");
        assert_eq!(path["get"]["tags"][0], "certificates");

        // POST (create)
        assert_eq!(path["post"]["operationId"], "private_keys_create");
        assert_eq!(
            path["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/PrivateKeysCreateParams"
        );
    }

    #[test]
    fn openapi_spec_private_keys_export_operation() {
        let spec = spec();
        let export = &spec["paths"]["/api/certificates/private_keys/{key_id}/_export"]["post"];
        assert_eq!(export["operationId"], "private_keys_export");
        assert_eq!(
            export["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/PrivateKeysExportParams"
        );
    }

    #[test]
    fn openapi_spec_private_key_schema() {
        let spec = spec();
        let schema = &spec["components"]["schemas"]["PrivateKey"];
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("id"));
        assert!(props.contains_key("name"));
        assert!(props.contains_key("alg"));
        assert!(props.contains_key("pkcs8"));
        assert!(props.contains_key("encrypted"));
        assert!(props.contains_key("createdAt"));
        assert!(props.contains_key("updatedAt"));
        assert_eq!(props["createdAt"]["type"], "integer");
        assert_eq!(props["updatedAt"]["type"], "integer");
    }

    #[test]
    fn openapi_spec_private_keys_create_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["PrivateKeysCreateParams"]["example"], @r###"
        {
          "keyName": "my-key",
          "alg": {
            "keyType": "ed25519"
          },
          "tagIds": []
        }
        "###);
    }

    #[test]
    fn openapi_spec_private_keys_export_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["PrivateKeysExportParams"]["example"], @r###"
        {
          "format": "pem"
        }
        "###);
    }

    #[test]
    fn openapi_spec_csp_crud_operations() {
        let spec = spec();
        let path = &spec["paths"]["/api/web_security/csp"];

        // GET (list)
        assert_eq!(path["get"]["operationId"], "csp_list");
        assert_eq!(path["get"]["tags"][0], "web_security");

        // POST (create)
        assert_eq!(path["post"]["operationId"], "csp_create");
        assert_eq!(
            path["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ContentSecurityPoliciesCreateParams"
        );
    }

    #[test]
    fn openapi_spec_csp_action_operations() {
        let spec = spec();

        // _serialize
        let serialize = &spec["paths"]["/api/web_security/csp/{policy_id}/_serialize"]["post"];
        assert_eq!(serialize["operationId"], "csp_serialize");
        assert_eq!(
            serialize["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ContentSecurityPoliciesSerializeParams"
        );

        // _share
        let share = &spec["paths"]["/api/web_security/csp/{policy_id}/_share"]["post"];
        assert_eq!(share["operationId"], "csp_share");

        // _unshare
        let unshare = &spec["paths"]["/api/web_security/csp/{policy_id}/_unshare"]["post"];
        assert_eq!(unshare["operationId"], "csp_unshare");
    }

    #[test]
    fn openapi_spec_csp_schema() {
        let spec = spec();
        let schema = &spec["components"]["schemas"]["ContentSecurityPolicy"];
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("id"));
        assert!(props.contains_key("name"));
        assert!(props.contains_key("directives"));
        assert!(props.contains_key("createdAt"));
        assert!(props.contains_key("updatedAt"));
        assert_eq!(props["createdAt"]["type"], "integer");
        assert_eq!(props["updatedAt"]["type"], "integer");
    }

    #[test]
    fn openapi_spec_csp_create_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["ContentSecurityPoliciesCreateParams"]["example"], @r###"
        {
          "name": "my-csp",
          "content": {
            "type": "serialized",
            "value": "default-src 'self'"
          },
          "tagIds": []
        }
        "###);
    }

    #[test]
    fn openapi_spec_csp_serialize_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["ContentSecurityPoliciesSerializeParams"]["example"], @r###"
        {
          "source": "enforcingHeader"
        }
        "###);
    }

    #[test]
    fn openapi_spec_responders_crud_operations() {
        let spec = spec();
        let path = &spec["paths"]["/api/webhooks/responders"];

        // GET (list)
        assert_eq!(path["get"]["operationId"], "responders_list");
        assert_eq!(path["get"]["tags"][0], "webhooks");

        // POST (create)
        assert_eq!(path["post"]["operationId"], "responders_create");
        assert_eq!(
            path["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/RespondersCreateParams"
        );
    }

    #[test]
    fn openapi_spec_responders_action_operations() {
        let spec = spec();

        // _history
        let history = &spec["paths"]["/api/webhooks/responders/{responder_id}/_history"]["get"];
        assert_eq!(history["operationId"], "responders_get_history");

        // _clear
        let clear = &spec["paths"]["/api/webhooks/responders/{responder_id}/_clear"]["post"];
        assert_eq!(clear["operationId"], "responders_clear_history");

        // _stats
        let stats = &spec["paths"]["/api/webhooks/responders/_stats"]["get"];
        assert_eq!(stats["operationId"], "responders_get_stats");
    }

    #[test]
    fn openapi_spec_responder_schema() {
        let spec = spec();
        let schema = &spec["components"]["schemas"]["Responder"];
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("id"));
        assert!(props.contains_key("name"));
        assert!(props.contains_key("location"));
        assert!(props.contains_key("method"));
        assert!(props.contains_key("enabled"));
        assert!(props.contains_key("settings"));
        assert!(props.contains_key("createdAt"));
        assert_eq!(props["createdAt"]["type"], "integer");
    }

    #[test]
    fn openapi_spec_responders_create_params_has_example() {
        let spec = spec();
        let example = &spec["components"]["schemas"]["RespondersCreateParams"]["example"];
        assert_eq!(example["name"], "my-responder");
        assert_eq!(example["method"], "ANY");
        assert_eq!(example["enabled"], true);
        assert_eq!(example["location"]["pathType"], "=");
        assert_eq!(example["location"]["path"], "/my-hook");
        assert!(
            example["settings"]["requestsToTrack"].is_number()
                || example["settings"]["requestsToTrack"].is_object()
        );
        assert!(example["tagIds"].is_array());
    }

    #[test]
    fn openapi_spec_responders_update_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["RespondersUpdateParams"]["example"], @r###"
        {
          "name": "renamed-responder"
        }
        "###);
    }

    #[test]
    fn openapi_spec_has_security_schemes() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["securitySchemes"], @r###"
        {
          "bearerAuth": {
            "type": "http",
            "scheme": "bearer",
            "bearerFormat": "JWT",
            "description": "JWT token obtained from the authentication service. Pass as `Authorization: Bearer <token>`."
          }
        }
        "###);
    }

    #[test]
    fn openapi_spec_has_global_security() {
        let spec = spec();
        assert_json_snapshot!(spec["security"], @r###"
        [
          {
            "bearerAuth": []
          }
        ]
        "###);
    }

    #[test]
    fn openapi_spec_anonymous_endpoint_overrides_security() {
        let spec = spec();
        let status_get = &spec["paths"]["/api/status"]["get"];
        assert_json_snapshot!(status_get["security"], @r###"
        [
          {}
        ]
        "###);
    }

    #[test]
    fn openapi_spec_optional_auth_endpoint_has_both_options() {
        let spec = spec();
        let template_get = &spec["paths"]["/api/certificates/templates/{template_id}"]["get"];
        assert_json_snapshot!(template_get["security"], @r###"
        [
          {},
          {
            "bearerAuth": []
          }
        ]
        "###);
    }

    #[test]
    fn openapi_spec_has_external_docs() {
        let spec = spec();
        assert_eq!(spec["externalDocs"]["url"], "https://secutils.dev/docs");
        assert_eq!(
            spec["externalDocs"]["description"],
            "Secutils.dev documentation"
        );
    }

    #[test]
    fn openapi_spec_has_tag_descriptions() {
        let spec = spec();
        let tags = spec["tags"].as_array().unwrap();
        let tag_map: std::collections::HashMap<&str, &str> = tags
            .iter()
            .map(|t| {
                (
                    t["name"].as_str().unwrap(),
                    t["description"].as_str().unwrap(),
                )
            })
            .collect();

        assert_eq!(
            tag_map["webhooks"],
            "Create HTTP responders that capture and replay incoming requests."
        );
        assert_eq!(
            tag_map["certificates"],
            "Generate X.509 certificate templates and manage private keys."
        );
        assert_eq!(
            tag_map["web_security"],
            "Build, parse, and serialize Content Security Policy headers."
        );
        assert_eq!(tag_map["tags"], "Organize resources with colored tags.");
        assert_eq!(
            tag_map["secrets"],
            "Store encrypted secrets for use in responder and tracker scripts."
        );
        assert_eq!(
            tag_map["scripts"],
            "Manage reusable JavaScript scripts for responders and trackers."
        );
        assert_eq!(
            tag_map["api_keys"],
            "Create and manage API keys for programmatic access."
        );
    }

    #[test]
    fn openapi_spec_api_keys_crud_operations() {
        let spec = spec();
        let path = &spec["paths"]["/api/user/api_keys"];

        // GET (list)
        assert_eq!(path["get"]["operationId"], "user_api_keys_list");
        assert_eq!(path["get"]["tags"][0], "api_keys");
        assert!(path["get"]["responses"]["200"]["content"]["application/json"]["schema"]["items"]
            ["$ref"]
            .as_str()
            .unwrap()
            .ends_with("/UserApiKey"));

        // POST (create)
        assert_eq!(path["post"]["operationId"], "user_api_keys_create");
        assert_eq!(
            path["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiKeyCreateParams"
        );
        assert!(
            path["post"]["responses"]["201"]["content"]["application/json"]["schema"]["$ref"]
                .as_str()
                .unwrap()
                .ends_with("/ApiKeyCreateResponse")
        );
    }

    #[test]
    fn openapi_spec_api_keys_update_delete_operations() {
        let spec = spec();
        let path = &spec["paths"]["/api/user/api_keys/{api_key_id}"];

        // PUT (update)
        assert_eq!(path["put"]["operationId"], "user_api_keys_update");
        assert_eq!(
            path["put"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiKeyUpdateParams"
        );
        assert!(
            path["put"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
                .as_str()
                .unwrap()
                .ends_with("/UserApiKey")
        );

        // DELETE
        assert_eq!(path["delete"]["operationId"], "user_api_keys_delete");
        assert!(path["delete"]["responses"]["204"].is_object());
    }

    #[test]
    fn openapi_spec_api_keys_regenerate_operation() {
        let spec = spec();
        let regenerate = &spec["paths"]["/api/user/api_keys/{api_key_id}/_regenerate"]["post"];
        assert_eq!(regenerate["operationId"], "user_api_keys_regenerate");
        assert_eq!(regenerate["tags"][0], "api_keys");
        assert_eq!(
            regenerate["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiKeyRegenerateParams"
        );
        assert!(
            regenerate["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
                .as_str()
                .unwrap()
                .ends_with("/ApiKeyCreateResponse")
        );
    }

    #[test]
    fn openapi_spec_api_keys_operator_provisioning() {
        let spec = spec();
        let provision = &spec["paths"]["/api/users/{user_id}/api_keys"]["post"];
        assert_eq!(provision["operationId"], "user_api_keys_create_for_user");
        assert_eq!(provision["tags"][0], "api_keys");
        assert_eq!(
            provision["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiKeyCreateParams"
        );
        assert!(
            provision["responses"]["201"]["content"]["application/json"]["schema"]["$ref"]
                .as_str()
                .unwrap()
                .ends_with("/ApiKeyCreateResponse")
        );
    }

    #[test]
    fn openapi_spec_user_api_key_schema() {
        let spec = spec();
        let schema = &spec["components"]["schemas"]["UserApiKey"];
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("id"));
        assert!(props.contains_key("name"));
        assert!(props.contains_key("createdAt"));
        assert!(props.contains_key("updatedAt"));
        assert!(props.contains_key("expiresAt"));
        assert!(props.contains_key("lastUsedAt"));
        // Internal fields must not appear.
        assert!(!props.contains_key("userId"), "userId should be excluded");
        assert!(
            !props.contains_key("tokenHash"),
            "tokenHash should be excluded"
        );
        // Timestamps are integers.
        assert_eq!(props["createdAt"]["type"], "integer");
        assert_eq!(props["updatedAt"]["type"], "integer");
    }

    #[test]
    fn openapi_spec_api_key_create_params_has_example() {
        let spec = spec();
        let example = &spec["components"]["schemas"]["ApiKeyCreateParams"]["example"];
        assert!(example["name"].is_string());
        assert!(example["expiresAt"].is_number());
    }

    #[test]
    fn openapi_spec_api_key_update_params_has_example() {
        let spec = spec();
        assert_json_snapshot!(spec["components"]["schemas"]["ApiKeyUpdateParams"]["example"], @r###"
        {
          "name": "Production agent key"
        }
        "###);
    }

    #[test]
    fn openapi_spec_api_key_regenerate_params_has_example() {
        let spec = spec();
        let example = &spec["components"]["schemas"]["ApiKeyRegenerateParams"]["example"];
        assert!(example["expiresAt"].is_number());
    }
}
