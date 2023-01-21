use crate::utils::{
    ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicyWebrtcDirectiveValue,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
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
    Webrtc(ContentSecurityPolicyWebrtcDirectiveValue),
    WorkerSrc(HashSet<String>),
    // 2 document directives
    BaseUri(HashSet<String>),
    Sandbox(HashSet<ContentSecurityPolicySandboxDirectiveValue>),
    // 2 navigation directives
    FormAction(HashSet<String>),
    FrameAncestors(HashSet<String>),
    // 2 reporting directives
    ReportUri(HashSet<String>),
    ReportTo(String),
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ContentSecurityPolicyDirective, ContentSecurityPolicySandboxDirectiveValue,
        ContentSecurityPolicyWebrtcDirectiveValue,
    };
    use insta::assert_json_snapshot;
    use std::collections::HashSet;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let sources = ["'self'".to_string()].into_iter().collect::<HashSet<_>>();
        assert_json_snapshot!(ContentSecurityPolicyDirective::ChildSrc(sources.clone()), @r###"
        {
          "child-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ConnectSrc(sources.clone()), @r###"
        {
          "connect-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::DefaultSrc(sources.clone()), @r###"
        {
          "default-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FontSrc(sources.clone()), @r###"
        {
          "font-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FrameSrc(sources.clone()), @r###"
        {
          "frame-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ImgSrc(sources.clone()), @r###"
        {
          "img-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ManifestSrc(sources.clone()), @r###"
        {
          "manifest-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::MediaSrc(sources.clone()), @r###"
        {
          "media-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ObjectSrc(sources.clone()), @r###"
        {
          "object-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ScriptSrc(sources.clone()), @r###"
        {
          "script-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ScriptSrcElem(sources.clone()), @r###"
        {
          "script-src-elem": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ScriptSrcAttr(sources.clone()), @r###"
        {
          "script-src-attr": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::StyleSrc(sources.clone()), @r###"
        {
          "style-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::StyleSrcElem(sources.clone()), @r###"
        {
          "style-src-elem": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::StyleSrcAttr(sources.clone()), @r###"
        {
          "style-src-attr": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::Webrtc(ContentSecurityPolicyWebrtcDirectiveValue::Allow), @r###"
        {
          "webrtc": "'allow'"
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::WorkerSrc(sources.clone()), @r###"
        {
          "worker-src": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::BaseUri(sources.clone()), @r###"
        {
          "base-uri": [
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
          "sandbox": [
            "allow-forms"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FormAction(sources.clone()), @r###"
        {
          "form-action": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::FrameAncestors(sources.clone()), @r###"
        {
          "frame-ancestors": [
            "'self'"
          ]
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ReportTo("https://google.com".to_string()), @r###"
        {
          "report-to": "https://google.com"
        }
        "###);

        assert_json_snapshot!(ContentSecurityPolicyDirective::ReportUri(sources), @r###"
        {
          "report-uri": [
            "'self'"
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let sources = ["'self'".to_string()].into_iter().collect::<HashSet<_>>();
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "child-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ChildSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "connect-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ConnectSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "default-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::DefaultSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "font-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FontSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "frame-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FrameSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "img-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ImgSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "manifest-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ManifestSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "media-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::MediaSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "object-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ObjectSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "script-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ScriptSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "script-src-elem": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ScriptSrcElem(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "script-src-attr": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ScriptSrcAttr(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "style-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::StyleSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "style-src-elem": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::StyleSrcElem(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "style-src-attr": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::StyleSrcAttr(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "webrtc": "'allow'" }"###
            )?,
            ContentSecurityPolicyDirective::Webrtc(
                ContentSecurityPolicyWebrtcDirectiveValue::Allow
            )
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "worker-src": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::WorkerSrc(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "base-uri": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::BaseUri(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "sandbox": ["allow-forms", "allow-top-navigation"] }"###
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
                r###"{ "form-action": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FormAction(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "frame-ancestors": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::FrameAncestors(sources.clone())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "report-to": "https://google.com" }"###
            )?,
            ContentSecurityPolicyDirective::ReportTo("https://google.com".to_string())
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyDirective>(
                r###"{ "report-uri": ["'self'"] }"###
            )?,
            ContentSecurityPolicyDirective::ReportUri(sources)
        );

        Ok(())
    }
}
