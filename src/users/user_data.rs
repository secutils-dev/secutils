use crate::users::UserId;
use time::OffsetDateTime;

pub mod export;
pub mod import;
mod shared;

pub use self::{
    export::{UserDataExportParams, generate_export},
    import::{
        UserDataImportParams, UserDataImportPreviewParams, execute_import, generate_import_preview,
    },
};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct UserData<V> {
    pub user_id: UserId,
    pub key: Option<String>,
    pub value: V,
    pub timestamp: OffsetDateTime,
}

impl<V> UserData<V> {
    pub fn new(user_id: UserId, value: V, timestamp: OffsetDateTime) -> Self {
        Self {
            user_id,
            key: None,
            value,
            timestamp,
        }
    }
}
