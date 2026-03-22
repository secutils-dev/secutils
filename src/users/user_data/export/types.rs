use crate::{
    retrack::RetrackTracker,
    users::{
        SecretsAccess, UserSettings, scripts::UserScript, secrets::SecretsEncryptionMeta,
        user_data::shared::DataFileSecret,
    },
    utils::{
        certificates::{CertificateTemplate, PrivateKey, PrivateKeyAlgorithm},
        web_scraping::{ApiTracker, PageTracker},
        web_security::ContentSecurityPolicy,
        webhooks::{Responder, ResponderRequest},
    },
};
use retrack_types::trackers::{TrackerConfig, TrackerDataRevision, TrackerTarget};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use time::OffsetDateTime;
use uuid::Uuid;

/// The current export format version.
pub use crate::users::user_data::shared::DATA_FILE_VERSION as EXPORT_VERSION;

/// The top-level export file structure.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataExport {
    pub version: u32,
    #[serde(with = "time::serde::timestamp")]
    pub exported_at: OffsetDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets_encryption: Option<SecretsEncryptionMeta>,
    pub data: UserDataExportData,
}

/// The data section of the export file containing all exported entities.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataExportData {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<UserScript>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<DataFileSecret>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub responders: Vec<ExportedResponder>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub certificate_templates: Vec<CertificateTemplate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub private_keys: Vec<ExportedPrivateKey>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub content_security_policies: Vec<ContentSecurityPolicy>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub page_trackers: Vec<ExportedTracker>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub api_trackers: Vec<ExportedTracker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<UserSettings>,
}

/// An exported responder with optional history.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedResponder {
    #[serde(flatten)]
    pub responder: Responder,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<ExportedResponderRequest>,
}

/// An exported responder request (history entry).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedResponderRequest {
    pub id: Uuid,
    pub responder_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_address: Option<String>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<JsonValue>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
}

impl<'a> From<ResponderRequest<'a>> for ExportedResponderRequest {
    fn from(req: ResponderRequest<'a>) -> Self {
        Self {
            id: req.id,
            responder_id: req.responder_id,
            client_address: req.client_address.map(|a| a.to_string()),
            method: req.method.into_owned(),
            headers: req.headers.map(|h| {
                JsonValue::Object(
                    h.into_iter()
                        .map(|(k, v)| {
                            (
                                k.into_owned(),
                                JsonValue::String(String::from_utf8_lossy(&v).into_owned()),
                            )
                        })
                        .collect(),
                )
            }),
            url: req.url.into_owned(),
            body: req.body.map(|b| String::from_utf8_lossy(&b).into_owned()),
            created_at: req.created_at,
            duration_ms: req.duration_ms,
            response_status_code: req.response_status_code,
            response_headers: req.response_headers.map(|h| {
                JsonValue::Object(
                    h.into_iter()
                        .map(|(k, v)| {
                            (
                                k.into_owned(),
                                JsonValue::String(String::from_utf8_lossy(&v).into_owned()),
                            )
                        })
                        .collect(),
                )
            }),
            response_body: req
                .response_body
                .map(|b| String::from_utf8_lossy(&b).into_owned()),
        }
    }
}

/// An exported private key with PKCS#8 data as base64.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedPrivateKey {
    pub id: Uuid,
    pub name: String,
    pub alg: PrivateKeyAlgorithm,
    pub pkcs8: String,
    pub encrypted: bool,
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}

impl From<PrivateKey> for ExportedPrivateKey {
    fn from(key: PrivateKey) -> Self {
        Self {
            id: key.id,
            name: key.name,
            alg: key.alg,
            pkcs8: openssl::base64::encode_block(&key.pkcs8),
            encrypted: key.encrypted,
            created_at: key.created_at,
            updated_at: key.updated_at,
        }
    }
}

/// The retrack tracker data as stored in the export file.
///
/// Unlike `RetrackTrackerValue`, this uses the standard `TrackerTarget` serialization
/// (internally tagged with `type`) so the format roundtrips cleanly for import.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedRetrackData {
    pub enabled: bool,
    pub config: TrackerConfig,
    pub target: TrackerTarget,
    pub notifications: bool,
}

