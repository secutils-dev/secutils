pub mod api_ext;
mod builtin_user;
mod builtin_users_initializer;
mod database_ext;
mod internal_user_data_namespace;
mod public_user_data_namespace;
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
    builtin_user::BuiltinUser,
    builtin_users_initializer::builtin_users_initializer,
    internal_user_data_namespace::InternalUserDataNamespace,
    public_user_data_namespace::PublicUserDataNamespace,
    user::User,
    user_data::UserData,
    user_data_key::UserDataKey,
    user_data_namespace::UserDataNamespace,
    user_id::UserId,
    user_settings::{UserSettings, UserSettingsSetter},
    user_share::{ClientUserShare, SharedResource, UserShare, UserShareId},
    user_subscription::{SubscriptionFeatures, SubscriptionTier, UserSubscription},
};

pub(crate) use self::api_ext::user_data_setters::DictionaryDataUserDataSetter;
