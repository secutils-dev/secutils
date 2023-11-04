mod shared_resource;
mod user_share_id;

use crate::users::UserId;
use serde::Serialize;
use time::OffsetDateTime;

pub use self::{
    shared_resource::{ClientSharedResource, SharedResource},
    user_share_id::UserShareId,
};

/// Represents a shared user resource.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct UserShare {
    pub id: UserShareId,
    pub user_id: UserId,
    pub resource: SharedResource,
    pub created_at: OffsetDateTime,
}

/// A special version of UserShare that can be safely serialized for the client side since not
/// all Serde attributes we need can be serialized with postcard (main serialization format). It
/// also excludes the user ID since it shouldn't be exposed to the client side.
#[derive(Serialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientUserShare {
    pub id: UserShareId,
    pub resource: ClientSharedResource,
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

impl From<UserShare> for ClientUserShare {
    fn from(user_share: UserShare) -> Self {
        Self {
            id: user_share.id,
            resource: user_share.resource.into(),
            created_at: user_share.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientSharedResource, ClientUserShare};
    use crate::users::{SharedResource, UserId, UserShare, UserShareId};

    #[test]
    fn can_create_client_user_share() {
        let user_share_id = UserShareId::new();
        let resource = SharedResource::content_security_policy("my-policy");
        let created_at = time::OffsetDateTime::now_utc();

        assert_eq!(
            ClientUserShare::from(UserShare {
                id: user_share_id,
                user_id: UserId::default(),
                resource: resource.clone(),
                created_at,
            }),
            ClientUserShare {
                id: user_share_id,
                resource: ClientSharedResource::from(resource),
                created_at,
            }
        );
    }
}
