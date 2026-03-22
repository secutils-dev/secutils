use super::file::UserDataImportFile;
use serde::Deserialize;
use uuid::Uuid;

/// Import mode.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportMode {
    Merge,
    Apply,
}

/// Parameters for the preview request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataImportPreviewParams {
    pub data: UserDataImportFile,
    pub mode: ImportMode,
}

/// Parameters for the import execution request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataImportParams {
    pub data: UserDataImportFile,
    pub mode: ImportMode,
    #[serde(default)]
    pub selections: ImportSelections,
    /// Passphrase to decrypt secret values from the import file.
    /// Required when the file contains encrypted secret values.
    #[serde(default)]
    pub secrets_passphrase: Option<String>,
    /// IDs of entities to delete in Apply mode (keyed by entity type).
    #[serde(default)]
    pub apply_deletions: Option<ApplyDeletionSelections>,
}

/// Specifies which entities to delete in Apply mode (user-confirmed deletions).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDeletionSelections {
    #[serde(default)]
    pub scripts: Vec<Uuid>,
    #[serde(default)]
    pub secrets: Vec<Uuid>,
    #[serde(default)]
    pub responders: Vec<Uuid>,
    #[serde(default)]
    pub certificate_templates: Vec<Uuid>,
    #[serde(default)]
    pub private_keys: Vec<Uuid>,
    #[serde(default)]
    pub content_security_policies: Vec<Uuid>,
    #[serde(default)]
    pub page_trackers: Vec<Uuid>,
    #[serde(default)]
    pub api_trackers: Vec<Uuid>,
}

/// Per-entity selections and conflict resolution.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSelections {
    #[serde(default)]
    pub scripts: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub secrets: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub responders: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub certificate_templates: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub private_keys: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub content_security_policies: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub page_trackers: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub api_trackers: Vec<ImportEntitySelection>,
    #[serde(default)]
    pub import_settings: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntitySelection {
    pub source_id: Uuid,
    pub action: ImportAction,
    #[serde(default)]
    pub conflict_resolution: Option<ConflictResolution>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportAction {
    Import,
    Skip,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictResolution {
    Rename,
    Overwrite,
    Skip,
}
