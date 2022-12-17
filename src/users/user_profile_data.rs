use crate::utils::{AutoResponder, USER_PROFILE_DATA_KEY_AUTO_RESPONDERS};
use anyhow::{bail, Context};
use std::collections::BTreeMap;

pub struct UserProfileData;
impl UserProfileData {
    pub fn merge(
        from_data: BTreeMap<String, String>,
        mut to_data: BTreeMap<String, String>,
    ) -> anyhow::Result<BTreeMap<String, String>> {
        for (key, value) in from_data {
            let value = match key.as_str() {
                USER_PROFILE_DATA_KEY_AUTO_RESPONDERS => {
                    let from_value =
                        serde_json::from_str::<BTreeMap<String, Option<AutoResponder>>>(&value)
                            .with_context(|| {
                                "Cannot deserialize new responders data".to_string()
                            })?;

                    let mut to_value = if let Some(to_value) = to_data.get(&key) {
                        serde_json::from_str::<BTreeMap<String, AutoResponder>>(to_value)
                            .with_context(|| {
                                "Cannot deserialize stored responders data".to_string()
                            })?
                    } else {
                        BTreeMap::new()
                    };

                    for (alias, auto_responder) in from_value {
                        if let Some(auto_responder) = auto_responder {
                            if !auto_responder.is_valid() {
                                bail!("Responder is not valid: {:?}", auto_responder);
                            }
                            to_value.insert(alias, auto_responder);
                        } else {
                            to_value.remove(&alias);
                        }
                    }

                    if to_value.is_empty() {
                        None
                    } else {
                        Some(serde_json::to_string(&to_value)?)
                    }
                }
                _ => {
                    if value.is_empty() {
                        None
                    } else {
                        Some(value)
                    }
                }
            };

            if let Some(value) = value {
                to_data.insert(key, value);
            } else {
                to_data.remove(&key);
            }
        }

        Ok(to_data)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::UserProfileData,
        utils::{
            tests::MockAutoResponder, AutoResponder, AutoResponderMethod,
            USER_PROFILE_DATA_KEY_AUTO_RESPONDERS,
        },
    };
    use std::collections::BTreeMap;

