use serde::Deserialize;
use uuid::Uuid;

/// Parameters for the export request, specifying which entities to include.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataExportParams {
    pub include: UserDataExportInclude,
    /// Optional passphrase for encrypting secret values in the export.
    /// If provided, secret values are decrypted with the server key and re-encrypted
    /// with a passphrase-derived key. If omitted, only secret names are exported.
    #[serde(default)]
    pub secrets_passphrase: Option<String>,
}

/// Selects which entities to export: all or specific IDs.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ExportSelection {
    All,
    Selected { ids: Vec<Uuid> },
}

/// Selects which trackable entities (responders/trackers) to export, with optional history.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ExportTrackableSelection {
    #[serde(rename_all = "camelCase")]
    All {
        #[serde(default)]
        include_history: bool,
    },
    #[serde(rename_all = "camelCase")]
    Selected {
        ids: Vec<Uuid>,
        #[serde(default)]
        include_history: bool,
    },
}

/// Specifies which entity types and individual items to include in the export.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDataExportInclude {
    #[serde(default)]
    pub scripts: Option<ExportSelection>,
    #[serde(default)]
    pub secrets: Option<ExportSelection>,
    #[serde(default)]
    pub responders: Option<ExportTrackableSelection>,
    #[serde(default)]
    pub certificate_templates: Option<ExportSelection>,
    #[serde(default)]
    pub private_keys: Option<ExportSelection>,
    #[serde(default)]
    pub content_security_policies: Option<ExportSelection>,
    #[serde(default)]
    pub page_trackers: Option<ExportTrackableSelection>,
    #[serde(default)]
    pub api_trackers: Option<ExportTrackableSelection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn deserialize_export_selection_all() {
        let sel: ExportSelection = serde_json::from_value(json!({"type": "all"})).unwrap();
        assert!(matches!(sel, ExportSelection::All));
    }

    #[test]
    fn deserialize_export_selection_selected() {
        let id = Uuid::now_v7();
        let sel: ExportSelection =
            serde_json::from_value(json!({"type": "selected", "ids": [id]})).unwrap();
        match sel {
            ExportSelection::Selected { ids } => {
                assert_eq!(ids, vec![id]);
            }
            _ => panic!("Expected Selected variant"),
        }
    }

    #[test]
    fn deserialize_export_selection_selected_empty_ids() {
        let sel: ExportSelection =
            serde_json::from_value(json!({"type": "selected", "ids": []})).unwrap();
        match sel {
            ExportSelection::Selected { ids } => assert!(ids.is_empty()),
            _ => panic!("Expected Selected variant"),
        }
    }

    #[test]
    fn deserialize_export_trackable_selection_all_with_history() {
        let sel: ExportTrackableSelection =
            serde_json::from_value(json!({"type": "all", "includeHistory": true})).unwrap();
        match sel {
            ExportTrackableSelection::All { include_history } => {
                assert!(include_history);
            }
            _ => panic!("Expected All variant"),
        }
    }

    #[test]
    fn deserialize_export_trackable_selection_all_default_history() {
        let sel: ExportTrackableSelection = serde_json::from_value(json!({"type": "all"})).unwrap();
        match sel {
            ExportTrackableSelection::All { include_history } => {
                assert!(!include_history);
            }
            _ => panic!("Expected All variant"),
        }
    }

    #[test]
    fn deserialize_export_trackable_selection_selected_with_history() {
        let id = Uuid::now_v7();
        let sel: ExportTrackableSelection = serde_json::from_value(
            json!({"type": "selected", "ids": [id], "includeHistory": true}),
        )
        .unwrap();
        match sel {
            ExportTrackableSelection::Selected {
                ids,
                include_history,
            } => {
                assert_eq!(ids, vec![id]);
                assert!(include_history);
            }
            _ => panic!("Expected Selected variant"),
        }
    }

    #[test]
    fn deserialize_export_trackable_selection_selected_default_history() {
        let id = Uuid::now_v7();
        let sel: ExportTrackableSelection =
            serde_json::from_value(json!({"type": "selected", "ids": [id]})).unwrap();
        match sel {
            ExportTrackableSelection::Selected {
                ids,
                include_history,
            } => {
                assert_eq!(ids, vec![id]);
                assert!(!include_history);
            }
            _ => panic!("Expected Selected variant"),
        }
    }

    #[test]
    fn deserialize_user_data_export_params() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let params: UserDataExportParams = serde_json::from_value(json!({
            "include": {
                "scripts": {"type": "all"},
                "secrets": {"type": "selected", "ids": [id1]},
                "responders": {"type": "all", "includeHistory": true},
                "pageTrackers": {"type": "selected", "ids": [id2], "includeHistory": false}
            },
            "secretsPassphrase": "my-passphrase"
        }))
        .unwrap();
        assert!(matches!(params.include.scripts, Some(ExportSelection::All)));
        assert!(matches!(
            params.include.secrets,
            Some(ExportSelection::Selected { .. })
        ));
        assert!(matches!(
            params.include.responders,
            Some(ExportTrackableSelection::All {
                include_history: true
            })
        ));
        assert!(matches!(
            params.include.page_trackers,
            Some(ExportTrackableSelection::Selected { .. })
        ));
        assert!(params.include.certificate_templates.is_none());
        assert!(params.include.private_keys.is_none());
        assert!(params.include.content_security_policies.is_none());
        assert!(params.include.api_trackers.is_none());
        assert_eq!(params.secrets_passphrase.as_deref(), Some("my-passphrase"));
    }

    #[test]
    fn deserialize_user_data_export_params_minimal() {
        let params: UserDataExportParams = serde_json::from_value(json!({"include": {}})).unwrap();
        assert!(params.include.scripts.is_none());
        assert!(params.include.secrets.is_none());
        assert!(params.include.responders.is_none());
        assert!(params.include.certificate_templates.is_none());
        assert!(params.include.private_keys.is_none());
        assert!(params.include.content_security_policies.is_none());
        assert!(params.include.page_trackers.is_none());
        assert!(params.include.api_trackers.is_none());
        assert!(params.secrets_passphrase.is_none());
    }
}
