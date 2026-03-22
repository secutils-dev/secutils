use super::file::UserDataImportFileData;
use std::collections::HashSet;

pub fn detect_intra_file_duplicates(data: &UserDataImportFileData, warnings: &mut Vec<String>) {
    fn check_duplicates(names: &[&str], entity_type: &str, warnings: &mut Vec<String>) {
        let mut seen = HashSet::new();
        for name in names {
            if !seen.insert(*name) {
                warnings.push(format!(
                    "Duplicate {entity_type} name in import file: \"{name}\". Only the first occurrence will be imported."
                ));
            }
        }
    }

    check_duplicates(
        &data
            .scripts
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        "script",
        warnings,
    );
    check_duplicates(
        &data
            .secrets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        "secret",
        warnings,
    );
    check_duplicates(
        &data
            .responders
            .iter()
            .map(|r| r.responder.name.as_str())
            .collect::<Vec<_>>(),
        "responder",
        warnings,
    );
    check_duplicates(
        &data
            .certificate_templates
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        "certificate template",
        warnings,
    );
    check_duplicates(
        &data
            .private_keys
            .iter()
            .map(|k| k.name.as_str())
            .collect::<Vec<_>>(),
        "private key",
        warnings,
    );
    check_duplicates(
        &data
            .content_security_policies
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        "content security policy",
        warnings,
    );
    check_duplicates(
        &data
            .page_trackers
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        "page tracker",
        warnings,
    );
    check_duplicates(
        &data
            .api_trackers
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        "API tracker",
        warnings,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::user_data::import::file::{ImportedScript, UserDataImportFileData};
    use time::macros::datetime;
    use uuid::Uuid;

    fn empty_data() -> UserDataImportFileData {
        UserDataImportFileData {
            scripts: vec![],
            secrets: vec![],
            responders: vec![],
            certificate_templates: vec![],
            private_keys: vec![],
            content_security_policies: vec![],
            page_trackers: vec![],
            api_trackers: vec![],
            settings: None,
        }
    }

    #[test]
    fn detect_duplicates_reports_duplicate_script_names() {
        let script_id_1 = Uuid::now_v7();
        let script_id_2 = Uuid::now_v7();
        let mut data = empty_data();
        data.scripts = vec![
            ImportedScript {
                id: script_id_1,
                name: "my_script".to_string(),
                script_type: "responder".to_string(),
                content: "a".to_string(),
                created_at: datetime!(2020-01-01 00:00:00 UTC),
                updated_at: datetime!(2020-01-01 00:00:00 UTC),
            },
            ImportedScript {
                id: script_id_2,
                name: "my_script".to_string(),
                script_type: "responder".to_string(),
                content: "b".to_string(),
                created_at: datetime!(2020-01-01 00:00:00 UTC),
                updated_at: datetime!(2020-01-01 00:00:00 UTC),
            },
        ];
        let mut warnings = Vec::new();
        detect_intra_file_duplicates(&data, &mut warnings);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("my_script"));
    }

    #[test]
    fn detect_duplicates_no_warnings_for_unique_names() {
        let mut data = empty_data();
        data.scripts = vec![ImportedScript {
            id: Uuid::now_v7(),
            name: "script_a".to_string(),
            script_type: "responder".to_string(),
            content: "a".to_string(),
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }];
        let mut warnings = Vec::new();
        detect_intra_file_duplicates(&data, &mut warnings);
        assert!(warnings.is_empty());
    }
}