    fn serialize_responders<'a>(
        responders: impl IntoIterator<Item = (&'a str, &'a AutoResponder)>,
    ) -> anyhow::Result<String> {
        Ok(serde_json::to_string(
            &responders.into_iter().collect::<BTreeMap<_, _>>(),
        )?)
    }

    fn serialize_optional_responders<'a>(
        responders: impl IntoIterator<Item = (&'a str, Option<&'a AutoResponder>)>,
    ) -> anyhow::Result<String> {
        Ok(serde_json::to_string(
            &responders.into_iter().collect::<BTreeMap<_, _>>(),
        )?)
    }

    #[test]
    fn can_merge_generic_data() -> anyhow::Result<()> {
        assert_eq!(
            UserProfileData::merge(
                [("key".to_string(), "value".to_string())]
                    .into_iter()
                    .collect(),
                [].into_iter().collect()
            )?,
            [("key".to_string(), "value".to_string())]
                .into_iter()
                .collect()
        );

        assert_eq!(
            UserProfileData::merge(
                [("key".to_string(), "value".to_string())]
                    .into_iter()
                    .collect(),
                [("key".to_string(), "old-value".to_string())]
                    .into_iter()
                    .collect()
            )?,
            [("key".to_string(), "value".to_string())]
                .into_iter()
                .collect()
        );

        assert_eq!(
            UserProfileData::merge(
                [("key".to_string(), "value".to_string())]
                    .into_iter()
                    .collect(),
                [("another-key".to_string(), "another-value".to_string())]
                    .into_iter()
                    .collect()
            )?,
            [
                ("key".to_string(), "value".to_string()),
                ("another-key".to_string(), "another-value".to_string())
            ]
            .into_iter()
            .collect()
        );

        assert_eq!(
            UserProfileData::merge(
                [("key".to_string(), "".to_string())].into_iter().collect(),
                [("key".to_string(), "old-value".to_string())]
                    .into_iter()
                    .collect()
            )?,
            [].into_iter().collect()
        );

        assert_eq!(
            UserProfileData::merge(
                [("key".to_string(), "".to_string())].into_iter().collect(),
                [].into_iter().collect()
            )?,
            [].into_iter().collect()
        );

        Ok(())
    }

    #[test]
    fn can_merge_auto_responders_data() -> anyhow::Result<()> {
        let auto_responder_one =
            MockAutoResponder::new("test-1-alias", AutoResponderMethod::Post, 300).build();
        let auto_responder_two =
            MockAutoResponder::new("test-2-alias", AutoResponderMethod::Post, 300)
                .set_body("body")
                .set_headers(vec![(
                    "Content-Type".to_string(),
                    "application/json".to_string(),
                )])
                .build();
        let auto_responder_two_existing =
            MockAutoResponder::new("test-2-alias", AutoResponderMethod::Get, 300)
                .set_body("body")
                .build();
        let auto_responder_three_existing =
            MockAutoResponder::new("test-3-alias", AutoResponderMethod::Options, 403).build();

        // Fill empty data.
        assert_eq!(
            UserProfileData::merge(
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([
                        (auto_responder_one.alias.as_str(), &auto_responder_one),
                        (auto_responder_two.alias.as_str(), &auto_responder_two)
                    ])?
                )]
                .into_iter()
                .collect(),
                [].into_iter().collect()
            )?,
            [(
                USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                serialize_responders([
                    (auto_responder_one.alias.as_str(), &auto_responder_one),
                    (auto_responder_two.alias.as_str(), &auto_responder_two)
                ])?
            )]
            .into_iter()
            .collect()
        );

        // Overwrite existing data.
        assert_eq!(
            UserProfileData::merge(
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([
                        (auto_responder_one.alias.as_str(), &auto_responder_one),
                        (auto_responder_two.alias.as_str(), &auto_responder_two)
                    ])?
                )]
                .into_iter()
                .collect(),
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([(
                        auto_responder_two_existing.alias.as_str(),
                        &auto_responder_two_existing
                    )])?
                )]
                .into_iter()
                .collect()
            )?,
            [(
                USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                serialize_responders([
                    (auto_responder_one.alias.as_str(), &auto_responder_one),
                    (auto_responder_two.alias.as_str(), &auto_responder_two)
                ])?
            )]
            .into_iter()
            .collect()
        );

        // Overwrite existing data and preserve non-conflicting existing data.
        assert_eq!(
            UserProfileData::merge(
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([
                        (auto_responder_one.alias.as_str(), &auto_responder_one),
                        (auto_responder_two.alias.as_str(), &auto_responder_two)
                    ])?
                )]
                .into_iter()
                .collect(),
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([
                        (
                            auto_responder_two_existing.alias.as_str(),
                            &auto_responder_two_existing
                        ),
                        (
                            auto_responder_three_existing.alias.as_str(),
                            &auto_responder_three_existing
                        )
                    ])?
                )]
                .into_iter()
                .collect()
            )?,
            [(
                USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                serialize_responders([
                    (auto_responder_one.alias.as_str(), &auto_responder_one),
                    (auto_responder_two.alias.as_str(), &auto_responder_two),
                    (
                        auto_responder_three_existing.alias.as_str(),
                        &auto_responder_three_existing
                    ),
                ])?
            )]
            .into_iter()
            .collect()
        );

        // Delete existing data.
        assert_eq!(
            UserProfileData::merge(
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_optional_responders([
                        (auto_responder_one.alias.as_str(), Some(&auto_responder_one)),
                        (auto_responder_two.alias.as_str(), None)
                    ])?
                )]
                .into_iter()
                .collect(),
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([
                        (
                            auto_responder_two_existing.alias.as_str(),
                            &auto_responder_two_existing
                        ),
                        (
                            auto_responder_three_existing.alias.as_str(),
                            &auto_responder_three_existing
                        )
                    ])?
                )]
                .into_iter()
                .collect()
            )?,
            [(
                USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                serialize_responders([
                    (auto_responder_one.alias.as_str(), &auto_responder_one),
                    (
                        auto_responder_three_existing.alias.as_str(),
                        &auto_responder_three_existing
                    ),
                ])?
            )]
            .into_iter()
            .collect()
        );

        // Delete full slot.
        assert_eq!(
            UserProfileData::merge(
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_optional_responders([(auto_responder_two.alias.as_str(), None)])?
                )]
                .into_iter()
                .collect(),
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_responders([(
                        auto_responder_two_existing.alias.as_str(),
                        &auto_responder_two_existing
                    ),])?
                )]
                .into_iter()
                .collect()
            )?,
            [].into_iter().collect()
        );

        // Does nothing if there is nothing to delete.
        assert_eq!(
            UserProfileData::merge(
                [(
                    USER_PROFILE_DATA_KEY_AUTO_RESPONDERS.to_string(),
                    serialize_optional_responders([(auto_responder_two.alias.as_str(), None)])?
                )]
                .into_iter()
                .collect(),
                [].into_iter().collect()
            )?,
            [].into_iter().collect()
        );

        Ok(())
    }
}
