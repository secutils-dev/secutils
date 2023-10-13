use crate::utils::{
    web_security::{
        ContentSecurityPolicyRequireTrustedTypesForDirectiveValue,
        ContentSecurityPolicyTrustedTypesDirectiveValue,
    },
    ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
    ContentSecurityPolicyWebrtcDirectiveValue,
};
use anyhow::anyhow;
use content_security_policy::Directive;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value as JSONValue;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", tag = "n", content = "v")]
pub enum ContentSecurityPolicyDirective {
    // 15 fetch directives
    ChildSrc(HashSet<String>),
    ConnectSrc(HashSet<String>),
    DefaultSrc(HashSet<String>),
    FontSrc(HashSet<String>),
    FrameSrc(HashSet<String>),
    ImgSrc(HashSet<String>),
    ManifestSrc(HashSet<String>),
    MediaSrc(HashSet<String>),
    ObjectSrc(HashSet<String>),
    ScriptSrc(HashSet<String>),
    ScriptSrcElem(HashSet<String>),
    ScriptSrcAttr(HashSet<String>),
    StyleSrc(HashSet<String>),
    StyleSrcElem(HashSet<String>),
    StyleSrcAttr(HashSet<String>),
    // 2 other directives
    Webrtc([ContentSecurityPolicyWebrtcDirectiveValue; 1]),
    WorkerSrc(HashSet<String>),
    // 2 document directives
    BaseUri(HashSet<String>),
    Sandbox(HashSet<ContentSecurityPolicySandboxDirectiveValue>),
    // 2 navigation directives
    FormAction(HashSet<String>),
    FrameAncestors(HashSet<String>),
    // 1 extension directive
    #[serde(deserialize_with = "deserialize_directive_without_value")]
    UpgradeInsecureRequests,
    // 2 experimental directives
    RequireTrustedTypesFor([ContentSecurityPolicyRequireTrustedTypesForDirectiveValue; 1]),
    TrustedTypes(HashSet<ContentSecurityPolicyTrustedTypesDirectiveValue>),
    // 2 reporting directives
    ReportUri(HashSet<String>),
    ReportTo([String; 1]),
}

impl ContentSecurityPolicyDirective {
    pub fn is_supported_for_source(&self, source: ContentSecurityPolicySource) -> bool {
        match (self, source) {
            // See https://html.spec.whatwg.org/multipage/semantics.html#attr-meta-http-equiv-content-security-policy
            (
                ContentSecurityPolicyDirective::Sandbox(_)
                | ContentSecurityPolicyDirective::FrameAncestors(_)
                | ContentSecurityPolicyDirective::ReportUri(_)
                | ContentSecurityPolicyDirective::ReportTo(_),
                ContentSecurityPolicySource::Meta,
            ) => false,
            // See https://w3c.github.io/webappsec-csp/#directive-sandbox
            (
                ContentSecurityPolicyDirective::Sandbox(_),
                ContentSecurityPolicySource::ReportOnlyHeader,
            ) => false,
            _ => true,
        }
    }
}

impl TryFrom<&Directive> for ContentSecurityPolicyDirective {
    type Error = anyhow::Error;

    fn try_from(directive: &Directive) -> Result<Self, Self::Error> {
        // [HACK]: Since `Directive` from the `content_security_policy` crate doesn't expose
        // directive name and values publicly, we need to serialize it to JSON and then deserialize
        // it back to the required enum. Before deserializing we should patch property names as
        // serialization formats aren't compatible (`name` -> `n` and `value` -> `v`). We rely on
        // this expensive hack to have only one place that's aware of the CSP format. Eventually,
        // we should update the `content_security_policy` crate and get rid of this workaround.
        let mut serialized_directive = serde_json::to_value(directive)?;
        if let JSONValue::Object(object) = &mut serialized_directive {
            for (source_key, target_key) in [("name", "n"), ("value", "v")] {
                if let Some(value) = object.remove(source_key) {
                    object.insert(target_key.to_string(), value);
                }
            }
        }

        Ok(serde_json::from_value(serialized_directive)?)
    }
}

