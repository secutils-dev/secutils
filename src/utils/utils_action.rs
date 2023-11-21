use crate::utils::UtilsResourceOperation;
use uuid::Uuid;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsAction {
    /// Get all util's resources (GET).
    List,
    /// Create util's resource (POST).
    Create,
    /// Get util's resource details (GET).
    Get { resource_id: Uuid },
    /// Update util's resource (PUT).
    Update { resource_id: Uuid },
    /// Delete util's resource (DELETE).
    Delete { resource_id: Uuid },
    /// Share util's resource (POST).
    Share { resource_id: Uuid },
    /// Unshare util's resource (POST).
    Unshare { resource_id: Uuid },
    /// Execute util's resource custom operation (POST).
    Execute {
        resource_id: Uuid,
        operation: UtilsResourceOperation,
    },
}

impl UtilsAction {
    /// Returns true if the action requires parameters (via HTTP body).
    pub fn requires_params(&self) -> bool {
        match self {
            UtilsAction::Create | UtilsAction::Update { .. } => true,
            UtilsAction::List
            | UtilsAction::Get { .. }
            | UtilsAction::Delete { .. }
            | UtilsAction::Share { .. }
            | UtilsAction::Unshare { .. } => false,
            UtilsAction::Execute { operation, .. } => operation.requires_params(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsAction;
    use crate::utils::UtilsResourceOperation;
    use uuid::uuid;

    #[test]
    fn properly_checks_if_action_requires_params() {
        assert!(UtilsAction::Create.requires_params());
        assert!(UtilsAction::Update {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001")
        }
        .requires_params());

        assert!(!UtilsAction::List.requires_params());
        assert!(!UtilsAction::Get {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001")
        }
        .requires_params());
        assert!(!UtilsAction::Delete {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001")
        }
        .requires_params());
        assert!(!UtilsAction::Share {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001")
        }
        .requires_params());
        assert!(!UtilsAction::Unshare {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001")
        }
        .requires_params());

        assert!(UtilsAction::Execute {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
            operation: UtilsResourceOperation::CertificatesPrivateKeyExport,
        }
        .requires_params());
        assert!(UtilsAction::Execute {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
            operation: UtilsResourceOperation::CertificatesTemplateGenerate,
        }
        .requires_params());
        assert!(UtilsAction::Execute {
            resource_id: uuid!("00000000-0000-0000-0000-000000000001"),
            operation: UtilsResourceOperation::WebScrapingGetHistory,
        }
        .requires_params());
    }
}
