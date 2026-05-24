pub mod api_ext;
mod api_keys;
mod database_ext;
mod notification_destinations;
mod scripts;
mod secrets;
mod settings;
mod tags;
mod user;
mod user_data;
mod user_id;
mod user_share;
mod user_subscription;

pub use self::{
    api_ext::errors::UserSignupError,
    api_keys::{
        ApiKeyCreateParams, ApiKeyCreateResponse, ApiKeyRegenerateParams, ApiKeyUpdateParams,
        UserApiKey,
    },
    notification_destinations::{
        NotificationChannelKind, NotificationEmailSetParams, NotificationEmailVerifyParams,
        ResolvedRecipient, UserNotificationDestination, resolve_recipient_for_user_id,
        unsubscribe_url,
    },
    scripts::{ScriptContext, ScriptCreateParams, ScriptUpdateParams, UserScript, UserScriptType},
    secrets::{SecretCreateParams, SecretUpdateParams, SecretsAccess, UserSecret},
    settings::{UserSettings, UserSettingsSetter},
    tags::{EntityTag, RawEntityTag, TagCreateParams, TagUpdateParams, UserTag, group_entity_tags},
    user::User,
    user_data::{
        UserDataExportParams, UserDataImportParams, UserDataImportPreviewParams, execute_import,
        generate_export, generate_import_preview,
    },
    user_id::UserId,
    user_share::{ClientSharedResource, ClientUserShare, SharedResource, UserShare, UserShareId},
    user_subscription::{
        ClientSubscriptionFeatures, SubscriptionFeatures, SubscriptionTier, UserSubscription,
    },
};

pub(crate) use self::secrets::RawSecretsAccess;

#[cfg(test)]
pub use self::notification_destinations::tests as notification_destinations_tests;
