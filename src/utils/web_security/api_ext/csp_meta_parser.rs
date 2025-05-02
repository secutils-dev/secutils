use crate::utils::web_security::ContentSecurityPolicySource;
use anyhow::anyhow;
use bytes::Bytes;
use html5ever::{
    LocalName, QualName, namespace_url, ns,
    tendril::{ByteTendril, ReadExt, fmt, fmt::Slice},
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};
use std::cell::Cell;
use tracing::error;

/// Parses content security policies from a `<meta>` HTML tags.
pub struct CspMetaParser;
impl CspMetaParser {
    /// Takes HTML document bytes and returns a list of CSP values found in `<meta>` tags.
    pub fn parse(html_bytes: &Bytes) -> anyhow::Result<Vec<String>> {
        let mut chunk = ByteTendril::new();
        html_bytes.as_bytes().read_to_tendril(&mut chunk)?;

        // Make sure HTML content is a valid UTF-8 text.
        let utf8_chunk = chunk
            .try_reinterpret::<fmt::UTF8>()
            .map_err(|_| anyhow!("HTML content isn't a valid UTF-8 text."))?;

        let input = BufferQueue::default();
        input.push_back(utf8_chunk);

        // Start tokenizing and collect CSP `<meta>` tags.
        let mut sink = CspMetaTokenSink::new();
        let tokenizer = Tokenizer::new(&mut sink, TokenizerOpts::default());
        let _ = tokenizer.feed(&input);
        tokenizer.end();

        Ok(sink.csp_values.take())
    }
}

/// Serves as a sink for the tokenizer that parses HTML.
struct CspMetaTokenSink {
    csp_values: Cell<Vec<String>>,
    equiv_attr_name: QualName,
    content_attr_name: QualName,
}

impl CspMetaTokenSink {
    /// Creates new sink.
    fn new() -> Self {
        Self {
            csp_values: Cell::new(vec![]),
            equiv_attr_name: QualName::new(None, ns!(), LocalName::from("http-equiv")),
            content_attr_name: QualName::new(None, ns!(), LocalName::from("content")),
        }
    }
}
impl TokenSink for &mut CspMetaTokenSink {
    type Handle = ();

    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<Self::Handle> {
        if let Token::TagToken(tag) = token {
            if tag.kind != TagKind::StartTag {
                return TokenSinkResult::Continue;
            }

            // We can stop parsing document as soon as we hit `<body>` tag (if present), as we are
            // only interested in `<meta>` tags, that are supposed to be placed within `<head>`.
            let tag_name = (*tag.name).to_ascii_lowercase();
            if tag_name == "body" {
                return TokenSinkResult::Script(());
            }

            // If it's a CSP meta tag, then we need to extract its value.
            if tag_name == "meta" {
                let header_name = ContentSecurityPolicySource::Meta.header_name();
                let is_csp_meta = tag.attrs.iter().any(|attr| {
                    attr.name == self.equiv_attr_name
                        && (*attr.value).to_ascii_lowercase() == header_name
                });

                if is_csp_meta {
                    let csp_meta_content = tag.attrs.into_iter().find_map(|attr| {
                        if attr.name == self.content_attr_name {
                            Some((*attr.value).to_string())
                        } else {
                            None
                        }
                    });
                    if let Some(csp_meta_content) = csp_meta_content {
                        let mut csp_values = self.csp_values.take();
                        csp_values.push(csp_meta_content);
                        self.csp_values.set(csp_values);
                    } else {
                        error!(
                            "Found `<meta http-equiv='Content-Security-Policy'>` tag without `content` attribute."
                        );
                    }
                }
            }
        }

        TokenSinkResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::CspMetaParser;
    use bytes::Bytes;

    #[test]
    fn parses_csp_meta() -> anyhow::Result<()> {
        let html_simple_meta = r#"
            <!DOCTYPE html>
            <meta charset="utf-8">
            <meta http-equiv="Content-Security-Policy" content="default-src 'self'">
            <title>Hello, world!</title>
            <h1 class="foo">Hello, <i>world!</i></h1>
        "#;

        let html_meta_in_head = r#"
            <!DOCTYPE html>
            <head>
                <meta charset="utf-8">
                <meta http-equiv="Content-Security-Policy" content="default-src 'self'">
            </head>
            <title>Hello, world!</title>
            <h1 class="foo">Hello, <i>world!</i></h1>
        "#;

        let html_meta_upper_case = r#"
            <!DOCTYPE html>
            <head>
                <meta charset="utf-8">
                <META HTTP-EQUIV="Content-Security-Policy" Content="default-src 'self'">
                <title>Hello, world!</title>
            </head>
            <h1 class="foo">Hello, <i>world!</i></h1>
        "#;

        let html_multiple_meta = r#"
            <!DOCTYPE html>
            <head>
                <meta charset="utf-8">
                <meta http-equiv="Content-Security-Policy" content="default-src 'self'">
                <title>Hello, world!</title>
                <meta http-equiv="Content-Security-Policy" content="script-src 'unsafe-inline'">
            </head>
            <body>
                <h1 class="foo">Hello, <i>world!</i></h1>
                <meta http-equiv="Content-Security-Policy" content="img-src 'self'">
            </body>
        "#;

        for html in [html_simple_meta, html_meta_in_head, html_meta_upper_case] {
            assert_eq!(
                CspMetaParser::parse(&Bytes::from(html))?,
                vec!["default-src 'self'".to_string()]
            );
        }

        assert_eq!(
            CspMetaParser::parse(&Bytes::from(html_multiple_meta))?,
            vec![
                "default-src 'self'".to_string(),
                "script-src 'unsafe-inline'".to_string()
            ]
        );

        Ok(())
    }
}
