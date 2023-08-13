use crate::users::{UserData, UserId};
use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserData {
    pub user_id: i64,
    pub key: Option<String>,
    pub value: Vec<u8>,
    pub timestamp: i64,
}

impl<V: DeserializeOwned> TryFrom<RawUserData> for UserData<V> {
    type Error = anyhow::Error;

    fn try_from(raw_user_data: RawUserData) -> Result<Self, Self::Error> {
        Ok(UserData {
            user_id: UserId(raw_user_data.user_id),
            key: raw_user_data
                .key
                .and_then(|key| if key.is_empty() { None } else { Some(key) }),
            value: serde_json::from_slice(raw_user_data.value.as_slice())
                .with_context(|| "Cannot deserialize user data value")?,
            timestamp: OffsetDateTime::from_unix_timestamp(raw_user_data.timestamp)?,
        })
    }
}

impl<'u, V: Serialize> TryFrom<&'u UserData<V>> for RawUserData {
    type Error = anyhow::Error;

    fn try_from(user_data: &'u UserData<V>) -> Result<Self, Self::Error> {
        Ok(Self {
            user_id: user_data.user_id.0,
            key: user_data.key.clone(),
            value: serde_json::ser::to_vec(&user_data.value)
                .with_context(|| "Cannot serialize user data value")?,
            timestamp: user_data.timestamp.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawUserData;
    use crate::users::{UserData, UserId};
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_user_data() -> anyhow::Result<()> {
        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            user_id: 0,
            key: None,
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: 946720800,
        })?, @r###"
        UserData {
            user_id: UserId(
                0,
            ),
            key: None,
            value: "hello",
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            user_id: 0,
            key: Some("some-key".to_string()),
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: 946720800,
        })?, @r###"
        UserData {
            user_id: UserId(
                0,
            ),
            key: Some(
                "some-key",
            ),
            value: "hello",
            timestamp: 2000-01-01 10:00:00.0 +00:00:00,
        }
        "###);

        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            user_id: 0,
            key: Some("".to_string()),
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: 946720800,
        })?, @r###"
        UserData {
            user_id: UserId(
                0,
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
                UserId::empty(),
                "data",
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))?,
            RawUserData {
                user_id: UserId::empty().0,
                key: None,
                value: serde_json::to_vec("data")?,
                // January 1, 2000 11:00:00
                timestamp: 946720800,
            }
        );

        assert_eq!(
            RawUserData::try_from(&UserData::new_with_key(
                UserId::empty(),
                "some-key",
                "data",
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))?,
            RawUserData {
                user_id: UserId::empty().0,
                key: Some("some-key".to_string()),
                value: serde_json::to_vec("data")?,
                // January 1, 2000 11:00:00
                timestamp: 946720800,
            }
        );

        Ok(())
    }
}
