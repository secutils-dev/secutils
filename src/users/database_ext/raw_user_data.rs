use crate::users::UserData;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserData {
    pub user_id: Uuid,
    pub key: Option<String>,
    pub value: Vec<u8>,
    pub timestamp: OffsetDateTime,
}

impl<V: for<'de> Deserialize<'de>> TryFrom<RawUserData> for UserData<V> {
    type Error = anyhow::Error;

    fn try_from(raw_user_data: RawUserData) -> Result<Self, Self::Error> {
        Ok(UserData {
            user_id: raw_user_data.user_id.into(),
            key: raw_user_data
                .key
                .and_then(|key| if key.is_empty() { None } else { Some(key) }),
            value: serde_json::from_slice(raw_user_data.value.as_slice())
                .with_context(|| "Cannot deserialize user data value")?,
            timestamp: raw_user_data.timestamp,
        })
    }
}

impl<'u, V: Serialize> TryFrom<&'u UserData<V>> for RawUserData {
    type Error = anyhow::Error;

    fn try_from(user_data: &'u UserData<V>) -> Result<Self, Self::Error> {
        Ok(Self {
            user_id: *user_data.user_id,
            key: user_data.key.clone(),
            value: serde_json::ser::to_vec(&user_data.value)
                .with_context(|| "Cannot serialize user data value")?,
            timestamp: user_data.timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUserData;
    use crate::users::UserData;
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_user_data() -> anyhow::Result<()> {
        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            user_id: uuid!("00000000-0000-0000-0000-000000000001"),
            key: None,
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })?, @r###"
        UserData {
            user_id: UserId(
                00000000-0000-0000-0000-000000000001,
            ),
            key: None,
            value: "hello",
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            user_id: uuid!("00000000-0000-0000-0000-000000000001"),
            key: Some("some-key".to_string()),
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })?, @r###"
        UserData {
            user_id: UserId(
                00000000-0000-0000-0000-000000000001,
            ),
            key: Some(
                "some-key",
            ),
            value: "hello",
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            user_id: uuid!("00000000-0000-0000-0000-000000000001"),
            key: Some("".to_string()),
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })?, @r###"
        UserData {
            user_id: UserId(
                00000000-0000-0000-0000-000000000001,
            ),
            key: None,
            value: "hello",
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_user_data() -> anyhow::Result<()> {
        assert_eq!(
            RawUserData::try_from(&UserData::new(
                uuid!("00000000-0000-0000-0000-000000000001").into(),
                "data",
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))?,
            RawUserData {
                user_id: uuid!("00000000-0000-0000-0000-000000000001"),
                key: None,
                value: serde_json::to_vec("data")?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }
}
