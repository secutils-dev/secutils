pub mod api_ext;
mod database_ext;
mod secrets;
mod user;
mod user_data;
mod user_data_key;
mod user_data_namespace;
mod user_id;
mod user_settings;
mod user_share;
mod user_subscription;

pub use self::{
    api_ext::errors::UserSignupError,
    secrets::SecretsAccess,
    user::User,
    user_data::UserData,
    user_data_key::UserDataKey,
    user_data_namespace::UserDataNamespace,
    user_id::UserId,
    user_settings::{UserSettings, UserSettingsSetter},
    user_share::{ClientUserShare, SharedResource, UserShare, UserShareId},
    user_subscription::{
        ClientSubscriptionFeatures, SubscriptionFeatures, SubscriptionTier, UserSubscription,
    },
};

pub(crate) use self::{
    api_ext::user_data_setters::DictionaryDataUserDataSetter, secrets::RawSecretsAccess,
};