/// An exported tracker (page or API) with optional revision history.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedTracker {
    pub id: Uuid,
    pub name: String,
    pub retrack: ExportedRetrackData,
    #[serde(default, skip_serializing_if = "SecretsAccess::is_none")]
    pub secrets: SecretsAccess,
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<TrackerDataRevision>,
}

/// Helper to convert a tracker with retrack data into an exported tracker.
fn to_exported_tracker(
    id: Uuid,
    name: String,
    retrack: &RetrackTracker,
    secrets: SecretsAccess,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    history: Vec<TrackerDataRevision>,
) -> Option<ExportedTracker> {
    let value = match retrack {
        RetrackTracker::Value(v) => v,
        RetrackTracker::Reference { .. } => return None,
    };
    Some(ExportedTracker {
        id,
        name,
        retrack: ExportedRetrackData {
            enabled: value.enabled,
            config: value.config.clone(),
            target: value.target.clone(),
            notifications: value.notifications,
        },
        secrets,
        created_at,
        updated_at,
        history,
    })
}

impl PageTracker {
    /// Converts this page tracker into an exported tracker with optional history.
    pub fn into_exported(self, history: Vec<TrackerDataRevision>) -> Option<ExportedTracker> {
        to_exported_tracker(
            self.id,
            self.name,
            &self.retrack,
            self.secrets,
            self.created_at,
            self.updated_at,
            history,
        )
    }
}

