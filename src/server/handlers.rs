pub mod certificate_templates;
mod home_summary_get;
pub mod scheduler_parse_schedule;
pub mod search;
pub mod security_subscription_update;
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
pub mod user_data_export;
pub mod user_data_import;
pub mod user_scripts;
pub mod user_secrets;
pub mod user_settings_get;
pub mod user_settings_set;
pub mod user_tags;
mod utils_action;
mod webhooks_responders;
mod webhooks_retrack;

pub use self::{
    home_summary_get::home_summary_get, ui_state_get::ui_state_get, utils_action::utils_action,
    webhooks_responders::webhooks_responders, webhooks_retrack::webhooks_retrack,
};

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Secutils",
        license(
            name = "AGPL-3.0",
            url = "https://github.com/secutils-dev/secutils/blob/main/LICENSE"
        )
    ),
    paths(
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
    ),
    components(schemas(
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
        // Status
        crate::server::Status,
        crate::server::StatusLevel,
        // Subscription
        crate::users::UserSubscription,
        crate::users::SubscriptionTier,
        // Identity
        crate::security::kratos::Identity,
        crate::security::kratos::IdentityTraits,
        crate::security::kratos::IdentityVerifiableAddress,
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
    use insta::assert_json_snapshot;
    use utoipa::OpenApi;

    fn spec() -> serde_json::Value {
        serde_json::from_str(&SecutilsOpenApi::openapi().to_json().unwrap()).unwrap()
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
          "/api/certificates/templates",
          "/api/certificates/templates/{template_id}",
          "/api/certificates/templates/{template_id}/_generate",
          "/api/certificates/templates/{template_id}/_share",
          "/api/certificates/templates/{template_id}/_unshare",
          "/api/scheduler/parse_schedule",
          "/api/search",
          "/api/send_message",
          "/api/status",
          "/api/user/data/_export",
          "/api/user/data/_import",
          "/api/user/data/_import_preview",
          "/api/user/scripts",
          "/api/user/scripts/{script_id}",
          "/api/user/secrets",
          "/api/user/secrets/{secret_id}",
          "/api/user/settings",
          "/api/user/subscription",
          "/api/user/tags",
          "/api/user/tags/{tag_id}",
          "/api/users",
          "/api/users/email",
          "/api/users/remove",
          "/api/users/self",
          "/api/users/signup",
          "/api/users/{user_id}"
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
          "ApplyDeletionSelections",
          "CertificateAttributes",
          "CertificateTemplate",
          "CertificateTemplateGetResponse",
          "ClientSharedResource",
          "ClientUserShare",
          "ConflictResolution",
          "EmailParams",
          "EntityTag",
          "ExportFormat",
          "ExportSelection",
          "ExportTrackableSelection",
          "ExtendedKeyUsage",
          "Identity",
          "IdentityTraits",
          "IdentityVerifiableAddress",
          "ImportAction",
          "ImportEntitySelection",
          "ImportMode",
          "ImportSelections",
          "KeyUsage",
          "PrivateKeyAlgorithm",
          "PrivateKeyEllipticCurve",
          "PrivateKeySize",
          "RemoveParams",
          "SchedulerParseScheduleParams",
          "SchedulerParseScheduleResult",
          "ScriptContext",
          "ScriptCreateParams",
          "ScriptUpdateParams",
          "SearchParams",
          "SecretCreateParams",
          "SecretUpdateParams",
          "SendMessageParams",
          "SetStatusAPIParams",
          "SignatureAlgorithm",
          "SignupParams",
          "Status",
          "StatusLevel",
          "SubscriptionTier",
          "TagCreateParams",
          "TagUpdateParams",
          "TemplatesCreateParams",
          "TemplatesFetchCertificatesParams",
          "TemplatesGenerateParams",
          "TemplatesUpdateParams",
          "UpdateSubscriptionParams",
          "UserDataExportInclude",
          "UserDataExportParams",
          "UserDataImportParams",
          "UserDataImportPreviewParams",
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
        assert_json_snapshot!(tags_path, @r###"
        {
          "get": {
            "tags": [
              "tags"
            ],
            "summary": "Lists all tags for the authenticated user.",
            "operationId": "user_tags_list",
            "responses": {
              "200": {
                "description": "List of user tags.",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": {
                        "$ref": "#/components/schemas/UserTag"
                      }
                    }
                  }
                }
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
            }
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
        assert_json_snapshot!(path, @r###"
        {
          "get": {
            "tags": [
              "certificates"
            ],
            "summary": "Lists all certificate templates for the authenticated user.",
            "operationId": "certificate_templates_list",
            "responses": {
              "200": {
                "description": "List of certificate templates.",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": {
                        "$ref": "#/components/schemas/CertificateTemplate"
                      }
                    }
                  }
                }
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
}
