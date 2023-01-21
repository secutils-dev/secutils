use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
#[allow(clippy::enum_variant_names)]
pub enum ContentSecurityPolicySandboxDirectiveValue {
    AllowDownloads,
    AllowForms,
    AllowModals,
    AllowOrientationLock,
    AllowPointerLock,
    AllowPopups,
    AllowPopupsToEscapeSandbox,
    AllowPresentation,
    AllowSameOrigin,
    AllowScripts,
    AllowTopNavigation,
    AllowTopNavigationByUserActivation,
    AllowTopNavigationToCustomProtocols,
}

#[cfg(test)]
mod tests {
    use crate::utils::ContentSecurityPolicySandboxDirectiveValue;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowDownloads, @r###""allow-downloads""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowForms, @r###""allow-forms""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowModals, @r###""allow-modals""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowOrientationLock, @r###""allow-orientation-lock""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowPointerLock, @r###""allow-pointer-lock""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowPopups, @r###""allow-popups""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowPopupsToEscapeSandbox, @r###""allow-popups-to-escape-sandbox""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowPresentation, @r###""allow-presentation""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowSameOrigin, @r###""allow-same-origin""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowScripts, @r###""allow-scripts""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigation, @r###""allow-top-navigation""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigationByUserActivation, @r###""allow-top-navigation-by-user-activation""###);
        assert_json_snapshot!(ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigationToCustomProtocols, @r###""allow-top-navigation-to-custom-protocols""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-downloads""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowDownloads
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-forms""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowForms
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-modals""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowModals
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-orientation-lock""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowOrientationLock
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-pointer-lock""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowPointerLock
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-popups""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowPopups
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-popups-to-escape-sandbox""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowPopupsToEscapeSandbox
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-presentation""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowPresentation
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-same-origin""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowSameOrigin
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-scripts""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowScripts
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-top-navigation""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigation
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-top-navigation-by-user-activation""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigationByUserActivation
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySandboxDirectiveValue>(
                r###""allow-top-navigation-to-custom-protocols""###
            )?,
            ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigationToCustomProtocols
        );

        Ok(())
    }
}