impl ApiTracker {
    /// Converts this API tracker into an exported tracker with optional history.
    pub fn into_exported(self, history: Vec<TrackerDataRevision>) -> Option<ExportedTracker> {
        to_exported_tracker(
            self.id,
            self.name,
            &self.retrack,
            self.secrets,
            self.created_at,
            self.updated_at,
            history,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        users::user_data::shared::DataFileSecret,
        utils::webhooks::{
            ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
        },
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::borrow::Cow;
    use time::macros::datetime;

    #[test]
    fn export_data_structure() {
        let export = UserDataExport {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataExportData {
                scripts: vec![],
                secrets: vec![DataFileSecret {
                    id: uuid::Uuid::nil(),
                    name: "TEST".to_string(),
                    encrypted_value: None,
                    created_at: datetime!(2020-01-01 00:00:00 UTC),
                    updated_at: datetime!(2020-01-01 00:00:00 UTC),
                }],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
                settings: None,
            },
        };
        assert_json_snapshot!(export, @r###"
        {
          "version": 1,
          "exportedAt": 1577880000,
          "data": {
            "secrets": [
              {
                "id": "00000000-0000-0000-0000-000000000000",
                "name": "TEST",
                "createdAt": 1577836800,
                "updatedAt": 1577836800
              }
            ]
          }
        }
        "###);
    }

    #[test]
    fn export_data_empty_vectors_omitted() {
        let data = UserDataExportData {
            scripts: vec![],
            secrets: vec![],
            responders: vec![],
            certificate_templates: vec![],
            private_keys: vec![],
            content_security_policies: vec![],
            page_trackers: vec![],
            api_trackers: vec![],
            settings: None,
        };
        let json = serde_json::to_value(&data).unwrap();
        // All fields should be omitted since all are empty.
        assert_eq!(json, json!({}));
    }

    #[test]
    fn export_data_secrets_encryption_omitted_when_none() {
        let export = UserDataExport {
            version: EXPORT_VERSION,
            exported_at: datetime!(2020-01-01 00:00:00 UTC),
            secrets_encryption: None,
            data: UserDataExportData {
                scripts: vec![],
                secrets: vec![],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
                settings: None,
            },
        };
        let json = serde_json::to_value(&export).unwrap();
        assert!(json.get("secretsEncryption").is_none());
    }

    #[test]
    fn serialize_exported_private_key() {
        let key = ExportedPrivateKey {
            id: Uuid::nil(),
            name: "my-key".to_string(),
            alg: PrivateKeyAlgorithm::Ed25519,
            pkcs8: openssl::base64::encode_block(&[1, 2, 3]),
            encrypted: false,
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-06-01 00:00:00 UTC),
        };
        assert_json_snapshot!(key, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000000",
          "name": "my-key",
          "alg": {
            "keyType": "ed25519"
          },
          "pkcs8": "AQID",
          "encrypted": false,
          "createdAt": 1577836800,
          "updatedAt": 1590969600
        }
        "###);
    }

    #[test]
    fn serialize_exported_responder_without_history() {
        let exported = ExportedResponder {
            responder: Responder {
                id: Uuid::nil(),
                name: "resp-1".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/api/test".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: crate::users::SecretsAccess::None,
                },
                created_at: datetime!(2020-01-01 00:00:00 UTC),
                updated_at: datetime!(2020-06-01 00:00:00 UTC),
            },
            history: vec![],
        };
        let json = serde_json::to_value(&exported).unwrap();
        assert_eq!(json["name"], "resp-1");
        assert_eq!(json["enabled"], true);
        // history is empty so should be omitted
        assert!(json.get("history").is_none());
    }

    #[test]
    fn serialize_exported_responder_request() {
        let req = ExportedResponderRequest {
            id: Uuid::nil(),
            responder_id: Uuid::nil(),
            client_address: Some("127.0.0.1".to_string()),
            method: "POST".to_string(),
            headers: Some(json!({"Content-Type": "application/json"})),
            url: "https://example.com/test".to_string(),
            body: Some("request body".to_string()),
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            duration_ms: Some(150),
            response_status_code: Some(200),
            response_headers: None,
            response_body: Some("response body".to_string()),
        };
        assert_json_snapshot!(req, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000000",
          "responderId": "00000000-0000-0000-0000-000000000000",
          "clientAddress": "127.0.0.1",
          "method": "POST",
          "headers": {
            "Content-Type": "application/json"
          },
          "url": "https://example.com/test",
          "body": "request body",
          "createdAt": 1577836800,
          "durationMs": 150,
          "responseStatusCode": 200,
          "responseBody": "response body"
        }
        "###);
    }

    #[test]
    fn exported_private_key_from_private_key() {
        let exported = ExportedPrivateKey::from(PrivateKey {
            id: Uuid::nil(),
            name: "test-key".to_string(),
            alg: PrivateKeyAlgorithm::Ed25519,
            pkcs8: vec![10, 20, 30, 40],
            encrypted: true,
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-06-01 00:00:00 UTC),
        });
        assert_eq!(exported.name, "test-key");
        assert!(exported.encrypted);
        // pkcs8 should be base64-encoded
        let decoded = openssl::base64::decode_block(&exported.pkcs8).unwrap();
        assert_eq!(decoded, vec![10, 20, 30, 40]);
    }

    #[test]
    fn exported_responder_request_from_responder_request() {
        let exported = ExportedResponderRequest::from(ResponderRequest {
            id: Uuid::nil(),
            responder_id: Uuid::nil(),
            client_address: Some(std::net::SocketAddr::from(([127, 0, 0, 1], 8080))),
            method: Cow::Borrowed("PUT"),
            headers: Some(vec![(
                Cow::Borrowed("X-Key"),
                Cow::Borrowed(b"x-value" as &[u8]),
            )]),
            url: Cow::Borrowed("https://example.com/path"),
            body: Some(Cow::Borrowed(b"body-data" as &[u8])),
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            duration_ms: Some(42),
            response_status_code: Some(201),
            response_headers: None,
            response_body: None,
        });
        assert_eq!(exported.method, "PUT");
        assert_eq!(exported.client_address.as_deref(), Some("127.0.0.1:8080"));
        assert_eq!(exported.url, "https://example.com/path");
        assert_eq!(exported.body.as_deref(), Some("body-data"));
        assert_eq!(exported.duration_ms, Some(42));
        assert_eq!(exported.response_status_code, Some(201));
        // Verify headers are converted correctly.
        let headers = exported.headers.unwrap();
        assert_eq!(headers["X-Key"], "x-value");
    }
}
