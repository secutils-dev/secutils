use serde::Serialize;
use uuid::Uuid;

/// A detected conflict between an import file and existing data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConflict {
    /// ID in the import file.
    pub source_id: Uuid,
    /// Name of the entity in the import file.
    pub name: String,
    /// ID of the existing entity with the same name.
    pub existing_id: Uuid,
}

/// Per-entity-type summary in the preview.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntitySummary {
    pub total: usize,
    pub conflicts: Vec<ImportConflict>,
}

/// Items that would be deleted in Apply mode.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDeleteItem {
    pub id: Uuid,
    pub name: String,
}

/// Per-entity-type delete list for Apply mode.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDeleteSummary {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub responders: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub certificate_templates: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub private_keys: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub content_security_policies: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub page_trackers: Vec<ApplyDeleteItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub api_trackers: Vec<ApplyDeleteItem>,
}

/// The preview response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataImportPreview {
    pub valid: bool,
    pub version: u32,
    pub summary: ImportPreviewSummary,
    pub warnings: Vec<String>,
    /// Only populated in Apply mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_delete: Option<ApplyDeleteSummary>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewSummary {
    pub scripts: ImportEntitySummary,
    pub secrets: ImportEntitySummary,
    pub responders: ImportEntitySummary,
    pub certificate_templates: ImportEntitySummary,
    pub private_keys: ImportEntitySummary,
    pub content_security_policies: ImportEntitySummary,
    pub page_trackers: ImportEntitySummary,
    pub api_trackers: ImportEntitySummary,
}

/// Per-entity-type result counts.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntityResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// The import execution response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataImportResult {
    pub results: ImportResultsSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultsSummary {
    pub scripts: ImportEntityResult,
    pub secrets: ImportEntityResult,
    pub responders: ImportEntityResult,
    pub certificate_templates: ImportEntityResult,
    pub private_keys: ImportEntityResult,
    pub content_security_policies: ImportEntityResult,
    pub page_trackers: ImportEntityResult,
    pub api_trackers: ImportEntityResult,
}
