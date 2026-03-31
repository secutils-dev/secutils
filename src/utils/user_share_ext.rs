use crate::{
    users::{SharedResource, UserShare},
    utils::{UtilsAction, UtilsResource},
};

impl UserShare {
    /// Checks if the user share is authorized to perform the specified action.
    pub fn is_action_authorized(&self, _action: &UtilsAction, _resource: &UtilsResource) -> bool {
        match &self.resource {
            // Certificate template shares are handled by dedicated /api/certificates/ routes.
            SharedResource::CertificateTemplate { .. } => false,
            // CSP shares are handled by dedicated /api/web_security/csp/ routes.
            SharedResource::ContentSecurityPolicy { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::{SharedResource, UserId, UserShare},
        utils::{UtilsAction, UtilsResource},
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn shared_csp_is_not_authorized_via_generic_dispatcher() {
        let policy_id = uuid!("00000000-0000-0000-0000-000000000001");
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::new(),
            resource: SharedResource::content_security_policy(policy_id),
            created_at: OffsetDateTime::now_utc(),
        };

        // CSP shares are handled by dedicated routes, not the generic dispatcher.
        assert!(
            !user_share
                .is_action_authorized(&UtilsAction::List, &UtilsResource::WebhooksResponders)
        );
        assert!(!user_share.is_action_authorized(
            &UtilsAction::Get {
                resource_id: policy_id,
            },
            &UtilsResource::WebhooksResponders
        ));
    }

    #[test]
    fn shared_certificate_template_is_not_authorized_via_generic_dispatcher() {
        let template_id = uuid!("00000000-0000-0000-0000-000000000001");
        let user_share = UserShare {
            id: Default::default(),
            user_id: UserId::new(),
            resource: SharedResource::certificate_template(template_id),
            created_at: OffsetDateTime::now_utc(),
        };

        assert!(
            !user_share
                .is_action_authorized(&UtilsAction::List, &UtilsResource::WebhooksResponders)
        );
    }
}