/// A custom deserialization function for directive types without values. It's required because
/// `content_security_policy` crate parses such directive with empty values array causing
/// deserialization failure.
fn deserialize_directive_without_value<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    if Vec::<String>::deserialize(deserializer)?.is_empty() {
        Ok(())
    } else {
        Err(de::Error::invalid_value(de::Unexpected::UnitVariant, &"0"))
    }
}

impl TryFrom<ContentSecurityPolicyDirective> for String {
    type Error = anyhow::Error;

    fn try_from(value: ContentSecurityPolicyDirective) -> Result<Self, Self::Error> {
        serde_json::to_value(value)?
            .as_object()
            .and_then(|directive| {
                let directive_value = if let Some(value_items) = directive.get("v") {
                    let mut value_items = value_items
                        .as_array()?
                        .iter()
                        .map(|value| value.as_str())
                        .collect::<Option<Vec<_>>>()?;
                    if !value_items.is_empty() {
                        value_items.sort();
                        Some(value_items.join(" "))
                    } else {
                        None
                    }
                } else {
                    None
                };

                let directive_name = directive.get("n")?.as_str()?;
                Some(if let Some(directive_value) = directive_value {
                    format!("{} {}", directive_name, directive_value)
                } else {
                    directive_name.to_string()
                })
            })
            .ok_or_else(|| anyhow!("Cannot serialize directive."))
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ContentSecurityPolicyDirective, ContentSecurityPolicyRequireTrustedTypesForDirectiveValue,
        ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
        ContentSecurityPolicyTrustedTypesDirectiveValue, ContentSecurityPolicyWebrtcDirectiveValue,
    };
    use content_security_policy::Directive;
    use insta::{assert_debug_snapshot, assert_json_snapshot};
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn serialization_to_json() -> anyhow::Result<()> {
        let sources = ["'self'".to_string()].into_iter().collect::<HashSet<_>>();
        assert_json_snapshot!(ContentSecurityPolicyDirective::ChildSrc(sources.clone()), @r###"
        {
          "n": "child-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ConnectSrc(sources.clone()), @r###"
        {
          "n": "connect-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::DefaultSrc(sources.clone()), @r###"
        {
          "n": "default-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FontSrc(sources.clone()), @r###"
        {
          "n": "font-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FrameSrc(sources.clone()), @r###"
        {
          "n": "frame-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ImgSrc(sources.clone()), @r###"
        {
          "n": "img-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ManifestSrc(sources.clone()), @r###"
        {
          "n": "manifest-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::MediaSrc(sources.clone()), @r###"
        {
          "n": "media-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ObjectSrc(sources.clone()), @r###"
        {
          "n": "object-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ScriptSrc(sources.clone()), @r###"
        {
          "n": "script-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ScriptSrcElem(sources.clone()), @r###"
        {
          "n": "script-src-elem",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ScriptSrcAttr(sources.clone()), @r###"
        {
          "n": "script-src-attr",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::StyleSrc(sources.clone()), @r###"
        {
          "n": "style-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::StyleSrcElem(sources.clone()), @r###"
        {
          "n": "style-src-elem",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::StyleSrcAttr(sources.clone()), @r###"
        {
          "n": "style-src-attr",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::Webrtc([ContentSecurityPolicyWebrtcDirectiveValue::Allow]), @r###"
        {
          "n": "webrtc",
          "v": [
            "'allow'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::WorkerSrc(sources.clone()), @r###"
        {
          "n": "worker-src",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::BaseUri(sources.clone()), @r###"
        {
          "n": "base-uri",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::Sandbox([
            ContentSecurityPolicySandboxDirectiveValue::AllowForms
        ]
        .into_iter()
        .collect()), @r###"
        {
          "n": "sandbox",
          "v": [
            "allow-forms"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FormAction(sources.clone()), @r###"
        {
          "n": "form-action",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FrameAncestors(sources.clone()), @r###"
        {
          "n": "frame-ancestors",
          "v": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::UpgradeInsecureRequests, @r###"
        {
          "n": "upgrade-insecure-requests"
        }
        "###);

        assert_json_snapshot!(
            ContentSecurityPolicyDirective::RequireTrustedTypesFor([ContentSecurityPolicyRequireTrustedTypesForDirectiveValue::Script]),
            @r###"
        {
          "n": "require-trusted-types-for",
          "v": [
            "'script'"
          ]
        }
        "###
        );

        assert_json_snapshot!(ContentSecurityPolicyDirective::TrustedTypes([
            ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates
        ]
        .into_iter()
        .collect()), @r###"
        {
          "n": "trusted-types",
          "v": [
            "'allow-duplicates'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ReportTo(["https://google.com".to_string()]), @r###"
        {
          "n": "report-to",
          "v": [
            "https://google.com"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ReportUri(sources), @r###"
        {
          "n": "report-uri",
          "v": [
            "'self'"
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn serialization_to_string() -> anyhow::Result<()> {
        assert_debug_snapshot!(
            String::try_from(ContentSecurityPolicyDirective::DefaultSrc(["'self'".to_string(), "https:".to_string()]
            .into_iter()
            .collect::<HashSet<_>>()))?, @r###""default-src 'self' https:""###);

        assert_debug_snapshot!(
            String::try_from(ContentSecurityPolicyDirective::UpgradeInsecureRequests)?,
            @r###""upgrade-insecure-requests""###
        );

        assert_debug_snapshot!(
            String::try_from(ContentSecurityPolicyDirective::RequireTrustedTypesFor(
                [ContentSecurityPolicyRequireTrustedTypesForDirectiveValue::Script]
            ))?,
            @r###""require-trusted-types-for 'script'""###
        );

        assert_debug_snapshot!(
            String::try_from(
                ContentSecurityPolicyDirective::TrustedTypes([
                    ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates,
                    ContentSecurityPolicyTrustedTypesDirectiveValue::PolicyName("my-policy".to_string()),
                    ContentSecurityPolicyTrustedTypesDirectiveValue::PolicyName("my-another-policy".to_string())
                ]
                .into_iter()
                .collect())
            )?,
            @r###""trusted-types 'allow-duplicates' my-another-policy my-policy""###
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let sources = ["'self'".to_string()].into_iter().collect::<HashSet<_>>();
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "child-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ChildSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "connect-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ConnectSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "default-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::DefaultSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "font-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::FontSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "frame-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::FrameSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "img-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ImgSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "manifest-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ManifestSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "media-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::MediaSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "object-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ObjectSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "script-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ScriptSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "script-src-elem", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ScriptSrcElem(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "script-src-attr", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ScriptSrcAttr(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "style-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::StyleSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "style-src-elem", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::StyleSrcElem(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "style-src-attr", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::StyleSrcAttr(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "webrtc", "v": ["'allow'"] }"#
            )?,
            ContentSecurityPolicyDirective::Webrtc([
                ContentSecurityPolicyWebrtcDirectiveValue::Allow
            ])
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "worker-src", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::WorkerSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "base-uri", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::BaseUri(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "sandbox", "v": ["allow-forms", "allow-top-navigation"] }"#
            )?,
            ContentSecurityPolicyDirective::Sandbox(
                [
                    ContentSecurityPolicySandboxDirectiveValue::AllowForms,
                    ContentSecurityPolicySandboxDirectiveValue::AllowTopNavigation
                ]
                .into_iter()
                .collect()
            )
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "form-action", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::FormAction(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "frame-ancestors", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::FrameAncestors(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "upgrade-insecure-requests" }"#
            )?,
            ContentSecurityPolicyDirective::UpgradeInsecureRequests
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "require-trusted-types-for", "v": ["'script'"] }"#
            )?,
            ContentSecurityPolicyDirective::RequireTrustedTypesFor([
                ContentSecurityPolicyRequireTrustedTypesForDirectiveValue::Script
            ])
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "trusted-types", "v": ["'allow-duplicates'", "my-another-policy", "my-policy"] }"#
            )?,
            ContentSecurityPolicyDirective::TrustedTypes(
                [
                    ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates,
                    ContentSecurityPolicyTrustedTypesDirectiveValue::PolicyName(
                        "my-policy".to_string()
                    ),
                    ContentSecurityPolicyTrustedTypesDirectiveValue::PolicyName(
                        "my-another-policy".to_string()
                    )
                ]
                .into_iter()
                .collect()
            )
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "report-to", "v": ["https://google.com"] }"#
            )?,
            ContentSecurityPolicyDirective::ReportTo(["https://google.com".to_string()])
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r#"{ "n": "report-uri", "v": ["'self'"] }"#
            )?,
            ContentSecurityPolicyDirective::ReportUri(sources)
        );

        Ok(())
    }

    #[test]
    fn conversion_from_directive() -> anyhow::Result<()> {
        let directive = serde_json::from_value::<Directive>(json!({
            "name": "child-src",
            "value": ["'self'", "https://secutils.dev"]
        }))?;

        assert_eq!(
            ContentSecurityPolicyDirective::try_from(&directive)?,
            ContentSecurityPolicyDirective::ChildSrc(
                ["'self'".to_string(), "https://secutils.dev".to_string()]
                    .into_iter()
                    .collect()
            )
        );

        let directive = serde_json::from_value::<Directive>(json!({
            "name": "upgrade-insecure-requests",
            "value": []
        }))?;

        assert_eq!(
            ContentSecurityPolicyDirective::try_from(&directive)?,
            ContentSecurityPolicyDirective::UpgradeInsecureRequests
        );

        Ok(())
    }

    #[test]
    fn deserialization_failures() -> anyhow::Result<()> {
        assert_debug_snapshot!(serde_json::from_str::<ContentSecurityPolicyDirective>(
            r#"{ "n": "webrtc", "v": ["'allow'", "'block'"] }"#,
        ), @r###"
        Err(
            Error("trailing characters", line: 1, column: 35),
        )
        "###);

        assert_debug_snapshot!(serde_json::from_str::<ContentSecurityPolicyDirective>(
            r#"{ "n": "report-to", "v": ["https://google.com", "https://yahoo.com"] }"#
        ), @r###"
        Err(
            Error("trailing characters", line: 1, column: 49),
        )
        "###);

        assert_debug_snapshot!(serde_json::from_str::<ContentSecurityPolicyDirective>(
            r#"{ "n": "require-trusted-types-for", "v": ["'script'", "'script'"] }"#
        ), @r###"
        Err(
            Error("trailing characters", line: 1, column: 55),
        )
        "###);

        assert_debug_snapshot!(serde_json::from_str::<ContentSecurityPolicyDirective>(
            r#"{ "n": "require-trusted-types-for", "v": ["'none'"] }"#
        ), @r###"
        Err(
            Error("unknown variant `'none'`, expected `'script'`", line: 1, column: 50),
        )
        "###);

        Ok(())
    }

    #[test]
    fn should_correctly_determine_if_supported_for_source() -> anyhow::Result<()> {
        let sources = ["'self'".to_string()]
            .into_iter()
            .collect::<HashSet<String>>();
        let all_directives = vec![
            ContentSecurityPolicyDirective::ChildSrc(sources.clone()),
            ContentSecurityPolicyDirective::ConnectSrc(sources.clone()),
            ContentSecurityPolicyDirective::DefaultSrc(sources.clone()),
            ContentSecurityPolicyDirective::FontSrc(sources.clone()),
            ContentSecurityPolicyDirective::FrameSrc(sources.clone()),
            ContentSecurityPolicyDirective::ImgSrc(sources.clone()),
            ContentSecurityPolicyDirective::ManifestSrc(sources.clone()),
            ContentSecurityPolicyDirective::MediaSrc(sources.clone()),
            ContentSecurityPolicyDirective::ObjectSrc(sources.clone()),
            ContentSecurityPolicyDirective::ScriptSrc(sources.clone()),
            ContentSecurityPolicyDirective::ScriptSrcElem(sources.clone()),
            ContentSecurityPolicyDirective::ScriptSrcAttr(sources.clone()),
            ContentSecurityPolicyDirective::StyleSrc(sources.clone()),
            ContentSecurityPolicyDirective::StyleSrcElem(sources.clone()),
            ContentSecurityPolicyDirective::StyleSrcAttr(sources.clone()),
            ContentSecurityPolicyDirective::UpgradeInsecureRequests,
            ContentSecurityPolicyDirective::Webrtc([
                ContentSecurityPolicyWebrtcDirectiveValue::Allow,
            ]),
            ContentSecurityPolicyDirective::WorkerSrc(sources.clone()),
            ContentSecurityPolicyDirective::BaseUri(sources.clone()),
            ContentSecurityPolicyDirective::Sandbox(
                [ContentSecurityPolicySandboxDirectiveValue::AllowForms]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::FormAction(sources.clone()),
            ContentSecurityPolicyDirective::FrameAncestors(sources.clone()),
            ContentSecurityPolicyDirective::UpgradeInsecureRequests,
            ContentSecurityPolicyDirective::RequireTrustedTypesFor([
                ContentSecurityPolicyRequireTrustedTypesForDirectiveValue::Script,
            ]),
            ContentSecurityPolicyDirective::TrustedTypes(
                [ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates]
                    .into_iter()
                    .collect(),
            ),
            ContentSecurityPolicyDirective::ReportUri(sources),
            ContentSecurityPolicyDirective::ReportTo(["endpoint".to_string()]),
        ];

        // Enforcing header supports all directives.
        for directive in all_directives.iter() {
            assert!(directive.is_supported_for_source(ContentSecurityPolicySource::EnforcingHeader));
        }

        // Report-only header supports all directives except for `sandbox`.
        let (unsupported_directives, report_only_directives): (Vec<_>, Vec<_>) = all_directives
            .iter()
            .partition(|directive| matches!(directive, ContentSecurityPolicyDirective::Sandbox(_)));
        assert_eq!(unsupported_directives.len(), 1);
        for directive in report_only_directives {
            assert!(
                directive.is_supported_for_source(ContentSecurityPolicySource::ReportOnlyHeader)
            );
        }
        for directive in unsupported_directives {
            assert!(
                !directive.is_supported_for_source(ContentSecurityPolicySource::ReportOnlyHeader)
            );
        }

        // Meta tag supports all directives except for `sandbox`, `frame-ancestors`, `report-uri` and `report-to`.
        let (unsupported_directives, meta_tag_directives): (Vec<_>, Vec<_>) =
            all_directives.iter().partition(|directive| {
                matches!(
                    directive,
                    ContentSecurityPolicyDirective::Sandbox(_)
                        | ContentSecurityPolicyDirective::FrameAncestors(_)
                        | ContentSecurityPolicyDirective::ReportUri(_)
                        | ContentSecurityPolicyDirective::ReportTo(_)
                )
            });
        assert_eq!(unsupported_directives.len(), 4);
        for directive in meta_tag_directives {
            assert!(directive.is_supported_for_source(ContentSecurityPolicySource::Meta));
        }
        for directive in unsupported_directives {
            assert!(!directive.is_supported_for_source(ContentSecurityPolicySource::Meta));
        }

        Ok(())
    }
}
