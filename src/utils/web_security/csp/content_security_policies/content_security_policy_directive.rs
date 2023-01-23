use crate::utils::{
    ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
    ContentSecurityPolicyWebrtcDirectiveValue,
};
use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};
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
            _ => true,
        }
    }
}

impl TryFrom<ContentSecurityPolicyDirective> for String {
    type Error = anyhow::Error;

    fn try_from(value: ContentSecurityPolicyDirective) -> Result<Self, Self::Error> {
        serde_json::to_value(value)?
            .as_object()
            .and_then(|directive| {
                let directive_name = directive.get("n")?.as_str()?;
                let mut directive_values = directive
                    .get("v")?
                    .as_array()?
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Option<Vec<_>>>()?;
                directive_values.sort();
                Some(if directive_values.is_empty() {
                    directive_name.to_string()
                } else {
                    format!("{} {}", directive_name, directive_values.join(" "))
                })
            })
            .ok_or_else(|| anyhow!("Cannot serialize directive."))
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ContentSecurityPolicyDirective, ContentSecurityPolicySandboxDirectiveValue,
        ContentSecurityPolicySource, ContentSecurityPolicyWebrtcDirectiveValue,
    };
    use insta::{assert_debug_snapshot, assert_json_snapshot};
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

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let sources = ["'self'".to_string()].into_iter().collect::<HashSet<_>>();
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "child-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ChildSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "connect-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ConnectSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "default-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::DefaultSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "font-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FontSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "frame-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FrameSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "img-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ImgSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "manifest-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ManifestSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "media-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::MediaSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "object-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ObjectSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "script-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ScriptSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "script-src-elem", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ScriptSrcElem(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "script-src-attr", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ScriptSrcAttr(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "style-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::StyleSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "style-src-elem", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::StyleSrcElem(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "style-src-attr", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::StyleSrcAttr(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "webrtc", "v": ["'allow'"] }"###
            )?,
            ContentSecurityPolicyDirective::Webrtc([
                ContentSecurityPolicyWebrtcDirectiveValue::Allow
            ])
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "worker-src", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::WorkerSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "base-uri", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::BaseUri(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "sandbox", "v": ["allow-forms", "allow-top-navigation"] }"###
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
                r###"{ "n": "form-action", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FormAction(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "frame-ancestors", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FrameAncestors(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "report-to", "v": ["https://google.com"] }"###
            )?,
            ContentSecurityPolicyDirective::ReportTo(["https://google.com".to_string()])
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "n": "report-uri", "v": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ReportUri(sources)
        );

        Ok(())
    }

    #[test]
    fn deserialization_failures() -> anyhow::Result<()> {
        assert_debug_snapshot!(serde_json::from_str::<ContentSecurityPolicyDirective>(
            r###"{ "n": "webrtc", "v": ["'allow'", "'block'"] }"###,
        ), @r###"
        Err(
            Error("trailing characters", line: 1, column: 35),
        )
        "###);

        assert_debug_snapshot!(serde_json::from_str::<ContentSecurityPolicyDirective>(
            r###"{ "n": "report-to", "v": ["https://google.com", "https://yahoo.com"] }"###
        ), @r###"
        Err(
            Error("trailing characters", line: 1, column: 49),
        )
        "###);

        Ok(())
    }

    #[test]
    fn should_correct_determine_if_supported_for_source() -> anyhow::Result<()> {
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
            ContentSecurityPolicyDirective::ReportUri(sources),
            ContentSecurityPolicyDirective::ReportTo(["endpoint".to_string()]),
        ];

        let (header_only_directives, universal_directives): (Vec<_>, Vec<_>) =
            all_directives.into_iter().partition(|directive| {
                matches!(
                    directive,
                    ContentSecurityPolicyDirective::Sandbox(_)
                        | ContentSecurityPolicyDirective::FrameAncestors(_)
                        | ContentSecurityPolicyDirective::ReportUri(_)
                        | ContentSecurityPolicyDirective::ReportTo(_)
                )
            });

        assert_eq!(header_only_directives.len(), 4);
        for directive in header_only_directives {
            assert!(directive.is_supported_for_source(ContentSecurityPolicySource::Header));
            assert!(!directive.is_supported_for_source(ContentSecurityPolicySource::Meta));
        }

        assert_eq!(universal_directives.len(), 19);
        for directive in universal_directives {
            assert!(directive.is_supported_for_source(ContentSecurityPolicySource::Header));
            assert!(directive.is_supported_for_source(ContentSecurityPolicySource::Meta));
        }

        Ok(())
    }
}
