use crate::users::user_data::import::{ConflictResolution, ImportAction, ImportEntitySelection};

/// Returns whether the selection indicates the item should be skipped.
pub fn should_skip(selection: Option<&&ImportEntitySelection>) -> bool {
    match selection {
        Some(sel) => {
            sel.action == ImportAction::Skip
                || sel.conflict_resolution == Some(ConflictResolution::Skip)
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_skip_returns_true_for_skip_action() {
        use super::super::params::{ImportAction, ImportEntitySelection};
        use uuid::Uuid;
        let sel = ImportEntitySelection {
            source_id: Uuid::nil(),
            action: ImportAction::Skip,
            conflict_resolution: None,
        };
        let sel_ref = &sel;
        assert!(should_skip(Some(&&sel_ref)));
    }

    #[test]
    fn should_skip_returns_false_for_none() {
        assert!(!should_skip(None));
    }
}
