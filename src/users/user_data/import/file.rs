use crate::{
    users::{
        secrets::SecretsEncryptionMeta,
        user_data::{
            export::{ExportedPrivateKey, ExportedResponder, ExportedTracker},
            shared::DataFileSecret,
        },
    },
    utils::{certificates::CertificateTemplate, web_security::ContentSecurityPolicy},
};
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

/// The expected import file structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataImportFile {
    pub version: u32,
    #[allow(dead_code)]
    #[serde(with = "time::serde::timestamp")]
    pub exported_at: OffsetDateTime,
    /// Encryption metadata for secret values (present when values are included).
    #[serde(default)]
    pub secrets_encryption: Option<SecretsEncryptionMeta>,
    pub data: UserDataImportFileData,
}

/// Data section of the import file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataImportFileData {
    #[serde(default)]
    pub scripts: Vec<ImportedScript>,
    #[serde(default)]
    pub secrets: Vec<DataFileSecret>,
    #[serde(default)]
    pub responders: Vec<ExportedResponder>,
    #[serde(default)]
    pub certificate_templates: Vec<CertificateTemplate>,
    #[serde(default)]
    pub private_keys: Vec<ExportedPrivateKey>,
    #[serde(default)]
    pub content_security_policies: Vec<ContentSecurityPolicy>,
    #[serde(default)]
    pub page_trackers: Vec<ExportedTracker>,
    #[serde(default)]
    pub api_trackers: Vec<ExportedTracker>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedScript {
    pub id: Uuid,
    pub name: String,
    pub script_type: String,
    pub content: String,
    #[allow(dead_code)]
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    #[allow(dead_code)]
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}
