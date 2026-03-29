use crate::users::UserId;
use serde::{Deserialize, Serialize};
use std::fmt;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents the type of user script, determining where it can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserScriptType {
    /// Script for webhook responders (Deno runtime).
    Responder,
    /// Script for API tracker request configuration (Deno runtime).
    ApiConfigurator,
    /// Script for API tracker response extraction (Deno runtime).
    ApiExtractor,
    /// Script for page tracker content extraction (Playwright/Node.js runtime).
    PageExtractor,
    /// Script that can be used in any context.
    Universal,
}

impl fmt::Display for UserScriptType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserScriptType::Responder => write!(f, "responder"),
            UserScriptType::ApiConfigurator => write!(f, "api_configurator"),
            UserScriptType::ApiExtractor => write!(f, "api_extractor"),
            UserScriptType::PageExtractor => write!(f, "page_extractor"),
            UserScriptType::Universal => write!(f, "universal"),
        }
    }
}

impl UserScriptType {
    /// Returns the string representation of this script type.
    pub fn as_str(&self) -> &'static str {
        match self {
            UserScriptType::Responder => "responder",
            UserScriptType::ApiConfigurator => "api_configurator",
            UserScriptType::ApiExtractor => "api_extractor",
            UserScriptType::PageExtractor => "page_extractor",
            UserScriptType::Universal => "universal",
        }
    }

    /// Parses a string into a `UserScriptType`.
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "responder" => Ok(UserScriptType::Responder),
            "api_configurator" => Ok(UserScriptType::ApiConfigurator),
            "api_extractor" => Ok(UserScriptType::ApiExtractor),
            "page_extractor" => Ok(UserScriptType::PageExtractor),
            "universal" => Ok(UserScriptType::Universal),
            _ => anyhow::bail!("Unknown script type: {s}"),
        }
    }

    /// Returns true if this script type is compatible with the given context.
    ///
    /// Compatibility matrix:
    /// - Responder: responders only
    /// - ApiConfigurator: API trackers only
    /// - ApiExtractor: API trackers only
    /// - PageExtractor: Page trackers only
    /// - Universal: all contexts
    pub fn is_compatible_with(&self, context: ScriptContext) -> bool {
        matches!(
            (self, context),
            (UserScriptType::Responder, ScriptContext::Responder)
                | (UserScriptType::ApiConfigurator, ScriptContext::ApiTracker)
                | (UserScriptType::ApiExtractor, ScriptContext::ApiTracker)
                | (UserScriptType::PageExtractor, ScriptContext::PageTracker)
                | (UserScriptType::Universal, _)
        )
    }
}

/// Represents the context where a script can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScriptContext {
    /// Webhook responder script context.
    Responder,
    /// API tracker script context (configurator or extractor).
    ApiTracker,
    /// Page tracker script context.
    PageTracker,
}

/// Represents a user-defined script for reuse across responders and trackers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserScript {
    /// Unique identifier for the script.
    pub id: Uuid,
    /// The user who owns this script.
    #[serde(skip)]
    pub user_id: UserId,
    /// The script name (used to reference it in the UI).
    pub name: String,
    /// The type of script, determining compatible contexts.
    pub script_type: UserScriptType,
    /// The script content (the actual code).
    pub content: String,
    /// Tags assigned to this script.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<crate::users::EntityTag>,
    /// When the script was first created.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub created_at: OffsetDateTime,
    /// When the script content was last updated.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub updated_at: OffsetDateTime,
}

/// Represents a request to create or update a user script.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct UserScriptRequest {
    pub name: String,
    pub script_type: UserScriptType,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::{ScriptContext, UserScriptType};

    #[test]
    fn script_type_compatibility() {
        // Responder type is only compatible with the responder context
        assert!(UserScriptType::Responder.is_compatible_with(ScriptContext::Responder));
        assert!(!UserScriptType::Responder.is_compatible_with(ScriptContext::ApiTracker));
        assert!(!UserScriptType::Responder.is_compatible_with(ScriptContext::PageTracker));

        // API configurator/extractor types are only compatible with API tracker context
        assert!(!UserScriptType::ApiConfigurator.is_compatible_with(ScriptContext::Responder));
        assert!(UserScriptType::ApiConfigurator.is_compatible_with(ScriptContext::ApiTracker));
        assert!(!UserScriptType::ApiConfigurator.is_compatible_with(ScriptContext::PageTracker));

        assert!(!UserScriptType::ApiExtractor.is_compatible_with(ScriptContext::Responder));
        assert!(UserScriptType::ApiExtractor.is_compatible_with(ScriptContext::ApiTracker));
        assert!(!UserScriptType::ApiExtractor.is_compatible_with(ScriptContext::PageTracker));

        // Page extractor type is only compatible with page tracker context
        assert!(!UserScriptType::PageExtractor.is_compatible_with(ScriptContext::Responder));
        assert!(!UserScriptType::PageExtractor.is_compatible_with(ScriptContext::ApiTracker));
        assert!(UserScriptType::PageExtractor.is_compatible_with(ScriptContext::PageTracker));

        // Universal type is compatible with all contexts
        assert!(UserScriptType::Universal.is_compatible_with(ScriptContext::Responder));
        assert!(UserScriptType::Universal.is_compatible_with(ScriptContext::ApiTracker));
        assert!(UserScriptType::Universal.is_compatible_with(ScriptContext::PageTracker));
    }

    #[test]
    fn script_type_display() {
        assert_eq!(UserScriptType::Responder.to_string(), "responder");
        assert_eq!(
            UserScriptType::ApiConfigurator.to_string(),
            "api_configurator"
        );
        assert_eq!(UserScriptType::ApiExtractor.to_string(), "api_extractor");
        assert_eq!(UserScriptType::PageExtractor.to_string(), "page_extractor");
        assert_eq!(UserScriptType::Universal.to_string(), "universal");
    }

    #[test]
    fn script_type_serialization() {
        use insta::assert_json_snapshot;

        assert_json_snapshot!(UserScriptType::Responder, @"\"responder\"");
        assert_json_snapshot!(UserScriptType::ApiConfigurator, @"\"api_configurator\"");
        assert_json_snapshot!(UserScriptType::ApiExtractor, @"\"api_extractor\"");
        assert_json_snapshot!(UserScriptType::PageExtractor, @"\"page_extractor\"");
        assert_json_snapshot!(UserScriptType::Universal, @"\"universal\"");
    }

    #[test]
    fn script_type_deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UserScriptType>("\"responder\"")?,
            UserScriptType::Responder
        );
        assert_eq!(
            serde_json::from_str::<UserScriptType>("\"api_configurator\"")?,
            UserScriptType::ApiConfigurator
        );
        assert_eq!(
            serde_json::from_str::<UserScriptType>("\"api_extractor\"")?,
            UserScriptType::ApiExtractor
        );
        assert_eq!(
            serde_json::from_str::<UserScriptType>("\"page_extractor\"")?,
            UserScriptType::PageExtractor
        );
        assert_eq!(
            serde_json::from_str::<UserScriptType>("\"universal\"")?,
            UserScriptType::Universal
        );

        Ok(())
    }
}
