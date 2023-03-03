use crate::users::UserData;
use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawUserData {
    pub value: Vec<u8>,
    pub timestamp: i64,
}

impl<V: DeserializeOwned> TryFrom<RawUserData> for UserData<V> {
    type Error = anyhow::Error;

    fn try_from(raw_user_data: RawUserData) -> Result<Self, Self::Error> {
        Ok(UserData {
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
            value: serde_json::ser::to_vec(&user_data.value)
                .with_context(|| "Cannot serialize user data value")?,
            timestamp: user_data.timestamp.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{datastore::primary_db::raw_user_data::RawUserData, users::UserData};
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn can_convert_into_user_data() -> anyhow::Result<()> {
        assert_debug_snapshot!(UserData::<String>::try_from(RawUserData {
            value: serde_json::to_vec("hello")?,
             // January 1, 2000 11:00:00
            timestamp: 946720800,
        })?, @r###"
        UserData {
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
                "data",
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))?,
            RawUserData {
                value: serde_json::to_vec("data")?,
                // January 1, 2000 11:00:00
                timestamp: 946720800,
            }
        );

        Ok(())
    }
}
