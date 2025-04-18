use crate::users::UserShare;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserShare {
    pub id: Uuid,
    pub user_id: Uuid,
    pub resource: Vec<u8>,
    pub created_at: OffsetDateTime,
}

impl TryFrom<RawUserShare> for UserShare {
    type Error = anyhow::Error;

    fn try_from(raw_user_share: RawUserShare) -> Result<Self, Self::Error> {
        Ok(UserShare {
            id: raw_user_share.id.into(),
            user_id: raw_user_share.user_id.into(),
            resource: postcard::from_bytes(&raw_user_share.resource)?,
            created_at: raw_user_share.created_at,
        })
    }
}

impl TryFrom<&UserShare> for RawUserShare {
    type Error = anyhow::Error;

    fn try_from(user_share: &UserShare) -> Result<Self, Self::Error> {
        Ok(RawUserShare {
            id: (&user_share.id).into(),
            user_id: *user_share.user_id,
            resource: postcard::to_stdvec(&user_share.resource)?,
            created_at: user_share.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUserShare;
    use crate::users::{SharedResource, UserShare};
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_user_share() -> anyhow::Result<()> {
        assert_debug_snapshot!(UserShare::try_from(RawUserShare {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            user_id: uuid!("00000000-0000-0000-0000-000000000002"),
            resource: vec![0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            // January 1, 2000 10:00:00
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        })?, @r###"
        UserShare {
            id: UserShareId(
                00000000-0000-0000-0000-000000000001,
            ),
            user_id: UserId(
                00000000-0000-0000-0000-000000000002,
            ),
            resource: ContentSecurityPolicy {
                policy_id: 00000000-0000-0000-0000-000000000001,
            },
            created_at: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_share() -> anyhow::Result<()> {
        assert_debug_snapshot!(RawUserShare::try_from(&UserShare {
            id: uuid!("00000000-0000-0000-0000-000000000001").into(),
            user_id: uuid!("00000000-0000-0000-0000-000000000002").into(),
            resource: SharedResource::content_security_policy(uuid!("00000000-0000-0000-0000-000000000001")),
            // January 1, 2000 10:00:00
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        })?, @r###"
        RawUserShare {
            id: 00000000-0000-0000-0000-000000000001,
            user_id: 00000000-0000-0000-0000-000000000002,
            resource: [
                0,
                16,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                1,
            ],
            created_at: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        Ok(())
    }

    #[test]
    fn fails_if_malformed() -> anyhow::Result<()> {
        assert!(
            UserShare::try_from(RawUserShare {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                user_id: uuid!("00000000-0000-0000-0000-000000000002"),
                resource: vec![1, 2, 3],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })
            .is_err()
        );

        Ok(())
    }
}
